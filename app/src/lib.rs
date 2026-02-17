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
use leptos_meta::{Link, Meta, MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    components::{Route, Router, Routes},
    hooks::use_params_map,
};
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

#[component]
pub fn FilesPage() -> impl IntoView {
    let path_query = use_params_map();

    let path = Memo::new(
        move |_| match path_query.read().get_str("path").map(decode) {
            Some(Ok(path)) => PathBuf::from(path.as_ref()),
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
      <div class="p-3 App">
        <UploadBar
          path=path_signal
          create_folder_action=create_folder_action
          on_upload=move || listing.refetch()
        />

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

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
      <Stylesheet id="leptos" href="/pkg/file-share.css" />
      <Link rel="shortcut icon" type_="image/ico" href="/favicon.ico" />
      <Meta charset="utf-8" />
      <Meta name="viewport" content="width=device-width, initial-scale=1" />
      <Title text="File Share" />

      <Router>
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
