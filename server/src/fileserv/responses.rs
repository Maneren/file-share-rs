//! Shared HTTP error responses (status + client-safe message).

use axum::http::StatusCode;
use file_share_app::{PATH_NOT_FOUND_MESSAGE, UPLOAD_DISABLED_MESSAGE};

pub const UPLOAD_DISABLED: (StatusCode, &str) = (StatusCode::FORBIDDEN, UPLOAD_DISABLED_MESSAGE);
pub const PATH_NOT_FOUND: (StatusCode, &str) = (StatusCode::NOT_FOUND, PATH_NOT_FOUND_MESSAGE);
