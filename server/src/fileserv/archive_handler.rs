//! `GET /archive` handlers: stream a directory as tar/zip.
//!
//! Archive *bytes* are produced by [`super::archive_io::create_archive`].

use std::{
    fmt::Write as _,
    path::{Path as StdPath, PathBuf},
};

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderValue, Response, StatusCode, header},
    response::IntoResponse,
};
pub use file_share_app::archive::Method;
use file_share_app::fs_guard::resolve_contained_path;
use leptos::logging;
use serde::Deserialize;
use tokio::{fs, io, io::AsyncWriteExt as _, spawn};
use tokio_util::io::ReaderStream;

use super::{archive_io, responses::PATH_NOT_FOUND};
use crate::state::AppState;

/// Size of the in-memory pipe between archive creation and the HTTP body.
/// Large enough to keep a 1 Gbps link fed while the compressor runs ahead.
const DUPLEX_BUF_SIZE: usize = 1024 * 1024;

/// Chunk size used when polling the pipe into the HTTP body stream.
const READER_STREAM_CAPACITY: usize = 64 * 1024;

#[derive(Debug, Deserialize)]
pub struct ArchiveQuery {
    method: Option<Method>,
}

pub async fn handle_archive_with_path<'a>(
    State(app_state): State<AppState>,
    Path(path): Path<String>,
    Query(params): Query<ArchiveQuery>,
) -> impl IntoResponse + use<'a> {
    let target_dir = &app_state.app_config.target_dir;
    logging::log!("Handling archive with path '{path:?}' and params '{params:?}'");

    // Axum already percent-decodes `Path` params exactly once; decoding
    // again here would turn `%252e` into `.` and reopen traversal.
    let Some(path) = resolve_contained_path(target_dir, StdPath::new(&path)).await else {
        return PATH_NOT_FOUND.into_response();
    };

    if let Err(response) = check_archive_dir(&path).await {
        return *response;
    }

    handle_archive(path, params.method.unwrap_or_default()).into_response()
}

pub async fn handle_archive_without_path(
    State(app_state): State<AppState>,
    Query(params): Query<ArchiveQuery>,
) -> impl IntoResponse + use<> {
    logging::log!("Handling archive without path and with params '{params:?}'");
    let path = app_state.app_config.target_dir.clone();
    if let Err(response) = check_archive_dir(&path).await {
        return *response;
    }
    handle_archive(path, params.method.unwrap_or_default()).into_response()
}

/// Reject missing paths and non-directories before archive headers are sent.
/// Otherwise a bad target would produce a `200 OK` with a truncated body.
async fn check_archive_dir(path: &StdPath) -> Result<(), Box<Response<Body>>> {
    match fs::metadata(path).await {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(Box::new(
            (
                StatusCode::BAD_REQUEST,
                "Archives can only be created from directories",
            )
                .into_response(),
        )),
        Err(_) => Err(Box::new(PATH_NOT_FOUND.into_response())),
    }
}

fn handle_archive(path: PathBuf, archive_method: Method) -> impl IntoResponse + use<> {
    let Some(name) = path.file_name() else {
        return (
            StatusCode::BAD_REQUEST,
            format!("Invalid path (missing folder name): '{}'", path.display()),
        )
            .into_response();
    };
    let file_name = format!("{}.{}", name.display(), archive_method);

    logging::log!("Creating: {file_name}");

    let Some(disposition) = content_disposition(&file_name) else {
        logging::error!("Failed to build Content-Disposition for {file_name:?}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to prepare download",
        )
            .into_response();
    };

    let (mut writer, reader) = io::duplex(DUPLEX_BUF_SIZE);
    let stream = ReaderStream::with_capacity(reader, READER_STREAM_CAPACITY);

    spawn(async move {
        if let Err(err) = archive_io::create_archive(archive_method, path, &mut writer).await {
            logging::error!("Error during archive creation: {err:?}");
            if let Err(err) = writer.shutdown().await {
                logging::error!("Failed to shut down archive stream: {err}");
            }
        }
    });

    let headers = [
        (header::CONTENT_DISPOSITION, disposition),
        (
            header::CONTENT_TYPE,
            archive_method
                .mimetype()
                .parse()
                .expect("Static mimetypes are valid"),
        ),
        (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
    ];

    (headers, Body::from_stream(stream)).into_response()
}

/// Build a `Content-Disposition` value safe for on-disk file names.
///
/// The quoted `filename` carries an ASCII-only fallback (`"` and other
/// problematic characters replaced), while `filename*` (RFC 5987) carries
/// the exact UTF-8 name. Returns `None` when no valid header can be built
/// instead of panicking on attacker-influenced input.
fn content_disposition(file_name: &str) -> Option<HeaderValue> {
    fn is_attr_char(b: u8) -> bool {
        b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
            )
    }

    let fallback: String = file_name
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() && !matches!(c, '"' | '\\') {
                c
            } else {
                '_'
            }
        })
        .collect();

    let mut encoded = String::with_capacity(file_name.len());
    for b in file_name.bytes() {
        if is_attr_char(b) {
            encoded.push(b as char);
        } else {
            let _ = write!(encoded, "%{b:02X}");
        }
    }

    format!(r#"attachment; filename="{fallback}"; filename*=UTF-8''{encoded}"#)
        .parse()
        .ok()
}
