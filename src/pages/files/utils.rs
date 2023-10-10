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

#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_precision_loss)]
pub fn format_bytes(bytes: u64) -> String {
  let prefixes = ["", "K", "M", "G", "T", "P", "E", "Z", "Y"];

  if bytes == 0 {
    return "0 B".into();
  }

  let bytes_f64 = bytes as f64;

  // calculate log1024(bytes) and round down
  let power_of_1024 = (bytes_f64.log2() / 10.0).floor() as i32;

  let number = bytes_f64 / 1024f64.powi(power_of_1024);
  let formatted = format!("{number:0.2}");
  let formatted = formatted.trim_end_matches('0').trim_end_matches('.'); // Remove trailing zeros

  let prefix = prefixes[power_of_1024 as usize];

  format!("{formatted} {prefix}B")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  pub fn test_format_bytes() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(1), "1 B");
    assert_eq!(format_bytes(1024), "1 KB");
    assert_eq!(format_bytes(1024 * 1024), "1 MB");
    assert_eq!(format_bytes(1024 * 1024 * 1024), "1 GB");
    assert_eq!(format_bytes(1024 * 1024 * 1024 * 1024), "1 TB");

    assert_eq!(format_bytes(5 * 1024 * 1024), "5 MB");

    assert_eq!(format_bytes(1024 + 256), "1.25 KB");
    assert_eq!(format_bytes(1024 + 100), "1.1 KB");
    assert_eq!(format_bytes(1024 + 1000), "1.98 KB");

    assert_eq!(format_bytes(u64::MAX), "16 EB");
  }
}
