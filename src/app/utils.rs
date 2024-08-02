use std::ffi::OsStr;

pub fn os_to_string(str: impl AsRef<OsStr>) -> String {
  str.as_ref().to_string_lossy().to_string()
}
