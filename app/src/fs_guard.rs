//! Filesystem containment guards shared by server functions.
//!
//! [`resolve_contained_path`] is SSR-only (it touches the real filesystem via
//! `tokio::fs::canonicalize`); the pure `is_safe_*` predicates compile for
//! WASM too.

#[cfg(feature = "ssr")]
use std::path::PathBuf;
use std::path::{Component, Path};

pub fn is_safe_relative_path(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|comp| matches!(comp, Component::Normal(_)))
}

pub fn is_safe_file_name(name: impl AsRef<Path>) -> bool {
    let mut components = name.as_ref().components();

    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

/// Join `rel` onto the canonical `base` share directory and resolve it.
///
/// Returns `None` when `rel` is not a safe relative path, does not exist, or
/// escapes `base` (e.g. via a symlink planted inside the share). `base` must
/// itself be canonical, which the server guarantees at startup. Callers map
/// `None` to `404` without distinguishing the cause, so attackers cannot
/// probe for symlinks or missing paths.
#[cfg(feature = "ssr")]
pub async fn resolve_contained_path(base: &Path, rel: &Path) -> Option<PathBuf> {
    if !is_safe_relative_path(rel) {
        return None;
    }
    let resolved = tokio::fs::canonicalize(base.join(rel)).await.ok()?;
    resolved.starts_with(base).then_some(resolved)
}

/// Best-effort removal of a partially written upload so failed transfers
/// don't leave corrupt files behind.
#[cfg(feature = "ssr")]
pub async fn remove_partial_upload(path: &Path) {
    if let Err(e) = tokio::fs::remove_file(path).await {
        leptos::logging::error!("Failed to remove partial upload {}: {e}", path.display());
    }
}
