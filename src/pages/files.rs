use std::{iter, path::PathBuf};

mod server;
mod utils;

use leptos::{ev::SubmitEvent, html::Input, logging::*, *};
use leptos_router::*;
use server::*;
use utils::*;

use crate::components::{File as FileComponent, Folder as FolderComponent};

#[derive(PartialEq, Eq, Params)]
struct PathQuery {
  path: String,
}

pub fn FilesPage() -> impl IntoView {
  let path_query = use_query::<PathQuery>();

  let path = create_memo(move |_| {
    path_query.with(|query| {
      query
        .as_ref()
        .map_or_else(|_| PathBuf::new(), |query| PathBuf::from(&query.path))
    })
  });

  let entries = create_resource(path, |path| async {
    list_dir(path).await.unwrap_or_default()
  });

  let path = Signal::from(path);

  let entries = move || {
    entries
      .get()
      .map(|entries| view! { <FileEntries path=path entries=entries/> })
  };

  view! {
    <div class="App p-3">
      <div class="w-full pt-2 flex flex-wrap items-center justify-center gap-2">
        <FileUpload path=path/>
        <div class="flex flex-grow gap-2">
          <NewFolderButton path=path/>
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

#[component]
fn FileUpload(path: Signal<PathBuf>) -> impl IntoView {
  let upload_input: NodeRef<Input> = create_node_ref();
  let upload_path = move || path.with(|path| format!("/upload/{}", path.display()));

  let on_file_upload_submit = move |ev: SubmitEvent| {
    let files = upload_input().unwrap().files().unwrap();
    if files.length() == 0 {
      ev.prevent_default();
    }
  };

  view! {
    <form
      class="flex flex-wrap grow-[2] gap-2"
      action=upload_path
      method="POST"
      enctype="multipart/form-data"
      on:submit=on_file_upload_submit
    >
      <input
        type="file"
        name="uploads"
        class="file-input file-input-bordered grow-[3]"
        multiple
        node_ref=upload_input
      />
      <button type="submit" class="btn btn-primary grow-[1]">
        Upload
      </button>
    </form>
  }
}

#[component]
fn NewFolderButton(path: Signal<PathBuf>) -> impl IntoView {
  let new_folder_input: NodeRef<Input> = create_node_ref();

  let new_folder = create_action(move |name: &String| {
    let name = name.clone();
    async move {
      new_folder(name, path.get_untracked()).await.unwrap();
      window().location().reload().unwrap();
    }
  });

  let on_new_folder_submit = move |ev: SubmitEvent| {
    ev.prevent_default();
    let name = new_folder_input().unwrap().value();
    new_folder.dispatch(name);
  };

  view! {
    <div class="flex-grow">
      <button class="btn btn-primary w-full" onclick="new_folder_modal.showModal()">
        Create New Folder
      </button>
      <dialog id="new_folder_modal" class="modal">
        <form
          method="dialog"
          class="modal-box"
          on:submit=on_new_folder_submit
          onsubmit="new_folder_modal.close()"
        >
          <h3 class="font-bold text-lg">New Folder</h3>
          <input
            class="input input-bordered w-full max-w-xs py-2 my-2"
            type="text"
            value="New Folder"
            node_ref=new_folder_input
          />
          <div class="modal-action">
            <button class="btn">Cancel</button>
          </div>
        </form>
        <form method="dialog" class="modal-backdrop">
          <button></button>
        </form>
      </dialog>
    </div>
  }
}

#[component]
fn FolderDownloads(path: Signal<PathBuf>) -> impl IntoView {
  let method_list = move || {
    let display_path = path.with(|path| path.display().to_string());
    ["zip", "tar", "tar.gz", "tar.zst"].map(|method| {
      view! {
        <li>
          <a href=format!("/archive/{display_path}?method={method}") class="px-3 min-w-20" download>
            {method}
          </a>
        </li>
      }
    })
  };

  view! {
    <div class="dropdown dropdown-hover flex-grow">
      <label tabindex="0" class="btn btn-primary w-full">
        Download Folder
      </label>
      <ul tabindex="0" class="dropdown-content menu p-2 shadow bg-base-100 rounded-box">
        {method_list}
      </ul>
    </div>
  }
}

#[component]
fn Breadcrumbs(path: Signal<PathBuf>) -> impl IntoView {
  let breadcrumbs = move || {
    path.with(|path| {
      let leading_breadcrumb = ("/".into(), "?path=".into());

      let path_breadcrumbs = path.iter().scan(PathBuf::new(), |path, part| {
        let part_str = os_to_string(part);
        path.push(part);
        let path = format!("?path={}", path.display());

        Some((part_str, path))
      });

      iter::once(leading_breadcrumb)
        .chain(path_breadcrumbs)
        .map(|(name, path)| {
          view! {
            <li>
              <a href=path>{name}</a>
            </li>
          }
        })
        .collect_view()
    })
  };

  view! {
    <div class="text-lg breadcrumbs max-w-full">
      <ul>{breadcrumbs}</ul>
    </div>
  }
}

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
