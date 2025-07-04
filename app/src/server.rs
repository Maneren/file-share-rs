use std::path::PathBuf;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use tokio::fs;

use crate::utils::SystemTime;

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

    let base_path = expect_context::<PathBuf>().clone();

    let Ok(path) = base_path.join(path).canonicalize() else {
        return Err(ServerFnError::ServerError(
            "Path must be inside target_dir".into(),
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
    let base_path = expect_context::<PathBuf>().clone();

    let path = base_path.join(path).join(name);

    fs::create_dir(path).await?;

    Ok(())
}
