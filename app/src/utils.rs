use std::{
    borrow::Cow,
    path::{Component, PathBuf},
    time::{self, UNIX_EPOCH},
};

use chrono::{DateTime, TimeZone, Utc};
use chrono_humanize::Humanize;
use leptos::prelude::IntoRender;
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
            },
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
impl IntoRender for SystemTime {
    type Output = String;

    fn into_render(self) -> Self::Output {
        DateTime::from(self).format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }
}
impl SystemTime {
    #[must_use]
    pub fn humanize(&self) -> String {
        DateTime::from(*self).humanize()
    }
}

use std::{ffi::OsStr, path::Path};

pub fn display_os_string(str: impl AsRef<OsStr>) -> String {
    str.as_ref().to_string_lossy().into_owned()
}

pub fn encode_path(path: impl AsRef<OsStr>) -> String {
    urlencoding::encode(path.as_ref().to_string_lossy().as_ref()).into_owned()
}

#[must_use]
pub fn try_decode_path(path: &str) -> PathBuf {
    urlencoding::decode(path)
        .map(Cow::into_owned)
        .unwrap_or_else(|_| path.to_owned())
        .into()
}
#[must_use]
pub fn format_folder_href(base_path: &Path, name: &str) -> String {
    format!("/index/{}", encode_path(base_path.join(name)))
}

#[must_use]
pub fn format_file_href(base_path: &Path, name: &str) -> String {
    format!("/files/{}", encode_path(base_path.join(name)))
}

pub fn is_safe_relative_path(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|comp| matches!(comp, Component::Normal(_) | Component::CurDir))
}

pub fn is_safe_file_name(name: impl AsRef<Path>) -> bool {
    let mut components = name.as_ref().components();

    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    const PREFIXES: [&str; 9] = ["", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "Zi", "Yi"];

    if bytes < 1024 {
        return format!("{bytes} B");
    }

    let bytes_f64 = bytes as f64;

    // calculate log1024(bytes) and round down
    let power_of_1024 = (bytes_f64.log2() / 10.0).floor() as i32;

    let number = bytes_f64 / 1024f64.powi(power_of_1024);
    let formatted = format!("{number:0.2}");
    let formatted = formatted.trim_end_matches('0').trim_end_matches('.'); // Remove trailing zeros

    let prefix = PREFIXES[power_of_1024 as usize];

    format!("{formatted} {prefix}B")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1), "1 B");
        assert_eq!(format_bytes(1024), "1 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1 MiB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1 GiB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 1024), "1 TiB");

        assert_eq!(format_bytes(5 * 1024 * 1024), "5 MiB");

        assert_eq!(format_bytes(1024 + 256), "1.25 KiB");
        assert_eq!(format_bytes(1024 + 100), "1.1 KiB");
        assert_eq!(format_bytes(1024 + 1000), "1.98 KiB");

        assert_eq!(format_bytes(u64::MAX), "16 EiB");
    }
}
