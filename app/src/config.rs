use std::path::PathBuf;

/// Message returned to clients when uploads are disabled.
pub const UPLOAD_DISABLED_MESSAGE: &str = "Uploads are disabled";

/// Message returned to clients when a path is missing or escapes the share.
/// Deliberately uniform so missing paths cannot be told apart from symlink
/// probes.
pub const PATH_NOT_FOUND_MESSAGE: &str = "Requested path not found";

/// Message returned to clients when receiving an upload fails.
/// Details stay server-side in the logs.
pub const UPLOAD_READ_ERROR_MESSAGE: &str = "Failed to read upload";

/// Message returned to clients when storing an upload fails.
/// Details stay server-side in the logs.
pub const UPLOAD_STORE_ERROR_MESSAGE: &str = "Failed to store upload";

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub target_dir: PathBuf,
    pub allow_upload: bool,
}
