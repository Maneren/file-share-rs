//! Leptos root component and HTML shell.

use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router_macro::path;

use crate::pages::{AppError, ErrorTemplate, FilesPage, LoginPage};

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
                <Route path=path!("/login") view=LoginPage />
            </Routes>
        </Router>
    }
}
