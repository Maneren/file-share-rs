//! `POST /upload` handlers: the single upload endpoint used by both curl
//! and the browser UI (which posts here directly with native XHR progress).

use std::path::{Path as StdPath, PathBuf};

use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use file_share_app::{
    UPLOAD_READ_ERROR_MESSAGE, UPLOAD_STORE_ERROR_MESSAGE, UPLOAD_TOO_LARGE_MESSAGE,
    format::format_bytes,
    fs_guard::{is_safe_file_name, remove_partial_upload, resolve_contained_path},
};
use leptos::logging;
use tokio::{
    fs::OpenOptions,
    io::{AsyncWriteExt, BufWriter},
};

use super::responses::{PATH_NOT_FOUND, UPLOAD_DISABLED};
use crate::state::AppState;

pub async fn file_upload_with_path(
    State(AppState {
        app_config,
        security,
        ..
    }): State<AppState>,
    Path(path): Path<String>,
    multipart: Multipart,
) -> impl IntoResponse {
    if !app_config.allow_upload {
        return UPLOAD_DISABLED.into_response();
    }

    let Some(base_path) = resolve_contained_path(&app_config.target_dir, StdPath::new(&path)).await
    else {
        return PATH_NOT_FOUND.into_response();
    };

    file_upload(base_path, multipart, security.max_upload_size)
        .await
        .into_response()
}

pub async fn file_upload_without_path(
    State(AppState {
        app_config,
        security,
        ..
    }): State<AppState>,
    multipart: Multipart,
) -> impl IntoResponse {
    if !app_config.allow_upload {
        return UPLOAD_DISABLED.into_response();
    }

    file_upload(
        app_config.target_dir.clone(),
        multipart,
        security.max_upload_size,
    )
    .await
    .into_response()
}

pub async fn file_upload(
    base_dir: PathBuf,
    mut multipart: Multipart,
    max_file_size: Option<u64>,
) -> impl IntoResponse {
    loop {
        let mut field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            // A corrupt/truncated multipart body must not look like an
            // empty (successful) upload. Debug-format: the Display is a
            // generic wrapper, the source carries the real multer error.
            Ok(None) => break,
            Err(e) => {
                logging::error!("Failed to read multipart: {e:?}");
                return (StatusCode::BAD_REQUEST, UPLOAD_READ_ERROR_MESSAGE).into_response();
            },
        };
        let Some(file_name) = field.file_name() else {
            continue;
        };

        // `file_name` is a single normal component, so joining it onto the
        // already-resolved `base_dir` cannot escape the share.
        let Some(path) = is_safe_file_name(file_name).then(|| base_dir.join(file_name)) else {
            return (
                StatusCode::BAD_REQUEST,
                format!("Invalid file name: {file_name}"),
            )
                .into_response();
        };

        logging::log!("Uploading to {path:?}");

        // Same policy as the browser upload: overwrite silently. Uploads are
        // opt-in (`--upload`), so a writer may replace its own files.
        let file = match OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .await
        {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                logging::error!("Permission denied creating {}: {err}", path.display());
                return (StatusCode::FORBIDDEN, UPLOAD_STORE_ERROR_MESSAGE).into_response();
            },
            Err(err) => {
                logging::error!("Failed to create file {}: {err}", path.display());
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    UPLOAD_STORE_ERROR_MESSAGE,
                )
                    .into_response();
            },
        };
        // Multipart chunks are only a few KiB; buffer to avoid a syscall
        // per chunk.
        let mut file = BufWriter::with_capacity(64 * 1024, file);

        let mut total_bytes: u64 = 0;
        loop {
            let chunk = match field.chunk().await {
                Ok(chunk) => chunk,
                Err(e) => {
                    logging::error!("Failed to read upload for {}: {e:?}", path.display());
                    drop(file);
                    remove_partial_upload(&path).await;
                    return (StatusCode::BAD_REQUEST, UPLOAD_READ_ERROR_MESSAGE).into_response();
                },
            };
            let Some(chunk) = chunk else { break };

            total_bytes += chunk.len() as u64;
            // Precise limit on file content (the HTTP layer only sees the
            // whole body including framing, so it can't judge file sizes).
            if max_file_size.is_some_and(|max| total_bytes > max) {
                logging::error!("Upload of {} exceeds the size limit", path.display());
                drop(file);
                remove_partial_upload(&path).await;
                return (StatusCode::PAYLOAD_TOO_LARGE, UPLOAD_TOO_LARGE_MESSAGE).into_response();
            }
            if let Err(err) = file.write_all(&chunk).await {
                logging::error!("Failed to write file {}: {err}", path.display());
                drop(file);
                remove_partial_upload(&path).await;
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    UPLOAD_STORE_ERROR_MESSAGE,
                )
                    .into_response();
            }
        }
        if let Err(err) = file.flush().await {
            logging::error!("Failed to flush file {}: {err}", path.display());
            drop(file);
            remove_partial_upload(&path).await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                UPLOAD_STORE_ERROR_MESSAGE,
            )
                .into_response();
        }

        logging::log!(
            "Writing {} bytes to {}",
            format_bytes(total_bytes),
            path.display()
        );
    }

    StatusCode::OK.into_response()
}
