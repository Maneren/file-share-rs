mod archive;

use std::{
    collections::HashMap,
    fmt::Write as _,
    path::{Path as StdPath, PathBuf},
};

use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{HeaderValue, Request, Response, StatusCode, Uri, header},
    response::IntoResponse,
};
pub use file_share_app::archive::Method;
use file_share_app::{
    AppState, shell,
    utils::{format_bytes, is_safe_file_name, is_safe_relative_path, try_decode_path},
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

/// Handles archive requests.
#[allow(clippy::implicit_hasher)]
pub async fn handle_archive_with_path<'a>(
    State(app_state): State<AppState>,
    Path(path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse + use<'a> {
    let target_dir = &app_state.app_config.target_dir;
    logging::log!("Handling archive with path '{path:?}' and params '{params:?}'");

    let Some(path) = safe_join_path(target_dir, &try_decode_path(&path)) else {
        return (StatusCode::BAD_REQUEST, format!("Invalid path: {path}")).into_response();
    };

    handle_archive(path, params.get("method")).into_response()
}

/// Handles archive requests.
#[allow(clippy::implicit_hasher)]
pub async fn handle_archive_without_path(
    State(app_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse + use<> {
    logging::log!("Handling archive without path and with params '{params:?}'");
    handle_archive(
        app_state.app_config.target_dir.clone(),
        params.get("method"),
    )
}

fn handle_archive(path: PathBuf, method: Option<&String>) -> impl IntoResponse + use<> {
    let method = method.map_or_default(String::as_str);

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
        if let Err(err) = archive::create_archive(archive_method, path, &mut writer).await {
            logging::error!("Error during archive creation: {err:?}");
            writer.shutdown().await.expect("Failed to shutdown writer");
        }
    });

    let headers: [(_, HeaderValue); 3] = [
        (
            header::CONTENT_DISPOSITION,
            format!(r#"attachment; filename="{file_name}""#).parse(),
        ),
        (header::CONTENT_TYPE, archive_method.mimetype().parse()),
        (header::CACHE_CONTROL, "no-cache".parse()),
    ]
    .map(|(key, value)| (key, value.expect("The headers are valid")));

    (headers, Body::from_stream(stream)).into_response()
}

const UPLOAD_DISABLED: (StatusCode, &str) = (StatusCode::FORBIDDEN, "Upload is not enabled");

fn safe_join_path(base_dir: &StdPath, path: &StdPath) -> Option<PathBuf> {
    is_safe_relative_path(path).then(|| base_dir.join(path))
}

fn safe_join_file_name(base_dir: &StdPath, file_name: &str) -> Option<PathBuf> {
    is_safe_file_name(file_name).then(|| base_dir.join(file_name))
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

    file_upload(app_config.target_dir.clone(), multipart)
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
            let chunk = match field.chunk().await {
                Ok(chunk) => chunk,
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        format!("Invalid file content: {e}"),
                    )
                        .into_response();
                },
            };
            let Some(chunk) = chunk else { break };

            total_bytes += chunk.len() as u64;
            if let Err(err) = file.write_all(&chunk).await {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to write file: {err}"),
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
