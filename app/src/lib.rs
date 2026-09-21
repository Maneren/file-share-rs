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

use leptos::prelude::*;
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
    components::{Breadcrumbs, ListingBrowser, UploadBar},
    error_template::{AppError, ErrorTemplate},
    server::NewFolder,
};

#[derive(PartialEq, Eq, Params, Debug)]
struct PathQuery {
    path: String,
}

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

    let path_signal = Signal::from(path);

    let app_config = expect_context::<Arc<AppConfig>>();

    let upload_bar = app_config.allow_upload.then(|| {
        view! { <UploadBar path=path_signal create_folder_action=create_folder_action /> }
    });

    view! {
        <div class="p-3 App">
            {upload_bar} <Breadcrumbs path=path_signal />
            // Snapshot path: the island remounts fresh on every navigation.
            <ListingBrowser path=path.get_untracked() allow_upload=app_config.allow_upload />
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
