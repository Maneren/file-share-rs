#![allow(non_snake_case)]

use std::path::PathBuf;

pub(crate) mod components;
pub(crate) mod server;
pub(crate) mod utils;

use components::*;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use leptos_use::use_preferred_dark;
use server::*;
use urlencoding::decode;

use crate::error_template::{AppError, ErrorTemplate};

#[derive(PartialEq, Eq, Params, Debug)]
struct PathQuery {
  path: String,
}

pub fn FilesPage() -> impl IntoView {
  let path_query = use_params::<PathQuery>();

  let path = create_memo(move |_| {
    path_query.with(
      |query| match query.as_ref().map(|query| decode(&query.path)) {
        Ok(Ok(path)) => PathBuf::from(path.as_ref()),
        _ => PathBuf::new(),
      },
    )
  });

  let create_folder_action = create_server_action::<NewFolder>();

  let entries_resource = create_resource(
    move || (path.get(), create_folder_action.version().get()),
    |(path, ..)| list_dir(path),
  );

  let path = Signal::from(path);

  let entries = move || {
    entries_resource.with(|entries| {
      entries.as_ref().map(|entries| match entries {
        Ok(entries) => view! { <FileEntries path=path entries=entries.clone()/> }.into_view(),
        Err(e) => view! { <p class="text-lg">{format!("{e}")}</p> }.into_view(),
      })
    })
  };

  view! {
    <div class="App p-3">
      <div class="w-full pt-2 flex flex-wrap items-start justify-center gap-2">
        <FileUpload path=path on_upload=move |()| entries_resource.refetch()/>
        <div class="flex flex-grow gap-2">
          <NewFolderButton path=path action=create_folder_action/>
          <FolderDownloads path=path/>
        </div>
      </div>

      <Breadcrumbs path=path/>

      <div class="grid grid-cols-entry-mobile md:grid-cols-entry gap-2 border-b border-base-content mb-1">
        <span></span>
        <span>Name</span>
        <span>Size</span>
        <span class="hidden md:inline">Last Modified</span>
      </div>

      <Suspense fallback=move || view! { <p>"Loading..."</p> }>{entries}</Suspense>
    </div>
  }
}

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
