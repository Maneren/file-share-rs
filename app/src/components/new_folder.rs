use std::path::PathBuf;

use leptos::{html::Input, prelude::*};

use crate::{server::NewFolder, utils::os_to_string};

#[component]
pub fn NewFolderButton(path: Signal<PathBuf>, action: ServerAction<NewFolder>) -> impl IntoView {
    let new_folder_input = NodeRef::<Input>::new();

    let on_new_folder_focus = move |_| {
        let input = new_folder_input.get().unwrap();
        let input_length =
            u32::try_from(input.value().len()).expect("New folder name is shorter than u32::MAX");
        let _ = input.set_selection_range(0, input_length);
    };

    view! {
      <div class="flex-grow">
        <button class="btn btn-primary w-full" onclick="new_folder_modal.showModal()">
          Create New Folder
        </button>
        <dialog id="new_folder_modal" class="modal">
          <ActionForm action=action>
            <div class="modal-box">
              <h3 class="font-bold text-lg">New Folder</h3>
              <input
                class="input input-bordered w-full max-w-xs py-2 my-2"
                type="text"
                value="New Folder"
                on:focus=on_new_folder_focus
                node_ref=new_folder_input
                name="name"
                autofocus
              />
              <input type="hidden" name="path" value=move || os_to_string(path.read().as_os_str()) />
              <div class="modal-action">
                <button class="btn" type="reset" onclick="new_folder_modal.close()">
                  Cancel
                </button>
                <button class="btn btn-primary" type="submit" onclick="new_folder_modal.close()">
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
