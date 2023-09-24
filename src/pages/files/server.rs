use std::path::PathBuf;

use cfg_if::cfg_if;
use leptos::*;
use serde::{Deserialize, Serialize};

use crate::utils::SystemTime;

cfg_if! { if #[cfg(feature = "ssr")] {
  use super::utils::os_to_string;
  use crate::{config::get_target_dir};
}}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntry {
  pub name: String,
  pub size: u64,
  pub last_modified: SystemTime,
}

impl std::cmp::PartialOrd for FileEntry {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl std::cmp::Ord for FileEntry {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.name.cmp(&other.name)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FolderEntry {
  pub name: String,
  pub last_modified: SystemTime,
}

impl std::cmp::PartialOrd for FolderEntry {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl std::cmp::Ord for FolderEntry {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.name.cmp(&other.name)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Entries {
  pub files: Vec<FileEntry>,
  pub folders: Vec<FolderEntry>,
}

#[server]
pub async fn list_dir(path: PathBuf) -> Result<Entries, ServerFnError> {
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

  let mut files = Vec::new();
  let mut folders = Vec::new();

  for entry in std::fs::read_dir(path)? {
    let entry = entry?;

    let path = entry.path();
    let name = os_to_string(path.file_name().unwrap());
    let metadata = entry.metadata()?;
    let last_modified = metadata.modified()?.into();
    if path.is_dir() {
      folders.push(FolderEntry {
        name,
        last_modified,
      });
    } else if path.is_file() {
      files.push(FileEntry {
        name,
        size: metadata.len(),
        last_modified,
      });
    }
  }

  files.sort_unstable();
  folders.sort_unstable();

  Ok(Entries { files, folders })
}

#[server]
pub async fn new_folder(name: String, target: PathBuf) -> Result<(), ServerFnError> {
  let base_path = get_target_dir().join(target);

  std::fs::create_dir(base_path.join(name))?;

  Ok(())
}
