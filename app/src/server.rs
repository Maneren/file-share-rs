use std::path::PathBuf;

cfg_if! { if #[cfg(feature = "ssr")] {
    use leptos::logging::warn;
    use tokio::fs;

    use crate::config::AppConfig;
}}

use cfg_if::cfg_if;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::utils::SystemTime;

#[server(name = UploadAllowed, prefix = "/api", endpoint = "upload_allowed")]
pub async fn upload_allowed() -> Result<bool, ServerFnError> {
    Ok(expect_context::<AppConfig>().allow_upload)
}

pub type Entries = Vec<ServerEntry>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Ord, Eq)]
pub enum ServerEntry {
    File {
        name: String,
        size: u64,
        last_modified: SystemTime,
    },
    Folder {
        name: String,
        last_modified: SystemTime,
    },
}

#[server(name = ListDir, prefix = "/api", endpoint = "list_dir")]
pub async fn list_dir(path: PathBuf) -> Result<Entries, ServerFnError> {
    if path.is_absolute() {
        return Err(ServerFnError::ServerError("Path must be relative".into()));
    }

    if path.to_string_lossy().contains("..") {
        return Err(ServerFnError::ServerError(
            "Path must not contain ..".into(),
        ));
    }

    let base_path = expect_context::<AppConfig>().target_dir;

    let Ok(path) = base_path.join(&path).canonicalize() else {
        warn!("Attempt to access invalid path: {path:?}");
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

    Ok(entries)
}

#[server(name = NewFolder, prefix = "/api", endpoint = "new_folder")]
pub async fn new_folder(name: String, path: PathBuf) -> Result<(), ServerFnError> {
    let app_config = expect_context::<AppConfig>();

    if !app_config.allow_upload {
        return Err(ServerFnError::ServerError("Uploads are disabled".into()));
    }

    let path = app_config.target_dir.join(path).join(name);

    fs::create_dir(path).await?;

    Ok(())
}
