use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use leptos_use::use_preferred_dark;

use crate::{
  error_template::{AppError, ErrorTemplate},
  pages::FilesPage,
};

#[allow(non_snake_case)]
pub fn App() -> impl IntoView {
  provide_meta_context();

  let is_dark_preferred = use_preferred_dark();

  let theme = move || {
    if is_dark_preferred() {
      "night"
    } else {
      "corporate"
    }
  };

  let theme_attribute = vec![("data-theme", theme.into_attribute())];

  view! {
    <Html lang="en" attributes=theme_attribute/>
    <Stylesheet id="leptos" href="/pkg/file-share.css"/>

    <Title text="File Sharing"/>

    <Router fallback=|| {
        let mut outside_errors = Errors::default();
        outside_errors.insert_with_default_key(AppError::NotFound);
        view! { <ErrorTemplate outside_errors/> }.into_view()
    }>
      <Routes>
        <Route path="/index/*path" view=FilesPage/>
      </Routes>
    </Router>
  }
}
