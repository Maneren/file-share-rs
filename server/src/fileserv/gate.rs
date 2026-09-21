//! Symlink-containment gate in front of `ServeDir`.
//!
//! `ServeDir` opens paths on disk as-is, so a symlink inside the share
//! pointing outside (e.g. `link -> /etc`) would expose arbitrary files.
//! Resolving through the real filesystem here closes that hole; valid
//! requests pass through untouched, preserving `ServeDir` range support.

use std::path::Path as StdPath;

use axum::{
    body::Body,
    extract::State,
    http::Request,
    middleware::Next,
    response::{IntoResponse, Response as AxumResponse},
};
use tokio::fs;
use urlencoding::decode;

use super::responses::PATH_NOT_FOUND;
use crate::state::AppState;

pub async fn gate_shared_files(
    State(app_state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> AxumResponse {
    let rel = req
        .uri()
        .path()
        .trim_start_matches("/files")
        .trim_start_matches('/');
    // `ServeDir` decodes percent-encoding itself, so validate the decoded
    // form exactly once here; anything unresolvable is a 404 either way.
    let Ok(decoded) = decode(rel) else {
        return PATH_NOT_FOUND.into_response();
    };
    let base = &app_state.app_config.target_dir;
    match fs::canonicalize(base.join(StdPath::new(decoded.as_ref()))).await {
        Ok(canonical) if canonical.starts_with(base) => next.run(req).await,
        _ => PATH_NOT_FOUND.into_response(),
    }
}
