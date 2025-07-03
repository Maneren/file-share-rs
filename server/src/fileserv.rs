mod archive;

use std::{collections::HashMap, path::PathBuf};

pub use archive::Method;
use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderValue, Request, StatusCode, Uri},
    response::IntoResponse,
};
use file_share_app::{shell, utils::format_bytes, AppState};
use leptos::{logging, prelude::provide_context};
use rust_embed::RustEmbed;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

#[derive(RustEmbed)]
#[folder = "../target/site"]
struct StaticFiles;

/// Handles static file requests by delegating to `StaticFiles`.
///
/// # Panics
///
/// This function will panic if the mimetype from `RustEmbed` is not recognized
/// by `http` crate
pub async fn file_and_error_handler(
    State(app_state): State<AppState>,
    uri: Uri,
    request: Request<Body>,
) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    if let Some(file) = StaticFiles::get(path) {
        let header = (
            header::CONTENT_TYPE,
            HeaderValue::from_str(file.metadata.mimetype())
                .expect("RustEmbed returns valid mimetypes"),
        );
        return (StatusCode::OK, [header], file.data).into_response();
    }

    let handler = leptos_axum::render_app_to_stream_with_context(
        // app_state.leptos_options.clone(),
        move || provide_context(app_state.target_dir.clone()),
        move || shell(app_state.leptos_options.clone()),
    );
    handler(request).await.into_response()
}

/// Handles archive requests.
#[allow(clippy::implicit_hasher)]
pub async fn handle_archive_with_path(
    State(base_dir): State<PathBuf>,
    Path(path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    logging::log!("Handling archive with path '{path:?}' and params '{params:?}'");
    handle_archive(base_dir, params.get("method"), path).await
}

/// Handles archive requests.
#[allow(clippy::implicit_hasher)]
pub async fn handle_archive_without_path(
    State(base_dir): State<PathBuf>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    logging::log!("Handling archive without path and with params '{params:?}'");
    handle_archive(base_dir, params.get("method"), String::new()).await
}

#[allow(clippy::unused_async)] // has to be in an async context, but doesn't await directly
async fn handle_archive(
    base_dir: PathBuf,
    method: Option<&String>,
    path: String,
) -> impl IntoResponse {
    let method = method.map_or_else(Default::default, String::as_str);

    let Ok(archive_method) = Method::try_from(method) else {
        return (
            StatusCode::BAD_REQUEST,
            format!("Invalid archive method: {method}"),
        )
            .into_response();
    };

    let path = base_dir.join(path);
    let Some(name) = path.file_name() else {
        return (
            StatusCode::BAD_REQUEST,
            format!("Invalid path (missing folder name): '{}'", path.display()),
        )
            .into_response();
    };
    let file_name = format!("{}.{}", name.display(), archive_method);

    logging::log!("Creating: {file_name}");

    let (mut writer, reader) = tokio::io::duplex(256 * 1024);
    let stream = ReaderStream::new(reader);

    tokio::spawn(async move {
        if let Err(err) = archive_method.create_archive(path, &mut writer).await {
            log::error!("Error during archive creation: {err:?}");
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

pub async fn file_upload_with_path(
    State(base_dir): State<PathBuf>,
    Path(path): Path<String>,
    multipart: Multipart,
) -> impl IntoResponse {
    file_upload(base_dir, path, multipart).await
}

pub async fn file_upload_without_path(
    State(base_dir): State<PathBuf>,
    multipart: Multipart,
) -> impl IntoResponse {
    file_upload(base_dir, String::new(), multipart).await
}

pub async fn file_upload(
    base_dir: PathBuf,
    path: String,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let base_req_path = base_dir.join(&path);
    while let Ok(Some(field)) = multipart.next_field().await {
        let Some(file_name) = field.file_name() else {
            continue;
        };
        let path = base_req_path.join(file_name);

        logging::log!("Uploading to {path:?}");

        let mut file = match tokio::fs::File::create(&path).await {
            Ok(file) => file,
            Err(err) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to create file: {err}"),
                )
                    .into_response();
            },
        };

        let bytes = match field.bytes().await {
            Ok(bytes) => bytes,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    format!("Invalid file content: {e}"),
                )
                    .into_response();
            },
        };

        logging::log!(
            "Writing {} bytes to {}",
            format_bytes(bytes.len() as u64),
            path.display()
        );

        if let Err(err) = file.write_all(&bytes).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to write file: {err}"),
            )
                .into_response();
        }
    }

    StatusCode::OK.into_response()
}
