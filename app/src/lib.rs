#![allow(non_snake_case)]
#![feature(duration_millis_float)]

use std::path::PathBuf;

mod components;
mod config;
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

pub use crate::config::AppConfig;
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

    let app_config = expect_context::<AppConfig>();

    let upload_bar = app_config.allow_upload.then(|| {
        view! {
          <div class="flex flex-wrap gap-2 justify-center items-start pt-2 w-full">
            <FileUpload path=path_signal() />
            <div class="flex gap-2 grow">
              <NewFolderButton path=path_signal action=create_folder_action />
              <FolderDownloads path=path_signal />
            </div>
          </div>
        }
    });

    view! {
      <div class="p-3 App">
        {upload_bar}
        <Breadcrumbs path=path_signal />
        <div class="grid gap-2 mb-1 border-b grid-cols-(--entry-cols-mobile) border-base-content md:grid-cols-(--entry-cols)">
          <span></span>
          <span>Name</span>
          <span>Size</span>
          <span class="hidden md:inline">Last Modified</span>
        </div>
        <Transition fallback=Loading>
          {move || Suspend::new(async move {
            match listing.await {
              Ok(entries) => {
                Either::Left(view! { <FileEntries path=path_signal entries=entries /> })
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
