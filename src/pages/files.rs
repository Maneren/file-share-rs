use std::path::PathBuf;

mod components;
mod server;
mod utils;

use components::*;
use leptos::*;
use leptos_router::*;
use server::*;
use urlencoding::decode;

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

  let entries = create_resource(
    move || (path.get(), create_folder_action.version().get()),
    |(path, ..)| list_dir(path),
  );

  let path = Signal::from(path);

  let entries = move || {
    entries.with(|entries| {
      entries.as_ref().map(|entries| match entries {
        Ok(entries) => view! { <FileEntries path=path entries=entries.clone()/> }.into_view(),
        Err(e) => view! { <p class="text-lg">{format!("{e}")}</p> }.into_view(),
      })
    })
  };

  view! {
    <div class="App p-3">
      <div class="w-full pt-2 flex flex-wrap items-center justify-center gap-2">
        <FileUpload path=path/>
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

      <Suspense fallback=move || {
          view! { <p>"Loading..."</p> }
      }>{entries}</Suspense>
    </div>
  }
}
