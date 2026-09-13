use std::{path::PathBuf, sync::Arc};

use leptos::prelude::*;

use crate::utils::display_os_string;

const METHODS: [&str; 4] = ["zip", "tar", "tar.gz", "tar.zst"];

fn method_url_query(path: &String, method: &str) -> String {
    format!("{path}?method={method}")
}

#[component]
pub fn FolderDownloads(path: Signal<PathBuf>) -> impl IntoView {
    let base_name = path.with(|path| display_os_string(path.file_name().unwrap_or_default()));
    let base_path = Arc::new(format!(
        "/archive/{}",
        path.with(|path| display_os_string(path))
    ));
    let curl_list_path = Arc::clone(&base_path);

    let method_list = move || {
        let base_path = &*base_path;
        METHODS.map(|method| {
            view! {
              <li>
                <a href=method_url_query(base_path, method) class="px-3 min-w-20" download>
                  {method}
                </a>
              </li>
            }
        })
    };

    let fast_lan_list = move || {
        let curl_list_path = &*curl_list_path;
        METHODS.map(|method| {
            let curl_url = method_url_query(curl_list_path, method);
            view! {
              <li>
                <button
                  class="px-3 min-w-20 text-left"
                  title=format!("Copies: curl -L <this server>{curl_url} | tar --zstd -xvC ./{base_name}")
                  data-url=curl_url.clone()
                  onclick=format!(
                    "navigator.clipboard.writeText(`curl -L \"${{window.location.origin}}{curl_url}\" | tar --zstd -xvC ./{base_name}`)",
                  )
                >
                  Copy curl | {method}
                </button>
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
          <li class="menu-title">Fast LAN</li>
          {fast_lan_list}
        </ul>
      </div>
    }
}
