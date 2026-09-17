mod archive;

use std::{
    fmt::Write as _,
    path::{Path as StdPath, PathBuf},
};

use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{HeaderValue, Request, Response, StatusCode, Uri, header},
    middleware::Next,
    response::{IntoResponse, Response as AxumResponse},
};
pub use file_share_app::archive::Method;
use file_share_app::{
    AppState, shell,
    utils::{format_bytes, is_safe_file_name, resolve_contained_path},
};
use leptos::{logging, prelude::provide_context};
use rust_embed::{EmbeddedFile, RustEmbed};
use tokio::{fs::File, io::AsyncWriteExt};
use tokio_util::io::ReaderStream;

#[derive(RustEmbed)]
#[folder = "../target/site"]
struct StaticFiles;

/// Size of the in-memory pipe between archive creation and the HTTP body.
/// Large enough to keep a 1 Gbps link fed while the compressor runs ahead.
const DUPLEX_BUF_SIZE: usize = 1024 * 1024;

/// Chunk size used when polling the pipe into the HTTP body stream.
const READER_STREAM_CAPACITY: usize = 64 * 1024;

/// Handles static file requests by delegating to `StaticFiles`.
pub async fn file_and_error_handler(
    State(app_state): State<AppState>,
    uri: Uri,
    request: Request<Body>,
) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    if let Some(file) = StaticFiles::get(path) {
        return serve_static_file(&request, path, file);
    }

    let handler = leptos_axum::render_app_to_stream_with_context(
        {
            let app_config = app_state.app_config.clone();
            move || provide_context(app_config.clone())
        },
        {
            let leptos_options = app_state.leptos_options.clone();
            move || shell((*leptos_options).clone())
        },
    );
    handler(request).await.into_response()
}

fn serve_static_file(request: &Request<Body>, path: &str, file: EmbeddedFile) -> Response<Body> {
    let etag = {
        let hash = file.metadata.sha256_hash();
        let mut hash_string = String::with_capacity(hash.len() * 2);
        for byte in hash {
            write!(hash_string, "{byte:02x}").expect("Writing to a string can't fail");
        }
        hash_string
    };

    if request
        .headers()
        .get(header::IF_NONE_MATCH)
        .is_some_and(|value| value == etag.as_str())
    {
        // Content hasn't changed; return 304 Not Modified
        logging::debug_log!("Serving static file '{path}' with 304 Not Modified");
        return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag)], "").into_response();
    }

    logging::debug_log!("Serving static file '{path}'");
    let content_type = (
        header::CONTENT_TYPE,
        HeaderValue::from_str(file.metadata.mimetype())
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    let cache_control = (header::CACHE_CONTROL, HeaderValue::from_static("public"));
    let etag_header = (
        header::ETAG,
        HeaderValue::from_str(&etag).expect("hash is valid utf-8"),
    );

    (
        StatusCode::OK,
        [content_type, cache_control, etag_header],
        file.data,
    )
        .into_response()
}

#[derive(Debug, serde::Deserialize)]
pub struct ArchiveQuery {
    method: Option<Method>,
}

/// Handles archive requests.
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
        return response;
    }

    handle_archive(path, params.method.unwrap_or_default()).into_response()
}

/// Handles archive requests.
pub async fn handle_archive_without_path(
    State(app_state): State<AppState>,
    Query(params): Query<ArchiveQuery>,
) -> impl IntoResponse + use<> {
    logging::log!("Handling archive without path and with params '{params:?}'");
    let path = app_state.app_config.target_dir.clone();
    if let Err(response) = check_archive_dir(&path).await {
        return response;
    }
    handle_archive(path, params.method.unwrap_or_default()).into_response()
}

/// Reject missing paths and non-directories before archive headers are sent.
/// Otherwise a bad target would produce a `200 OK` with a truncated body.
async fn check_archive_dir(path: &StdPath) -> Result<(), Response<Body>> {
    match tokio::fs::metadata(path).await {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err((
            StatusCode::BAD_REQUEST,
            "Archives can only be created from directories",
        )
            .into_response()),
        Err(_) => Err(PATH_NOT_FOUND.into_response()),
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

    let (mut writer, reader) = tokio::io::duplex(DUPLEX_BUF_SIZE);
    let stream = ReaderStream::with_capacity(reader, READER_STREAM_CAPACITY);

    tokio::spawn(async move {
        if let Err(err) = archive::create_archive(archive_method, path, &mut writer).await {
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
        b.is_ascii_alphanumeric() || matches!(b, b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~')
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
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{b:02X}");
        }
    }

    format!(r#"attachment; filename="{fallback}"; filename*=UTF-8''{encoded}"#)
        .parse()
        .ok()
}

const UPLOAD_DISABLED: (StatusCode, &str) = (StatusCode::FORBIDDEN, "Upload is not enabled");
const PATH_NOT_FOUND: (StatusCode, &str) = (StatusCode::NOT_FOUND, "Requested path not found");

/// Reject requests escaping the share before `ServeDir` sees them.
///
/// `ServeDir` opens paths on disk as-is, so a symlink inside the share
/// pointing outside (e.g. `link -> /etc`) would expose arbitrary files.
/// Resolving through the real filesystem here closes that hole; valid
/// requests pass through untouched, preserving `ServeDir` range support.
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
    let Ok(decoded) = urlencoding::decode(rel) else {
        return PATH_NOT_FOUND.into_response();
    };
    let base = &app_state.app_config.target_dir;
    match tokio::fs::canonicalize(base.join(StdPath::new(decoded.as_ref()))).await {
        Ok(canonical) if canonical.starts_with(base) => next.run(req).await,
        _ => PATH_NOT_FOUND.into_response(),
    }
}

pub async fn file_upload_with_path(
    State(AppState { app_config, .. }): State<AppState>,
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

    file_upload(base_path, multipart).await.into_response()
}

pub async fn file_upload_without_path(
    State(AppState { app_config, .. }): State<AppState>,
    multipart: Multipart,
) -> impl IntoResponse {
    if !app_config.allow_upload {
        return UPLOAD_DISABLED.into_response();
    }

    file_upload(app_config.target_dir.clone(), multipart)
        .await
        .into_response()
}

pub async fn file_upload(base_dir: PathBuf, mut multipart: Multipart) -> impl IntoResponse {
    while let Ok(Some(mut field)) = multipart.next_field().await {
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

        let mut file = match File::create_new(&path).await {
            Ok(file) => file,
            Err(err) => {
                logging::error!("Failed to create file {}: {err}", path.display());
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to create file",
                )
                    .into_response();
            },
        };

        let mut total_bytes: u64 = 0;
        loop {
            let chunk = match field.chunk().await {
                Ok(chunk) => chunk,
                Err(e) => {
                    logging::error!("Failed to read upload for {}: {e}", path.display());
                    return (StatusCode::BAD_REQUEST, "Failed to read upload").into_response();
                },
            };
            let Some(chunk) = chunk else { break };

            total_bytes += chunk.len() as u64;
            if let Err(err) = file.write_all(&chunk).await {
                logging::error!("Failed to write file {}: {err}", path.display());
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to store upload",
                )
                    .into_response();
            }
        }

        logging::log!(
            "Writing {} bytes to {}",
            format_bytes(total_bytes),
            path.display()
        );
    }

    StatusCode::OK.into_response()
}
