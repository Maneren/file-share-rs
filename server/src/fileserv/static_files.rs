//! Embedded frontend assets + Leptos fallback.
//!
//! Serves files from the compiled `target/site` bundle with `ETag` support,
//! falling back to the Leptos app for unknown paths.

use std::fmt::Write as _;

use axum::{
    body::Body,
    extract::State,
    http::{HeaderValue, Request, Response, StatusCode, Uri, header},
    response::IntoResponse,
};
use file_share_app::shell;
use leptos::{logging, prelude::provide_context};
use rust_embed::{EmbeddedFile, RustEmbed};

use crate::state::AppState;

#[derive(RustEmbed)]
#[folder = "../target/site"]
struct StaticFiles;

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
