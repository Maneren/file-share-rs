mod archive;
mod pipe;

use std::collections::HashMap;

use futures::channel::mpsc::channel;
use http::header::{self, HeaderValue};
use leptos::*;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "$CARGO_TARGET_DIR/site"]
struct StaticFiles;

use axum::{
  body::{Body, StreamBody},
  extract::{Multipart, Path, Query, State},
  http::{Request, Response, StatusCode, Uri},
  response::{IntoResponse, Redirect},
};
use tokio::{io, io::AsyncWriteExt};

pub use crate::fileserv::archive::ArchiveMethod;
use crate::{app::App, config::get_target_dir, fileserv::pipe::Pipe};

/// Handles static file requests by delegating to `StaticFiles`.
pub async fn file_and_error_handler(
  uri: Uri,
  State(options): State<LeptosOptions>,
  request: Request<Body>,
) -> impl IntoResponse {
  let path = uri.path().trim_start_matches('/');

  if let Some(file) = StaticFiles::get(path) {
    return Response::builder()
      .status(StatusCode::OK)
      .header(
        header::CONTENT_TYPE,
        HeaderValue::from_str(file.metadata.mimetype()).unwrap(),
      )
      .body(Body::from(file.data))
      .unwrap()
      .into_response();
  }

  let handler = leptos_axum::render_app_to_stream(options.clone(), App);
  handler(request).await.into_response()
}

fn response(status: StatusCode, message: impl Into<Body>) -> Response<Body> {
  Response::builder()
    .status(status)
    .body(message.into())
    .unwrap()
}

/// Handles archive requests.
#[allow(clippy::implicit_hasher)]
#[allow(clippy::unused_async)]
pub async fn handle_archive_with_path(
  Path(path): Path<String>,
  Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
  eprintln!("path: {:?}", path);
  handle_archive(params.get("method"), path)
}

/// Handles archive requests.
#[allow(clippy::implicit_hasher)]
#[allow(clippy::unused_async)]
pub async fn handle_archive_without_path(
  Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
  eprintln!("without path");
  handle_archive(params.get("method"), String::new())
}

fn handle_archive(method: Option<&String>, path: String) -> impl IntoResponse {
  let method = method.map_or("tar", String::as_str);

  let Ok(archive_method) = ArchiveMethod::try_from(method) else {
    return response(
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

  eprintln!("Creating: {file_name}");

  let response = Response::builder()
    .header(
      header::CONTENT_DISPOSITION,
      HeaderValue::from_str(&format!("attachment; filename=\"{file_name}\"")).unwrap(),
    )
    .header(
      header::CONTENT_TYPE,
      HeaderValue::from_str(archive_method.mimetype()).unwrap(),
    )
    .header(
      header::TRANSFER_ENCODING,
      HeaderValue::from_str("chunked").unwrap(),
    )
    .header(
      header::CACHE_CONTROL,
      HeaderValue::from_str("no-cache").unwrap(),
    )
    .header(
      header::CONNECTION,
      HeaderValue::from_str("keep-alive").unwrap(),
    )
    .header(
      header::CONTENT_ENCODING,
      HeaderValue::from_str("identity").unwrap(),
    );

  // We will create the archive in a separate thread, and stream the content using a pipe.
  // The pipe is an adapter for async Sender to implement sync Write using block_on calls.
  let (tx, rx) = channel::<io::Result<axum::body::Bytes>>(10);
  let pipe = Pipe::new(tx);

  std::thread::spawn(move || {
    if let Err(err) = archive_method.create_archive(path, pipe) {
      log::error!("Error during archive creation: {err:?}");
    }
  });

  response.body(StreamBody::new(rx)).unwrap().into_response()
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
  while let Some(field) = multipart.next_field().await.unwrap() {
    let Some(file_name) = field.file_name() else {
      continue;
    };
    let path = get_target_dir().join(&path).join(file_name);

    println!("Uploading to {path:?}");

    let mut file = match tokio::fs::File::create(path).await {
      Ok(file) => file,
      Err(err) => {
        return response(
          StatusCode::INTERNAL_SERVER_ERROR,
          format!("Failed to create file: {err}"),
        )
        .into_response();
      }
    };

    let bytes = match field.bytes().await {
      Ok(bytes) => bytes,
      Err(e) => {
        return response(
          StatusCode::BAD_REQUEST,
          format!("Invalid file content: {e}"),
        )
        .into_response();
      }
    };

    eprintln!("Writing {} bytes", bytes.len());

    if let Err(err) = file.write_all(&bytes).await {
      return response(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Failed to write file: {err}"),
      )
      .into_response();
    }
  }

  Redirect::to(&format!("/?path={path}")).into_response()
}
