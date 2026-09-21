//! Top action bar: upload form + new-folder button + folder downloads.
//!
//! Renamed from `upload_bar` — only one of three children is an upload.

use std::path::PathBuf;

use leptos::prelude::*;

use crate::components::{FileUpload, FolderDownloads, NewFolderButton};

#[component]
pub fn ActionBar(
    #[prop(into)] path: Signal<PathBuf>,
    create_folder_action: ServerAction<crate::api::NewFolder>,
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
