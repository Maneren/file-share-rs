use std::path::PathBuf;

use leptos::{html::Input, logging, prelude::*};

use crate::{api::NewFolder, paths::display_os_string};

#[component]
pub fn NewFolderButton(path: Signal<PathBuf>, action: ServerAction<NewFolder>) -> impl IntoView {
    let new_folder_input = NodeRef::<Input>::new();

    let on_new_folder_focus = move |_| {
        let input = new_folder_input
            .get()
            .expect("input is mounted with its dialog");
        let input_length =
            u32::try_from(input.value().len()).expect("New folder name is shorter than u32::MAX");
        if input.set_selection_range(0, input_length).is_err() {
            logging::warn!("Couldn't select new folder name");
        }
    };

    view! {
        <div class="grow">
            <button class="w-full btn btn-primary" onclick="new_folder_modal.showModal()">
                Create New Folder
            </button>
            <dialog id="new_folder_modal" class="modal">
                <ActionForm action=action>
                    <div class="modal-box">
                        <h3 class="text-lg font-bold">New Folder</h3>
                        <input
                            class="py-2 my-2 input"
                            type="text"
                            value="New Folder"
                            on:focus=on_new_folder_focus
                            node_ref=new_folder_input
                            name="name"
                            autofocus
                        />
                        <input
                            type="hidden"
                            name="path"
                            value=move || path.with(|path| display_os_string(path))
                        />
                        <div class="modal-action">
                            <button class="btn" type="reset" onclick="new_folder_modal.close()">
                                Cancel
                            </button>
                            <button
                                class="btn btn-primary"
                                type="submit"
                                onclick="new_folder_modal.close()"
                            >
                                Create
                            </button>
                        </div>
                    </div>
                </ActionForm>
                <form method="dialog" class="modal-backdrop">
                    <button></button>
                </form>
            </dialog>
        </div>
    }
}
