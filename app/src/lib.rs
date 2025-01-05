#![allow(non_snake_case)]
#![feature(duration_millis_float)]

use std::path::PathBuf;

mod components;
mod error_template;
mod server;
#[cfg(feature = "ssr")]
mod state;
pub mod utils;

use leptos::{either::Either, prelude::*};
use leptos_meta::*;
use leptos_router::{components::*, hooks::use_params, params::*};
use leptos_router_macro::path;
use urlencoding::decode;

#[cfg(feature = "ssr")]
pub use crate::state::AppState;
use crate::{
  components::*,
  error_template::{AppError, ErrorTemplate},
  server::*,
};

#[derive(PartialEq, Eq, Params, Debug)]
struct PathQuery {
  path: String,
}

#[component]
pub fn FilesPage() -> impl IntoView {
  let path_query = use_params::<PathQuery>();

  let path =
    Memo::new(
      move |_| match path_query.read().as_ref().map(|query| decode(&query.path)) {
        Ok(Ok(path)) => PathBuf::from(path.as_ref()),
        _ => PathBuf::new(),
      },
    );

  let create_folder_action = ServerAction::<NewFolder>::new();

  let listing = Resource::new(
    move || (path.get(), create_folder_action.version().get()),
    |(path, ..)| list_dir(path),
  );

  let path_signal = Signal::from(path);

  view! {
    <div class="App p-3">
      <div class="w-full pt-2 flex flex-wrap items-start justify-center gap-2">
        <FileUpload path=path_signal on_upload=move || listing.refetch() />
        <div class="flex flex-grow gap-2">
          <NewFolderButton path=path_signal action=create_folder_action />
          <FolderDownloads path=path_signal />
        </div>
      </div>

      <Breadcrumbs path=path_signal />

      <div class="grid grid-cols-entry-mobile md:grid-cols-entry gap-2 border-b border-base-content mb-1">
        <span></span>
        <span>Name</span>
        <span>Size</span>
        <span class="hidden md:inline">Last Modified</span>
      </div>

      <Transition fallback=move || {
        view! { <p>"Loading..."</p> }
      }>
        {move || Suspend::new(async move {
          match listing.await {
            Ok(entries) => {
              Either::Left(view! { <FileEntries path=path_signal entries=entries.clone() /> })
            }
            Err(e) => Either::Right(view! { <p class="text-lg">{format!("{e}")}</p> }),
          }
        })}
      </Transition>
    </div>
  }
}

pub fn shell(options: LeptosOptions) -> impl IntoView {
  view! {
    <!DOCTYPE html>
    <html lang="en">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <link rel="shortcut icon" type="image/ico" href="/favicon.ico" />
        <AutoReload options=options.clone() />
        <HydrationScripts options />
        <MetaTags />
      </head>
      <body>
        <App />
      </body>
    </html>
  }
}

#[allow(non_snake_case)]
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
