//! Backwards-compatible re-exports.
//!
//! The helpers formerly living here now have coherent homes:
//! - [`crate::time`] — `SystemTime` wrapper
//! - [`crate::paths`] — path/URL helpers
//! - [`crate::fs_guard`] — containment guards
//! - [`crate::format`] — `format_bytes`
//! - [`crate::pagination`] — `page_window`
//!
//! This module re-exports them so existing `crate::utils::*` imports keep
//! working while call sites migrate.

#[cfg(feature = "ssr")]
pub use crate::fs_guard::resolve_contained_path;
pub use crate::{
    format::format_bytes,
    fs_guard::{is_safe_file_name, is_safe_relative_path},
    pagination::page_window,
    paths::{display_os_string, encode_path, format_file_href, format_folder_href},
    time::SystemTime,
};
