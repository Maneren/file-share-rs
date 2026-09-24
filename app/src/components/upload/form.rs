use leptos::{
    ev::SubmitEvent,
    html::{Form, Input},
    prelude::*,
};

/// Native file form. Posts to the `/upload` endpoint directly, so it also
/// works without JS; the island intercepts the submit for progress.
#[component]
pub fn UploadForm(
    action: String,
    file_ref: NodeRef<Input>,
    form_ref: NodeRef<Form>,
    on_submit: impl Fn(SubmitEvent) + 'static,
) -> impl IntoView {
    view! {
        <form
            class="flex flex-row gap-2 grow-[2]"
            action=action
            method="POST"
            enctype="multipart/form-data"
            node_ref=form_ref
            on:submit=on_submit
        >
            <input
                type="file"
                name="files"
                class="file-input grow-[3]"
                multiple
                node_ref=file_ref
            />
            <button type="submit" class="btn btn-primary grow">
                Upload
            </button>
        </form>
    }
}
