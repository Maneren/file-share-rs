#![allow(non_snake_case)]

use std::{path::PathBuf, sync::Arc};

pub mod archive;
mod components;
mod config;
mod error_template;
mod server;
#[cfg(feature = "ssr")]
mod state;
pub mod utils;

use leptos::{either::Either, prelude::*};
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    components::{Route, Router, Routes},
    hooks::use_params,
    params::Params,
};
use leptos_router_macro::path;
use urlencoding::decode;

pub use crate::config::{
    AppConfig, PATH_NOT_FOUND_MESSAGE, UPLOAD_DISABLED_MESSAGE, UPLOAD_READ_ERROR_MESSAGE,
    UPLOAD_STORE_ERROR_MESSAGE,
};
#[cfg(feature = "ssr")]
pub use crate::state::AppState;
use crate::{
    components::{Breadcrumbs, FileEntries, Loading, UploadBar},
    error_template::{AppError, ErrorTemplate},
    server::{ListQuery, NewFolder, list_dir},
    utils::page_window,
};

#[derive(PartialEq, Eq, Params, Debug)]
struct PathQuery {
    path: String,
}

/// Entries per directory page.
const PAGE_SIZE: usize = 100;

#[component]
#[allow(clippy::must_use_candidate)]
pub fn FilesPage() -> impl IntoView {
    let path_query = use_params::<PathQuery>();

    let path = Memo::new(move |_| {
        path_query
            .read()
            .as_ref()
            .ok()
            .and_then(|query| decode(&query.path).ok())
            .map_or_default(|path| PathBuf::from(path.as_ref()))
    });

    let create_folder_action = ServerAction::<NewFolder>::new();

    let page = RwSignal::new(0usize);
    // Reset paging on navigation or new folders. Setting an already-zero
    // page does not notify, so this only refetches when shrinking a page.
    Effect::new(move |_| {
        let _ = (path.get(), create_folder_action.version().get());
        page.set(0);
    });

    let listing = Resource::new(
        move || (path.get(), create_folder_action.version().get(), page.get()),
        |(path, _, page)| async move {
            // NOTE: the `Ok` error type must be annotated. Nothing else
            // pins it, and an ambiguous error type surfaces as bogus
            // `FnMut`/`IntoView` errors on the `Transition` below instead
            // of an inference error.
            Ok::<_, ServerFnError>(
                list_dir(ListQuery {
                    path,
                    limit: PAGE_SIZE,
                    offset: page * PAGE_SIZE,
                })
                .await?,
            )
        },
    );

    let path_signal = Signal::from(path);

    let app_config = expect_context::<Arc<AppConfig>>();

    let upload_bar = app_config.allow_upload.then(|| {
        view! { <UploadBar path=path_signal create_folder_action=create_folder_action /> }
    });

    view! {
      <div class="p-3 App">
        {upload_bar} <Breadcrumbs path=path_signal />
        <div class="grid gap-2 mb-1 border-b grid-cols-(--entry-cols-mobile) border-base-content md:grid-cols-(--entry-cols)">
          <span></span>
          <span>Name</span>
          <span>Size</span>
          <span class="hidden md:inline">Last Modified</span>
        </div>
        <Transition fallback=Loading>
          {move || Suspend::new(async move {
            match listing.await {
              Ok(page_data) => {
                Either::Left(
                  view! {
                    <FileEntries path=path_signal entries=page_data.entries />
                    <Pagination page=page total=page_data.total />
                  },
                )
              }
              Err(e) => Either::Right(view! { <p class="text-lg">{format!("{e}")}</p> }),
            }
          })}
        </Transition>
      </div>
    }
}

#[component]
fn Pagination(page: RwSignal<usize>, total: usize) -> impl IntoView {
    let pages = total.div_ceil(PAGE_SIZE);

    view! {
      <div class="flex flex-col items-center gap-1 my-2">
        <span class="text-sm opacity-70">
          {move || {
            if total == 0 {
              "No entries".to_owned()
            } else {
              let start = page.get() * PAGE_SIZE + 1;
              let end = ((page.get() + 1) * PAGE_SIZE).min(total);
              format!("Showing {start}–{end} of {total}")
            }
          }}
        </span>
        <Show when=move || { pages > 1 }>
          <div class="join">
            <button
              class="join-item btn btn-sm"
              disabled=move || page.get() == 0
              on:click=move |_| page.set(0)
            >
              "«"
            </button>
            <button
              class="join-item btn btn-sm"
              disabled=move || page.get() == 0
              on:click=move |_| page.update(|page| *page = page.saturating_sub(1))
            >
              "‹"
            </button>
            {move || {
              page_window(page.get(), pages)
                .into_iter()
                .map(|item| match item {
                  None => {
                    view! { <span class="join-item btn btn-sm btn-disabled">"…"</span> }
                      .into_any()
                  }
                  Some(p) => {
                    let label = (p + 1).to_string();
                    if p == page.get() {
                      view! { <button class="join-item btn btn-sm btn-active">{label}</button> }
                        .into_any()
                    } else {
                      view! {
                        <button class="join-item btn btn-sm" on:click=move |_| page.set(p)>
                          {label}
                        </button>
                      }
                        .into_any()
                    }
                  }
                })
                .collect_view()
            }}
            <button
              class="join-item btn btn-sm"
              disabled=move || { page.get() + 1 >= pages }
              on:click=move |_| page.update(|page| *page += 1)
            >
              "›"
            </button>
            <button
              class="join-item btn btn-sm"
              disabled=move || { page.get() + 1 >= pages }
              on:click=move |_| page.set(pages - 1)
            >
              "»"
            </button>
          </div>
        </Show>
      </div>
    }
}

#[must_use]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
      <!DOCTYPE html>
      <html lang="en">
        <head>
          <meta charset="utf-8" />
          <meta name="viewport" content="width=device-width, initial-scale=1" />
          <link rel="shortcut icon" type="image/ico" href="/favicon.ico" />
          <AutoReload options=options.clone() />
          <HydrationScripts options islands=true />
          <MetaTags />
        </head>
        <body>
          <App />
        </body>
      </html>
    }
}

#[component]
#[allow(clippy::must_use_candidate)]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
      <Router>
        <Stylesheet id="leptos" href="/pkg/file-share.css" />
        <Title text="File Share" />
        <Routes fallback=|| {
          let mut outside_errors = Errors::default();
          outside_errors.insert_with_default_key(AppError::NotFound);
          view! { <ErrorTemplate outside_errors /> }.into_view()
        }>
          <Route path=path!("/index/*path") view=FilesPage />
        </Routes>
      </Router>
    }
}
