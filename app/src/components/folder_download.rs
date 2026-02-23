use std::path::PathBuf;

use leptos::prelude::*;

use crate::utils::display_os_string;

#[component]
pub fn FolderDownloads(path: Signal<PathBuf>) -> impl IntoView {
    let method_list = move || {
        let path = path.with(|path| display_os_string(path));
        ["zip", "tar", "tar.gz", "tar.zst"].map(|method| {
            view! {
              <li>
                <a href=format!("/archive/{path}?method={method}") class="px-3 min-w-20" download>
                  {method}
                </a>
              </li>
            }
        })
    };

    view! {
      <div class="dropdown dropdown-hover grow">
        <label tabindex="0" class="w-full btn btn-primary">
          Download Folder
        </label>
        <ul tabindex="0" class="p-2 shadow dropdown-content menu bg-base-100 rounded-box">
          {method_list}
        </ul>
      </div>
    }
}
