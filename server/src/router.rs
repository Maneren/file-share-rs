//! Axum router: Leptos routes, archive/upload APIs, static fallback.
//!
//! Split out of `main.rs` — all HTTP wiring lives here.

use std::sync::Arc;

use axum::{
    Router,
    http::{HeaderValue, header},
    middleware,
    response::Redirect,
    routing::{get, post},
};
use file_share_app::shell;
use leptos::prelude::provide_context;
use leptos_axum::{AxumRouteListing, LeptosRoutes};
use tower::Layer as _;
use tower_http::{
    catch_panic::CatchPanicLayer,
    compression::{
        CompressionLayer,
        predicate::{DefaultPredicate, NotForContentType, Predicate as _},
    },
    limit::RequestBodyLimitLayer,
    services::ServeDir,
    set_header::SetResponseHeaderLayer,
};

use crate::{
    fileserv::{
        file_and_error_handler, file_upload_with_path, file_upload_without_path, gate_shared_files,
        handle_archive_with_path, handle_archive_without_path,
    },
    security::{login, rate_limit, require_auth},
    state::AppState,
};

pub const API_HELP_TEXT: &str = r"
File Share
===========
Endpoints:
- /help                         -- show this help text
- /api/list_dir path=           -- list the contents of a directory
- /api/new_folder name=&target= -- create a new folder with name in path
- /archive/*path?method=        -- create an archive from a path
- /archive?method=              -- create an archive from root directory
- /upload/*path                 -- upload a file to a path
- /upload                       -- upload a file to root directory
- /login                        -- browser login form (only with --auth-token)

Available methods are tar, tar.gz, tar.zst, zip.
";

pub fn create_router(app_state: AppState, routes: Vec<AxumRouteListing>) -> Router {
    // Compress only responses that actually benefit from it.
    let compression_predicate = DefaultPredicate::new()
        // skip upload progress
        .and(NotForContentType::new("application/octet-stream"))
        // skip already-compressed payloads
        .and(NotForContentType::new("application/zip"))
        .and(NotForContentType::new("application/gzip"))
        .and(NotForContentType::new("application/zstd"))
        .and(NotForContentType::new("application/x-tar"))
        // skip generally incompressible payloads
        .and(NotForContentType::new("video/"))
        .and(NotForContentType::new("audio/"));
    let compression = CompressionLayer::new().compress_when(compression_predicate);

    let app_config = Arc::clone(&app_state.app_config);
    let target_dir = app_state.app_config.target_dir.clone();

    // Bounds `/upload` bodies and the browser upload server-fn; unlimited
    // without `--max-upload-size` (back-compat).
    let body_limit = app_state
        .security
        .max_upload_size
        .and_then(|max| usize::try_from(max).ok())
        .unwrap_or(usize::MAX);

    Router::new()
        .route("/", get(|| async { Redirect::to("/index") }))
        .route("/help", get(|| async { API_HELP_TEXT }))
        // POST verifies the token; GET renders the login page
        // (registered with the Leptos routes below, merged by Axum).
        .route("/login", post(login))
        .leptos_routes_with_context(
            &app_state,
            routes,
            move || provide_context(Arc::clone(&app_config)),
            {
                let leptos_options = Arc::clone(&app_state.leptos_options);
                move || shell((*leptos_options).clone())
            },
        )
        .fallback(file_and_error_handler)
        .route("/archive/{*path}", get(handle_archive_with_path))
        .route("/archive/", get(handle_archive_without_path))
        .route("/upload/{*path}", post(file_upload_with_path))
        .route("/upload/", post(file_upload_without_path))
        .nest_service(
            "/files",
            middleware::from_fn_with_state(app_state.clone(), gate_shared_files)
                .layer(ServeDir::new(&target_dir)),
        )
        .layer(compression)
        .layer(RequestBodyLimitLayer::new(body_limit))
        // A panicking handler becomes `500` instead of a hung connection.
        .layer(CatchPanicLayer::new())
        // Shed floods before auth; no-op without `--rate-limit`.
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            rate_limit,
        ))
        // Reject unauthenticated requests before any body is read; no-op
        // without `--auth-token`.
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            require_auth,
        ))
        // No CSP: Leptos hydration relies on inline scripts, which a
        // `script-src` policy without `unsafe-inline` would block.
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("SAMEORIGIN"),
        ))
        .with_state(app_state)
}
