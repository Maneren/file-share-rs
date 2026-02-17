use std::path::PathBuf;

use leptos::{
    ev::SubmitEvent,
    html::{Form, Input},
    prelude::*,
};

#[component]
pub fn UploadForm(
    #[prop(into)] path: Signal<PathBuf>,
    file_ref: NodeRef<Input>,
    form_ref: NodeRef<Form>,
    on_submit: impl Fn(SubmitEvent) + 'static,
) -> impl IntoView {
    view! {
      <form
        class="flex flex-wrap gap-2 grow-2"
        method="POST"
        enctype="multipart/form-data"
        node_ref=form_ref
        on:submit=on_submit
      >
        <input
          type="hidden"
          name="path"
          value=move || path.with(|path| path.to_string_lossy().into_owned())
        />
        // placeholder that is filled on submission
        <input type="hidden" name="id" value="" />
        <input type="file" name="uploads" class="file-input grow-3" multiple node_ref=file_ref />
        <button type="submit" class="btn btn-primary grow-1">
          Upload
        </button>
      </form>
    }
}
