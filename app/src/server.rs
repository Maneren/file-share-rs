use std::path::PathBuf;
#[cfg(feature = "ssr")]
use std::sync::Arc;

cfg_if! { if #[cfg(feature = "ssr")] {
    use leptos::logging::warn;
    use tokio::fs;

    use crate::{
        config::{AppConfig, PATH_NOT_FOUND_MESSAGE, UPLOAD_DISABLED_MESSAGE},
        utils::resolve_contained_path,
    };
}}

use cfg_if::cfg_if;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::utils::SystemTime;

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
    fn read_dir_error(path: &PathBuf, e: impl std::fmt::Display) -> ServerFnError {
        warn!("Failed to read directory {path:?}: {e}");
        ServerFnError::ServerError("Failed to read directory".into())
    }

    let base_path = expect_context::<Arc<AppConfig>>().target_dir.clone();

    let Some(path) = resolve_contained_path(&base_path, &path).await else {
        warn!("Attempt to access invalid or missing path: {path:?}");
        return Err(ServerFnError::ServerError(PATH_NOT_FOUND_MESSAGE.into()));
    };

    let mut entries = Vec::new();

    let mut directory = fs::read_dir(&path)
        .await
        .map_err(|e| read_dir_error(&path, e))?;

    while let Some(entry) = directory
        .next_entry()
        .await
        .map_err(|e| read_dir_error(&path, e))?
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        // One unreadable entry must not fail the whole listing, and its
        // OS error stays server-side.
        let Ok(metadata) = entry.metadata().await else {
            warn!("Skipping {path:?}/{name}: cannot read metadata");
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            warn!("Skipping {path:?}/{name}: cannot read modification time");
            continue;
        };
        let last_modified = modified.into();

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

    fn create_dir_error(path: &PathBuf, name: &str, e: impl std::fmt::Display) -> ServerFnError {
        warn!("Failed to create folder {path:?}/{name}: {e}");
        ServerFnError::ServerError("Failed to create folder".into())
    }

    let app_config = expect_context::<Arc<AppConfig>>();

    if !app_config.allow_upload {
        return Err(ServerFnError::ServerError(UPLOAD_DISABLED_MESSAGE.into()));
    }

    if !is_safe_relative_path(&path) || !is_safe_file_name(&name) {
        return Err(ServerFnError::ServerError("Invalid path or name".into()));
    }

    // Resolve the parent through the real filesystem so a symlinked
    // directory cannot redirect the new folder outside the share.
    // `name` is a single normal component, so joining it cannot escape.
    let Some(parent) = resolve_contained_path(&app_config.target_dir, &path).await else {
        return Err(ServerFnError::ServerError("Invalid path".into()));
    };

    fs::create_dir(parent.join(&name))
        .await
        .map_err(|e| create_dir_error(&path, &name, e))?;

    Ok(())
}
