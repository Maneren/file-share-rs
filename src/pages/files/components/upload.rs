use std::path::PathBuf;

use leptos::{ev::SubmitEvent, html::Input, logging::*, *};

#[component]
pub fn FileUpload(path: Signal<PathBuf>) -> impl IntoView {
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
