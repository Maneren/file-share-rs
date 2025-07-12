use std::path::PathBuf;

use leptos::prelude::*;

#[component]
pub fn FolderDownloads(path: Signal<PathBuf>) -> impl IntoView {
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
      <div class="dropdown dropdown-hover grow">
        <label tabindex="0" class="btn btn-primary w-full">
          Download Folder
        </label>
        <ul tabindex="0" class="dropdown-content menu p-2 shadow bg-base-100 rounded-box">
          {method_list}
        </ul>
      </div>
    }
}
