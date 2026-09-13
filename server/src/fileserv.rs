mod archive;

use std::{
    collections::HashMap,
    fmt::Write,
    path::{Component as StdComponent, Path as StdPath, PathBuf},
};

pub use archive::Method;
use async_compression::{
    Level,
    tokio::write::{GzipEncoder, ZstdEncoder},
};
use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{HeaderValue, Request, Response, StatusCode, Uri, header},
    response::IntoResponse,
};
pub use file_share_app::archive::Method;
use file_share_app::{
    AppConfig, AppState, shell,
    utils::{format_bytes, is_safe_relative_path, try_decode_path},
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
        move || provide_context(app_state.app_config.clone()),
        move || shell(app_state.leptos_options.clone()),
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

/// Handles archive requests.
#[allow(clippy::implicit_hasher)]
pub async fn handle_archive_with_path<'a>(
    State(AppConfig { target_dir, .. }): State<AppConfig>,
    Path(path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse + use<'a> {
    logging::log!("Handling archive with path '{path:?}' and params '{params:?}'");

    let Some(path) = safe_join_path(&target_dir, &try_decode_path(&path)) else {
        return (StatusCode::BAD_REQUEST, format!("Invalid path: {path}")).into_response();
    };

    handle_archive(path, params.get("method"))
        .await
        .into_response()
}

/// Handles archive requests.
#[allow(clippy::implicit_hasher)]
pub async fn handle_archive_without_path(
    State(AppConfig { target_dir, .. }): State<AppConfig>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse + use<> {
    logging::log!("Handling archive without path and with params '{params:?}'");
    handle_archive(target_dir, params.get("method")).await
}

async fn handle_archive(path: PathBuf, method: Option<&String>) -> impl IntoResponse + use<> {
    let method = method.map_or_else(Default::default, String::as_str);

    let Ok(archive_method) = Method::try_from(method) else {
        return (
            StatusCode::BAD_REQUEST,
            format!("Invalid archive method: {method}"),
        )
            .into_response();
    };

    let Some(name) = path.file_name() else {
        return (
            StatusCode::BAD_REQUEST,
            format!("Invalid path (missing folder name): '{}'", path.display()),
        )
            .into_response();
    };
    let file_name = format!("{}.{}", name.display(), archive_method);

    logging::log!("Creating: {file_name}");

    let (mut writer, reader) = tokio::io::duplex(DUPLEX_BUF_SIZE);
    let stream = ReaderStream::with_capacity(reader, READER_STREAM_CAPACITY);

    tokio::spawn(async move {
        if let Err(err) = archive_method.create_archive(path, &mut writer).await {
            logging::error!("Error during archive creation: {err:?}");
            writer.shutdown().await.expect("Failed to shutdown writer");
        }
    });

    let headers: [(_, HeaderValue); 6] = [
        (
            header::CONTENT_DISPOSITION,
            format!(r#"attachment; filename="{file_name}""#).parse(),
        ),
        (header::CONTENT_TYPE, archive_method.mimetype().parse()),
        (header::TRANSFER_ENCODING, "chunked".parse()),
        (header::CACHE_CONTROL, "no-cache".parse()),
        (header::CONNECTION, "keep-alive".parse()),
        (header::CONTENT_ENCODING, "identity".parse()),
    ]
    .map(|(key, value)| (key, value.expect("The headers are valid")));

    (headers, Body::from_stream(stream)).into_response()
}

/// Single-file streaming download with optional on-the-fly compression.
/// The client decompresses via `DecompressionStream` and streams straight to
/// disk (`showSaveFilePicker`), so a 100 GiB file never sits in RAM.
/// `?compress=zstd` (default) is tuned for a 1 Gbps link; `gzip` is for very
/// old clients, `none` skips compression entirely.
#[allow(clippy::implicit_hasher)]
pub async fn handle_file_download(
    State(AppConfig { target_dir, .. }): State<AppConfig>,
    Path(path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let Some(path) = safe_join_path(&target_dir, &try_decode_path(&path)) else {
        return (StatusCode::BAD_REQUEST, format!("Invalid path: {path}")).into_response();
    };

    let Ok(meta) = tokio::fs::metadata(&path).await else {
        return (StatusCode::NOT_FOUND, "File not found".to_string()).into_response();
    };
    if !meta.is_file() {
        return (
            StatusCode::BAD_REQUEST,
            "Path is not a file (use /archive for folders)".to_string(),
        )
            .into_response();
    }

    let compression = params.get("compress").map_or("zstd", String::as_str);
    let (algo, extension) = match compression {
        "none" | "identity" => ("identity", ""),
        "gzip" | "gz" => ("gzip", ".gz"),
        "zstd" | "zst" => ("zstd", ".zst"),
        other => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Invalid compress parameter: {other} (use zstd, gzip, none)"),
            )
                .into_response();
        },
    };

    let file_name = format!(
        "{}{extension}",
        path.file_name().map_or_else(
            || "download".to_string(),
            |n| n.to_string_lossy().into_owned()
        ),
    );
    logging::log!("Streaming file {file_name} with compression {algo}");

    let (mut writer, reader) = tokio::io::duplex(DUPLEX_BUF_SIZE);
    let stream = ReaderStream::with_capacity(reader, READER_STREAM_CAPACITY);

    tokio::spawn(async move {
        let result = match algo {
            "gzip" => {
                let mut enc = GzipEncoder::with_quality(&mut writer, Level::Fastest);
                let r = copy_file_to_writer(&path, &mut enc).await;
                let _ = enc.shutdown().await;
                r
            },
            "zstd" => {
                let mut enc = ZstdEncoder::with_quality(&mut writer, Level::Fastest);
                let r = copy_file_to_writer(&path, &mut enc).await;
                let _ = enc.shutdown().await;
                r
            },
            _ => copy_file_to_writer(&path, &mut writer).await,
        };
        if let Err(err) = result {
            logging::error!("Error during file streaming: {err:?}");
        }
        let _ = writer.shutdown().await;
    });

    let headers = [
        (
            header::CONTENT_DISPOSITION,
            format!(r#"attachment; filename="{file_name}""#)
                .parse()
                .unwrap_or(HeaderValue::from_static("attachment")),
        ),
        (
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        ),
        // Deliberately NOT `Content-Encoding`: the browser must not
        // transparently decode, our JS pipes through `DecompressionStream`.
        (
            header::HeaderName::from_static("x-compression"),
            HeaderValue::from_str(algo).unwrap_or(HeaderValue::from_static("identity")),
        ),
        (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
    ];

    (headers, Body::from_stream(stream)).into_response()
}

async fn copy_file_to_writer<W>(path: &StdPath, writer: &mut W) -> std::io::Result<u64>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut file = File::open(path).await?;
    tokio::io::copy(&mut file, writer).await
}

const UPLOAD_DISABLED: (StatusCode, &str) = (StatusCode::FORBIDDEN, "Upload is not enabled");

fn safe_join_path(base_dir: &StdPath, path: &StdPath) -> Option<PathBuf> {
    if !is_safe_relative_path(path) {
        return None;
    }
    Some(base_dir.join(path))
}

fn safe_join_file_name(base_dir: &StdPath, file_name: &str) -> Option<PathBuf> {
    if file_name.is_empty() {
        return None;
    }
    let p = StdPath::new(file_name);
    if p.components().count() != 1 {
        return None;
    }
    if !matches!(p.components().next(), Some(StdComponent::Normal(_))) {
        return None;
    }
    safe_join_path(base_dir, p)
}

pub async fn file_upload_with_path(
    State(AppState { app_config, .. }): State<AppState>,
    Path(path): Path<String>,
    multipart: Multipart,
) -> impl IntoResponse {
    if !app_config.allow_upload {
        return UPLOAD_DISABLED.into_response();
    }

    let Some(base_path) = safe_join_path(&app_config.target_dir, StdPath::new(&path)) else {
        return (StatusCode::BAD_REQUEST, format!("Invalid path: {path}")).into_response();
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

    file_upload(app_config.target_dir, multipart)
        .await
        .into_response()
}

pub async fn file_upload(base_dir: PathBuf, mut multipart: Multipart) -> impl IntoResponse {
    while let Ok(Some(mut field)) = multipart.next_field().await {
        let Some(file_name) = field.file_name() else {
            continue;
        };

        let Some(path) = safe_join_file_name(&base_dir, file_name) else {
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
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to create file: {err}"),
                )
                    .into_response();
            },
        };

        let mut total_bytes: u64 = 0;
        loop {
            match field.chunk().await {
                Ok(Some(chunk)) => {
                    total_bytes += chunk.len() as u64;
                    if let Err(err) = file.write_all(&chunk).await {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Failed to write file: {err}"),
                        )
                            .into_response();
                    }
                },
                Ok(None) => break,
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        format!("Invalid file content: {e}"),
                    )
                        .into_response();
                },
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
