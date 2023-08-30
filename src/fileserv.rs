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
  body::{boxed, Body, StreamBody},
  extract::{Path, Query, State},
  http::{Request, Response, StatusCode, Uri},
  response::{IntoResponse, Response as AxumResponse},
};
use tokio::io;

pub use crate::fileserv::archive::ArchiveMethod;
use crate::{app::App, config::get_target_dir, fileserv::pipe::Pipe};

/// Handles static file requests by delegating to `StaticFiles`.
pub async fn file_and_error_handler(
  uri: Uri,
  State(options): State<LeptosOptions>,
  request: Request<Body>,
) -> AxumResponse {
  let response = {
    let uri = &uri;
    let path = uri.path().trim_start_matches('/');

    // log!("File request: {path:?}");

    let builder = Response::builder();

    let res = if let Some(file) = StaticFiles::get(path) {
      builder
        .status(StatusCode::OK)
        .header(
          header::CONTENT_TYPE,
          HeaderValue::from_str(file.metadata.mimetype()).unwrap(),
        )
        .body(Body::from(file.data))
    } else {
      builder
        .status(StatusCode::NOT_FOUND)
        .body(Body::from(format!("File {path} not found")))
    };

    res.unwrap().map(boxed)
  };

  // log!("File response: {}", response.status());

  if response.status() == StatusCode::OK {
    response.into_response()
  } else {
    let handler = leptos_axum::render_app_to_stream(options.clone(), App);
    handler(request).await.into_response()
  }
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
    return Response::builder()
      .status(StatusCode::BAD_REQUEST)
      .body(Body::from("Invalid archive method"))
      .unwrap()
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

  response.body(StreamBody::new(rx)).unwrap().map(boxed)
}
