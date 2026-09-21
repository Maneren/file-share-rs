use std::{ffi::OsStr, path::Path};

use urlencoding::encode;

pub fn display_os_string(str: impl AsRef<OsStr>) -> String {
    str.as_ref().to_string_lossy().into_owned()
}

pub fn encode_path(path: impl AsRef<Path>) -> String {
    path.as_ref()
        .components()
        .map(|component| encode(&display_os_string(component)).into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

#[must_use]
pub fn format_folder_href(base_path: &Path, name: &str) -> String {
    format!("/index/{}", encode_path(base_path.join(name)))
}

#[must_use]
pub fn format_file_href(base_path: &Path, name: &str) -> String {
    format!("/files/{}", encode_path(base_path.join(name)))
}
