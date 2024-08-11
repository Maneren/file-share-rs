use std::time::{self, UNIX_EPOCH};

use chrono::{DateTime, TimeZone, Utc};
use chrono_humanize::Humanize;
use leptos::{IntoView, View};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct SystemTime(pub i64, pub u32);
impl From<time::SystemTime> for SystemTime {
  #[allow(clippy::similar_names)]
  #[allow(clippy::cast_possible_wrap)]
  fn from(time: time::SystemTime) -> Self {
    let (sec, nsec) = match time.duration_since(UNIX_EPOCH) {
      Ok(dur) => (dur.as_secs() as i64, dur.subsec_nanos()),
      Err(e) => {
        // unlikely but should be handled
        let dur = e.duration();
        let (sec, nsec) = (dur.as_secs() as i64, dur.subsec_nanos());
        if nsec == 0 {
          (-sec, 0)
        } else {
          (-sec - 1, 1_000_000_000 - nsec)
        }
      }
    };
    Self(sec, nsec)
  }
}
impl From<SystemTime> for DateTime<Utc> {
  #[allow(clippy::similar_names)]
  fn from(time: SystemTime) -> Self {
    let SystemTime(sec, nsec) = time;
    Utc.timestamp_opt(sec, nsec).unwrap() // per docs, Utc can't fail
  }
}
impl IntoView for SystemTime {
  fn into_view(self) -> View {
    DateTime::from(self)
      .format("%Y-%m-%d %H:%M:%S")
      .to_string()
      .into_view()
  }
}
impl SystemTime {
  pub fn humanize(&self) -> String {
    DateTime::from(*self).humanize()
  }
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
