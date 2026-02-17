use std::path::PathBuf;

use leptos::prelude::*;

use crate::{
    components::{FileUpload, FolderDownloads, NewFolderButton},
    server::upload_allowed,
};

#[component]
pub fn UploadBar(
    #[prop(into)] path: Signal<PathBuf>,
    create_folder_action: ServerAction<crate::server::NewFolder>,
    #[prop(into)] on_upload: Callback<()>,
) -> impl IntoView {
    view! {
      <Transition fallback=|| {
        view! { <div class="h-16"></div> }
      }>
        {move || {
          Suspend::new(async move {
            upload_allowed()
              .await
              .ok()
              .and_then(|allow_upload| {
                allow_upload
                  .then(|| {
                    view! {
                      <div class="flex flex-wrap gap-2 justify-center items-start py-2 w-full">
                        <FileUpload path=path on_upload=on_upload />
                        <div class="flex gap-2 grow">
                          <NewFolderButton path=path action=create_folder_action />
                          <FolderDownloads path=path />
                        </div>
                      </div>
                    }
                  })
              })
          })
        }}
      </Transition>
    }
}
