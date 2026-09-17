use std::{path::PathBuf, sync::Arc};

cfg_if! { if #[cfg(feature = "ssr")] {
    use leptos::logging::warn;
    use tokio::fs;

    use crate::config::AppConfig;
}}

use cfg_if::cfg_if;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::utils::{SystemTime, resolve_contained_path};

pub type Entries = Vec<ServerEntry>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Ord, Eq)]
pub enum ServerEntry {
    Folder {
        name: String,
        last_modified: SystemTime,
    },
    File {
        name: String,
        size: u64,
        last_modified: SystemTime,
    },
}

#[server(name = ListDir, prefix = "/api", endpoint = "list_dir")]
pub async fn list_dir(path: PathBuf) -> Result<Entries, ServerFnError> {
    let base_path = expect_context::<Arc<AppConfig>>().target_dir.clone();

    let Some(path) = resolve_contained_path(&base_path, &path).await else {
        warn!("Attempt to access invalid or missing path: {path:?}");
        return Err(ServerFnError::ServerError(
            "Requested path not found".into(),
        ));
    };

    let mut entries = Vec::new();

    let mut directory = fs::read_dir(path).await?;

    while let Some(entry) = directory.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        let metadata = entry.metadata().await?;
        let last_modified = metadata.modified()?.into();

        if metadata.is_dir() {
            entries.push(ServerEntry::Folder {
                name,
                last_modified,
            });
        } else if metadata.is_file() {
            entries.push(ServerEntry::File {
                name,
                size: metadata.len(),
                last_modified,
            });
        }
    }

    entries.sort_unstable();

    Ok(entries)
}

#[server(name = NewFolder, prefix = "/api", endpoint = "new_folder")]
pub async fn new_folder(name: String, path: PathBuf) -> Result<(), ServerFnError> {
    use crate::utils::{is_safe_file_name, is_safe_relative_path};

    let app_config = expect_context::<Arc<AppConfig>>();

    if !app_config.allow_upload {
        return Err(ServerFnError::ServerError("Uploads are disabled".into()));
    }

    if !is_safe_relative_path(&path) || !is_safe_file_name(&name) {
        return Err(ServerFnError::ServerError("Invalid path or name".into()));
    }

    // Resolve the parent through the real filesystem so a symlinked
    // directory cannot redirect the new folder outside the share.
    // `name` is a single normal component, so joining it cannot escape.
    let Some(parent) = resolve_contained_path(&app_config.target_dir, &path).await
    else {
        return Err(ServerFnError::ServerError("Invalid path".into()));
    };

    fs::create_dir(parent.join(&name)).await?;

    Ok(())
}
