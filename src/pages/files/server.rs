use std::path::PathBuf;

use leptos::*;
use serde::{Deserialize, Serialize};

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

#[server(ListDir, "/api", "Url", "list_dir")]
pub async fn list_dir(path: PathBuf) -> Result<Entries, ServerFnError> {
  use crate::config::get_target_dir;

  if path.is_absolute() {
    return Err(ServerFnError::ServerError("Path must be relative".into()));
  }

  if path.as_os_str().to_string_lossy().contains("..") {
    return Err(ServerFnError::ServerError(
      "Path must not contain ..".into(),
    ));
  }

  let Ok(path) = get_target_dir().join(path).canonicalize() else {
    return Err(ServerFnError::ServerError(
      "Path must be inside target_dir".into(),
    ));
  };

  let mut entries = Vec::new();

  for entry in std::fs::read_dir(path)? {
    let entry = entry?;

    let name = entry.file_name().to_string_lossy().into_owned();
    let metadata = entry.metadata()?;
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

#[server(NewFolder, "/api", "Url", "new_folder")]
pub async fn new_folder(name: String, path: PathBuf) -> Result<(), ServerFnError> {
  use crate::config::get_target_dir;

  let path = get_target_dir().join(path).join(name);

  std::fs::create_dir(path)?;

  Ok(())
}
