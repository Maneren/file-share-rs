use std::path::PathBuf;

use leptos::{logging::*, *};

use crate::{
  components::{File as FileComponent, Folder as FolderComponent},
  pages::files::{
    server::Entries,
    utils::{format_bytes, get_file_extension, os_to_string},
  },
};

#[component]
pub fn FileEntries(path: Signal<PathBuf>, entries: Entries) -> impl IntoView {
  let Entries { files, folders } = entries;
  log!("Files: {files:?}");
  log!("Folders: {folders:?}");

  if files.is_empty() && folders.is_empty() {
    return view! { <div class="file-view">The folder is empty</div> };
  }

  let folders = move || {
    path.with(|path| {
      folders
        .iter()
        .map(|folder| {
          view! {
            <FolderComponent
              name=&folder.name
              icon="folder"
              target=&os_to_string(path.join(&folder.name))
              last_modified=folder.last_modified
            />
          }
        })
        .collect_view()
    })
  };

  let files = move || {
    path.with(|path| {
      files
        .iter()
        .map(|file| {
          view! {
            <FileComponent
              path=&os_to_string(path.join(&file.name))
              name=&file.name
              extension=&get_file_extension(&file.name)
              size=format_bytes(file.size)
              last_modified=file.last_modified
            />
          }
        })
        .collect_view()
    })
  };

  view! { <div class="file-view">{folders} {files}</div> }
}
