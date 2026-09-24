//! Browser upload island: posts files to `/upload` with XHR progress.
//!
//! The form also works without JS (native POST to the same endpoint);
//! progress, cancel, error display and listing refresh need the island.

use std::{
    cell::RefCell,
    collections::VecDeque,
    path::PathBuf,
    rc::Rc,
    sync::{Mutex, OnceLock},
};

use leptos::{
    ev::SubmitEvent,
    html::{Form, Input},
    logging,
    prelude::*,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{FormData, ProgressEvent, XmlHttpRequest};
use web_time::Instant;

mod form;
pub mod progress_bar;
pub mod state;

use form::UploadForm;
use progress_bar::ProgressBar;
pub use state::Progress;

use crate::paths::encode_path;

/// Samples kept per upload for the speed average.
const MAX_SAMPLES: usize = 10;

/// Refresh callback of the currently mounted listing, if any. Cross-island
/// channel: islands can't share context, so the listing registers here and
/// the uploader notifies through it. Only meaningful client-side.
static LISTING_REFRESH: OnceLock<Mutex<Option<Callback<()>>>> = OnceLock::new();

fn refresh_slot() -> &'static Mutex<Option<Callback<()>>> {
    LISTING_REFRESH.get_or_init(|| Mutex::new(None))
}

/// Register the current listing's refresh; the latest mount wins.
pub fn set_listing_refresh(refresh: Callback<()>) {
    if let Ok(mut slot) = refresh_slot().lock() {
        *slot = Some(refresh);
    }
}

/// Forget the current listing so a later upload can't hit a dead island.
pub fn clear_listing_refresh() {
    if let Ok(mut slot) = refresh_slot().lock() {
        *slot = None;
    }
}

/// Refresh the mounted listing after a successful upload, if any.
fn notify_listing_changed() {
    if let Ok(slot) = refresh_slot().lock()
        && let Some(refresh) = *slot
    {
        refresh.run(());
    }
}

/// XHR event callbacks, held until the request terminates so the JS side
/// never outlives the Rust closures. Cleared from a microtask after a
/// terminal event, never synchronously inside a running callback.
#[derive(Default)]
struct XhrCallbacks {
    progress: Option<Closure<dyn FnMut(JsValue)>>,
    load: Option<Closure<dyn FnMut(JsValue)>>,
    error: Option<Closure<dyn FnMut(JsValue)>>,
    abort: Option<Closure<dyn FnMut(JsValue)>>,
}

impl XhrCallbacks {
    fn clear(&mut self) {
        *self = Self::default();
    }
}

#[island]
pub fn FileUpload(path: PathBuf) -> impl IntoView {
    let current_upload = RwSignal::new(None::<Progress>);
    let upload_error = RwSignal::new(None::<String>);
    let xhr_handle = RwSignal::new(None::<XmlHttpRequest>);

    let file_ref: NodeRef<Input> = NodeRef::new();
    let form_ref: NodeRef<Form> = NodeRef::new();

    let target_url = format!("/upload/{}", encode_path(&path));
    let xhr_url = target_url.clone();

    // A bare `fn` pointer satisfies `on_cleanup`'s `Send + Sync` bound,
    // which JS-backed types can't.
    on_cleanup(clear_listing_refresh);

    let cancel = Callback::new(move |_| {
        if let Some(xhr) = xhr_handle.get_untracked() {
            let _ = xhr.abort();
        }
        xhr_handle.set(None);
        current_upload.set(None);
    });
    // Abort an in-flight upload on navigation instead of completing into a
    // dead island.
    on_cleanup(move || cancel.run(()));

    let fail = Callback::new(move |message: String| {
        logging::warn!("{message}");
        upload_error.set(Some(message));
        xhr_handle.set(None);
        current_upload.set(None);
    });

    let succeed = Callback::new(move |_| {
        current_upload.set(None);
        xhr_handle.set(None);
        if let Some(input) = file_ref.get() {
            let _ = input.set_value("");
        }
        notify_listing_changed();
    });

    let on_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        upload_error.set(None);

        if current_upload.with(Option::is_some) {
            fail.run("Upload already in progress.".to_string());
            return;
        }
        let (Some(form), Some(input)) = (form_ref.get(), file_ref.get()) else {
            return;
        };
        let Ok(form_data) = FormData::new_with_form(&form) else {
            fail.run("Couldn't read selected files.".to_string());
            return;
        };
        let files = input
            .files()
            .map(|list| {
                (0..list.length())
                    .filter_map(|i| list.get(i))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if files.is_empty() {
            fail.run("No files selected.".to_string());
            return;
        }

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let total = files.iter().map(|file| file.size() as u64).sum::<u64>();

        let progress = Progress {
            size: total,
            start_time: Instant::now(),
            uploaded: RwSignal::new(VecDeque::new()),
        };
        let uploaded = progress.uploaded;
        current_upload.set(Some(progress));

        let Ok(xhr) = XmlHttpRequest::new() else {
            fail.run("Couldn't start upload.".to_string());
            return;
        };
        let Ok(upload) = xhr.upload() else {
            fail.run("Couldn't start upload.".to_string());
            return;
        };

        let callbacks: Rc<RefCell<XhrCallbacks>> = Rc::new(RefCell::new(XhrCallbacks::default()));

        let onprogress = Closure::wrap(Box::new(move |event: JsValue| {
            let event: ProgressEvent = event.unchecked_into();
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let loaded = event.loaded() as u64;
            uploaded.update(|samples| {
                if samples.len() >= MAX_SAMPLES {
                    samples.pop_front();
                }
                samples.push_back((loaded, Instant::now()));
            });
        }) as Box<dyn FnMut(_)>);
        upload.set_onprogress(Some(onprogress.as_ref().unchecked_ref()));

        // Each terminal handler schedules the holder's drop past the JS
        // stack; cloning inside keeps the handlers `FnMut`.
        let release_on = callbacks.clone();
        let onload = Closure::wrap(Box::new({
            let xhr = xhr.clone();
            move |_: JsValue| {
                let status = xhr.status().unwrap_or(0);
                if status == 200 {
                    succeed.run(());
                } else {
                    fail.run(format!("Upload failed (status {status})."));
                }
                let release_on = release_on.clone();
                leptos::task::spawn_local(async move {
                    release_on.borrow_mut().clear();
                });
            }
        }) as Box<dyn FnMut(_)>);
        xhr.set_onload(Some(onload.as_ref().unchecked_ref()));

        let release_on = callbacks.clone();
        let onerror = Closure::wrap(Box::new(move |_: JsValue| {
            fail.run("Upload failed (network error).".to_string());
            let release_on = release_on.clone();
            leptos::task::spawn_local(async move {
                release_on.borrow_mut().clear();
            });
        }) as Box<dyn FnMut(_)>);
        xhr.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        let release_on = callbacks.clone();
        let onabort = Closure::wrap(Box::new(move |_: JsValue| {
            xhr_handle.set(None);
            current_upload.set(None);
            let release_on = release_on.clone();
            leptos::task::spawn_local(async move {
                release_on.borrow_mut().clear();
            });
        }) as Box<dyn FnMut(_)>);
        xhr.set_onabort(Some(onabort.as_ref().unchecked_ref()));

        callbacks.borrow_mut().progress = Some(onprogress);
        callbacks.borrow_mut().load = Some(onload);
        callbacks.borrow_mut().error = Some(onerror);
        callbacks.borrow_mut().abort = Some(onabort);

        if xhr.open("POST", &xhr_url).is_err()
            || xhr.send_with_opt_form_data(Some(&form_data)).is_err()
        {
            fail.run("Couldn't start upload.".to_string());
            return;
        }
        xhr_handle.set(Some(xhr));
    };

    view! {
        <div class="flex flex-col gap-2 grow">
            <UploadForm
                action=target_url
                file_ref=file_ref
                form_ref=form_ref
                on_submit=on_submit
            />

            {move || {
                current_upload.get().map(|progress| {
                    view! {
                        <div class="flex gap-2 items-center">
                            <ProgressBar
                                size=progress.size
                                start_time=progress.start_time
                                uploaded=progress.uploaded.read_only()
                            />
                            <button
                                type="button"
                                class="btn btn-sm btn-ghost"
                                on:click=move |_| cancel.run(())
                            >
                                Cancel
                            </button>
                        </div>
                    }
                })
            }}
            {move || {
                upload_error.get().map(|error| {
                    view! {
                        <div class="alert alert-error" role="alert">
                            <span>{error}</span>
                        </div>
                    }
                })
            }}
        </div>
    }
}
