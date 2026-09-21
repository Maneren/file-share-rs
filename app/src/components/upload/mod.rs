//! Browser upload island.
//!
//! Server RPCs live in [`crate::api::upload`] /
//! [`crate::api::upload_progress`]; progress state lives in [`state`]; the
//! polling hook is [`progress_hook`].

use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
};

use leptos::{
    ev::SubmitEvent,
    html::{Form, Input},
    logging,
    prelude::*,
    task::spawn_local,
};
use web_sys::FormData;
use web_time::Instant;

mod form;
pub mod progress_bar;
pub mod progress_hook;
pub mod state;

use form::UploadForm;
use progress_bar::ProgressBar;
use progress_hook::subscribe_upload_progress;
pub use state::Progress;

use crate::api::upload::upload_file;

#[island]
pub fn FileUpload(path: PathBuf) -> impl IntoView {
    let current_upload = RwSignal::new(None::<(String, Progress)>);

    let file_ref: NodeRef<Input> = NodeRef::new();
    let form_ref: NodeRef<Form> = NodeRef::new();

    let on_submit = move |ev: SubmitEvent| {
        ev.prevent_default();

        let form = form_ref.get().unwrap();
        let form_data = FormData::new_with_form(&form).unwrap();

        let file_list = file_ref.get().unwrap().files().unwrap();

        let files = (0..file_list.length())
            .map(|i| file_list.get(i).unwrap())
            .collect::<Vec<_>>();

        if files.is_empty() {
            logging::warn!("No files selected. Aborting.");
            return;
        }

        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        let total = files.iter().map(|f| f.size() as u64).sum::<u64>();

        let id = {
            let mut hasher = DefaultHasher::default();

            for file in &files {
                file.name().hash(&mut hasher);
            }

            hasher.finish().to_string()
        };

        if current_upload.with(Option::is_some) {
            logging::warn!("Upload already in progress. Aborting.");
            return;
        }

        _ = form_data.set_with_str("id", &id);

        let _ = current_upload.write().insert((
            id.clone(),
            Progress {
                size: total,
                start_time: Instant::now(),
                uploaded: RwSignal::default(),
            },
        ));

        spawn_local(subscribe_upload_progress(id.clone(), current_upload));

        spawn_local(async move {
            upload_file(form_data.into())
                .await
                .expect("couldn't upload file");

            logging::log!("[{id}]\tfinished (upload)");
        });
    };

    view! {
        <div class="flex flex-col gap-2 grow">
            <UploadForm path=path file_ref=file_ref form_ref=form_ref on_submit=on_submit />

            {move || {
                current_upload
                    .read()
                    .as_ref()
                    .map(|(_, progress)| {
                        view! {
                            <ProgressBar
                                size=progress.size
                                start_time=progress.start_time
                                uploaded=progress.uploaded.read_only()
                            />
                        }
                    })
            }}
        </div>
    }
}
