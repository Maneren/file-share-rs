use std::{ffi::OsStr, path::Path};

pub fn os_to_string(str: impl AsRef<OsStr>) -> String {
  str.as_ref().to_string_lossy().to_string()
}

pub fn get_file_extension(path: impl AsRef<OsStr>) -> String {
  Path::new(path.as_ref())
    .extension()
    .map(os_to_string)
    .unwrap_or_default()
}
