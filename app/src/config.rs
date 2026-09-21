//! Shared server configuration.
//!
//! Client-facing strings live in [`crate::messages`].

use std::path::PathBuf;

pub use crate::messages::{
    PATH_NOT_FOUND_MESSAGE, UPLOAD_DISABLED_MESSAGE, UPLOAD_READ_ERROR_MESSAGE,
    UPLOAD_STORE_ERROR_MESSAGE,
};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub target_dir: PathBuf,
    pub allow_upload: bool,
}
