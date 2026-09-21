use std::{path::PathBuf, sync::Arc};

use leptos::prelude::*;

use crate::{archive::Method, utils::display_os_string};

fn method_url_query(path: &String, method: &str) -> String {
    format!("{path}?method={method}")
}

#[component]
pub fn FolderDownloads(path: Signal<PathBuf>) -> impl IntoView {
    let base_path = Arc::new(format!(
        "/archive/{}",
        path.with(|path| display_os_string(path))
    ));
    let curl_list_path = Arc::clone(&base_path);

    let method_list = move || {
        let base_path = &*base_path;
        Method::ALL.map(|method| {
            let label = method.as_str();
            view! {
                <li>
                    <a href=method_url_query(base_path, label) class="px-3 min-w-20" download>
                        {label}
                    </a>
                </li>
            }
        })
    };

    let fast_lan_list = move || {
        let curl_list_path = &*curl_list_path;
        Method::ALL
            .iter()
            .filter_map(|method| {
                let label = method.as_str();
                let curl_url = method_url_query(curl_list_path, label);
                let flags = method.tar_extract_flags()?;
                let make_command = |host| format!("curl -L \'{host}{curl_url}\' | tar x {flags}");
                let view = view! {
                    <li>
                        <button
                            class="px-3 min-w-20 text-left"
                            title=format!(
                                "Copies: {command}",
                                command = make_command("<host>").trim(),
                            )
                            data-url=curl_url.clone()
                            onclick=format!(
                                "navigator.clipboard.writeText(`{command}`)",
                                command = make_command("${window.location.origin}").trim(),
                            )
                        >
                            Copy curl |
                            {label}
                        </button>
                    </li>
                };
                Some(view)
            })
            .collect::<Vec<_>>()
    };

    view! {
        <div class="dropdown dropdown-hover grow">
            <label tabindex="0" class="w-full btn btn-primary">
                Download Folder
            </label>
            <ul tabindex="0" class="p-2 shadow dropdown-content menu bg-base-100 rounded-box">
                <li class="menu-title">Archive</li>
                {method_list}
                <li class="menu-title">Curl-stream</li>
                {fast_lan_list}
            </ul>
        </div>
    }
}
