mod archive;

use std::{
    collections::HashMap,
    ops::Not,
    os,
    path::{self, PathBuf},
};

pub use archive::Method;
use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{HeaderValue, Request, StatusCode, Uri, header},
    response::IntoResponse,
};
use file_share_app::{
    AppConfig, AppState, shell,
    utils::{format_bytes, try_decode_path},
};
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
        move || provide_context(app_state.app_config.clone()),
        move || shell(app_state.leptos_options.clone()),
    );
    handler(request).await.into_response()
}

/// Handles archive requests.
#[allow(clippy::implicit_hasher)]
pub async fn handle_archive_with_path<'a>(
    State(AppConfig { target_dir, .. }): State<AppConfig>,
    Path(path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse + use<'a> {
    logging::log!("Handling archive with path '{path:?}' and params '{params:?}'");

    let Some(path) = safe_join_path(&target_dir, try_decode_path(&path).as_ref()) else {
        return (StatusCode::BAD_REQUEST, format!("Invalid path: {path}")).into_response();
    };

    handle_archive(path, params.get("method")).await
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

#[allow(clippy::unused_async)] // has to be in an async context, but doesn't await directly
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

const UPLOAD_DISABLED: (StatusCode, &'static str) =
    (StatusCode::FORBIDDEN, "Upload is not enabled");

fn safe_join_path(base_dir: &path::Path, path: &str) -> Option<PathBuf> {
    path.contains("..").not().then(|| base_dir.join(path))
}

pub async fn file_upload_with_path(
    State(AppState { app_config, .. }): State<AppState>,
    Path(path): Path<String>,
    multipart: Multipart,
) -> impl IntoResponse {
    if !app_config.allow_upload {
        return UPLOAD_DISABLED.into_response();
    }

    let Some(base_path) = safe_join_path(&app_config.target_dir, &path) else {
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
    while let Ok(Some(field)) = multipart.next_field().await {
        let Some(file_name) = field.file_name() else {
            continue;
        };

        let Some(path) = safe_join_path(&base_dir, file_name) else {
            return (
                StatusCode::BAD_REQUEST,
                format!("Invalid file name: {file_name}"),
            )
                .into_response();
        };

        logging::log!("Uploading to {path:?}");

        let mut file = match tokio::fs::File::create_new(&path).await {
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
