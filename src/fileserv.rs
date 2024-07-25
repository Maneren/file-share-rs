mod archive;

use std::collections::HashMap;

use axum::{
  body::Body,
  extract::{Multipart, Path, Query, State},
  http::{header, HeaderValue, Request, StatusCode, Uri},
  response::IntoResponse,
};
use leptos::*;
use rust_embed::RustEmbed;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

pub use crate::fileserv::archive::ArchiveMethod;
use crate::{app::App, config::get_target_dir, utils::format_bytes};

#[derive(RustEmbed)]
#[folder = "target/site"]
struct StaticFiles;

/// Handles static file requests by delegating to `StaticFiles`.
pub async fn file_and_error_handler(
  uri: Uri,
  State(options): State<LeptosOptions>,
  request: Request<Body>,
) -> impl IntoResponse {
  let path = uri.path().trim_start_matches('/');

  if let Some(file) = StaticFiles::get(path) {
    let header = (
      header::CONTENT_TYPE,
      HeaderValue::from_str(file.metadata.mimetype()).unwrap(),
    );
    return (StatusCode::OK, [header], file.data).into_response();
  }

  let handler = leptos_axum::render_app_to_stream(options.clone(), App);
  handler(request).await.into_response()
}

/// Handles archive requests.
#[allow(clippy::implicit_hasher)]
#[allow(clippy::unused_async)]
pub async fn handle_archive_with_path(
  Path(path): Path<String>,
  Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
  logging::log!("Handling archive with path '{path:?}' and params '{params:?}'");
  handle_archive(params.get("method"), path)
}

/// Handles archive requests.
#[allow(clippy::implicit_hasher)]
#[allow(clippy::unused_async)]
pub async fn handle_archive_without_path(
  Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
  logging::log!("Handling archive without path and params '{params:?}'");
  handle_archive(params.get("method"), String::new())
}

fn handle_archive(method: Option<&String>, path: String) -> impl IntoResponse {
  let method = method.map_or("tar", String::as_str);

  let Ok(archive_method) = ArchiveMethod::try_from(method) else {
    return (
      StatusCode::BAD_REQUEST,
      format!("Invalid archive method: {method}"),
    )
      .into_response();
  };

  let path = get_target_dir().join(path);
  let file_name = format!(
    "{}.{}",
    path.file_name().unwrap().to_string_lossy(),
    archive_method
  );

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
      format!("attachment; filename=\"{file_name}\"")
        .parse()
        .unwrap(),
    ),
    (
      header::CONTENT_TYPE,
      archive_method.mimetype().parse().unwrap(),
    ),
    (header::TRANSFER_ENCODING, "chunked".parse().unwrap()),
    (header::CACHE_CONTROL, "no-cache".parse().unwrap()),
    (header::CONNECTION, "keep-alive".parse().unwrap()),
    (header::CONTENT_ENCODING, "identity".parse().unwrap()),
  ];

  (headers, Body::from_stream(stream)).into_response()
}

pub async fn file_upload_with_path(
  Path(path): Path<String>,
  multipart: Multipart,
) -> impl IntoResponse {
  file_upload(path, multipart).await
}

pub async fn file_upload_without_path(multipart: Multipart) -> impl IntoResponse {
  file_upload(String::new(), multipart).await
}

pub async fn file_upload(path: String, mut multipart: Multipart) -> impl IntoResponse {
  let base_path = get_target_dir().join(&path);
  while let Some(field) = multipart.next_field().await.unwrap() {
    let Some(file_name) = field.file_name() else {
      continue;
    };
    let path = base_path.join(file_name);

    logging::log!("Uploading to {path:?}");

    let mut file = match tokio::fs::File::create(&path).await {
      Ok(file) => file,
      Err(err) => {
        return (
          StatusCode::INTERNAL_SERVER_ERROR,
          format!("Failed to create file: {err}"),
        )
          .into_response();
      }
    };

    let bytes = match field.bytes().await {
      Ok(bytes) => bytes,
      Err(e) => {
        return (
          StatusCode::BAD_REQUEST,
          format!("Invalid file content: {e}"),
        )
          .into_response();
      }
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
