use std::path::PathBuf;

use leptos::prelude::*;

use crate::components::{FileUpload, FolderDownloads, NewFolderButton};

#[component]
pub fn UploadBar(
    #[prop(into)] path: Signal<PathBuf>,
    create_folder_action: ServerAction<crate::server::NewFolder>,
) -> impl IntoView {
    view! {
      <div class="flex flex-wrap gap-2 justify-center items-start py-2 w-full">
        <FileUpload path=path() />
        <div class="flex gap-2 grow">
          <NewFolderButton path=path action=create_folder_action />
          <FolderDownloads path=path />
        </div>
      </div>
    }
}
