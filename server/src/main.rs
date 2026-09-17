#![warn(clippy::pedantic)]
#![recursion_limit = "256"]

pub mod config;
pub mod fileserv;

use std::{
    fs::create_dir_all,
    io,
    net::{IpAddr, SocketAddr},
    process,
    sync::Arc,
};

use axum::{
    Router,
    extract::DefaultBodyLimit,
    response::Redirect,
    routing::{get, post},
};
use axum_server::Handle;
use colored::Colorize;
use file_share_app::{App, AppConfig, AppState, shell};
use if_addrs::Interface;
use leptos::{
    logging::{error, warn},
    prelude::{get_configuration, provide_context},
};
use leptos_axum::{AxumRouteListing, LeptosRoutes, generate_route_list};
use tokio::task::JoinSet;
use tower_http::{
    compression::{
        CompressionLayer,
        predicate::{DefaultPredicate, NotForContentType, Predicate as _},
    },
    services::ServeDir,
};

use crate::{
    config::{Config, get_config},
    fileserv::{
        file_and_error_handler, file_upload_with_path, file_upload_without_path,
        handle_archive_with_path, handle_archive_without_path,
    },
};

const API_HELP_TEXT: &str = r"
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

Available methods are tar, tar.gz, tar.zst, zip.
";

#[tokio::main]
async fn main() {
    let conf = get_configuration(None).unwrap_or_else(|e| {
        eprintln!("Failed to load Leptos configuration: {e}");
        process::exit(1);
    });
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    let cli_config = get_config().await.unwrap_or_else(|e| {
        eprintln!("Failed to get CLI config: {e}");
        process::exit(1);
    });

    let Config {
        target_dir,
        port,
        qr,
        interfaces,
        allow_upload,
    } = cli_config;

    let app_config = Arc::new(AppConfig {
        target_dir: target_dir.clone(),
        allow_upload,
    });

    let app_state = AppState {
        app_config: Arc::clone(&app_config),
        leptos_options: Arc::new(leptos_options),
    };

    if let Err(e) = create_dir_all(&target_dir) {
        error!("Failed to create target directory: {e}");
        process::exit(1);
    }

    println!(
        "Serving files from {}",
        target_dir.to_string_lossy().yellow().bold()
    );

    let app = create_router(app_state.clone(), routes);

    let display_urls = get_display_urls(&interfaces, port);

    let socket_addresses = interfaces
        .iter()
        .map(|&interface| SocketAddr::new(interface, port))
        .collect::<Vec<_>>();

    let display_sockets = socket_addresses
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");

    println!("Listening on {display_sockets}");
    println!(
        "Available on:\n{}",
        display_urls
            .iter()
            .map(|url| format!("   {url}").green().bold().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );

    let is_terminal = io::IsTerminal::is_terminal(&io::stdout());

    if qr && is_terminal {
        print_qr_codes(&display_urls);
    }

    if is_terminal {
        println!("Quit by pressing CTRL-C");
    }

    let handle = Handle::new();
    let shutdown_handle = handle.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            shutdown_handle.shutdown();
        }
    });

    let mut join_set = JoinSet::new();
    for addr in socket_addresses {
        let app = app.clone();
        let handle = handle.clone();
        join_set.spawn(async move {
            axum_server::bind(addr)
                .handle(handle)
                .serve(app.into_make_service())
                .await
                .map_err(|e| format!("Failed to start server at {addr}: {e}"))
        });
    }

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(())) => {},
            Ok(Err(e)) => error!("{e}"),
            Err(e) => error!("Server task failed: {e}"),
        }
    }
}

fn create_router(app_state: AppState, routes: Vec<AxumRouteListing>) -> Router {
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

    // NOTE: `Router::layer` only wraps routes registered *before* it, so
    // compression applies solely to the UI/API routes above it.
    Router::new()
        .route("/", get(|| async { Redirect::to("/index") }))
        .route("/help", get(|| async { API_HELP_TEXT }))
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
        .layer(compression)
        .route("/archive/{*path}", get(handle_archive_with_path))
        .route("/archive/", get(handle_archive_without_path))
        .route("/upload/{*path}", post(file_upload_with_path))
        .route("/upload/", post(file_upload_without_path))
        .nest_service("/files", ServeDir::new(&target_dir))
        .layer(DefaultBodyLimit::disable())
        .with_state(app_state)
}

fn print_qr_codes(display_urls: &[String]) {
    for url in display_urls
        .iter()
        .filter(|url| !url.contains("127.0.0.1") && !url.contains("[::1]"))
    {
        match qr_code::QrCode::new(url) {
            Ok(qr) => {
                println!(
                    "QR code for {}:\n{}",
                    url.green().bold(),
                    qr.to_string(false, 1)
                );
            },
            Err(e) => {
                error!("Failed to render QR to terminal: {e}");
                break;
            },
        }
    }
}

fn get_display_urls(interfaces: &[IpAddr], port: u16) -> Vec<String> {
    let (wildcard, mut ifaces): (Vec<IpAddr>, Vec<IpAddr>) =
        interfaces.iter().copied().partition(IpAddr::is_unspecified);

    // Replace wildcard addresses with local interface addresses
    if !wildcard.is_empty() {
        let all_ipv4 = wildcard.iter().any(IpAddr::is_ipv4);
        let all_ipv6 = wildcard.iter().any(IpAddr::is_ipv6);

        ifaces = if_addrs::get_if_addrs()
            .map_err(|e| error!("Failed to get local interface addresses: {e}"))
            .unwrap_or_default()
            .iter()
            .map(Interface::ip)
            .filter(|ip| (all_ipv4 && ip.is_ipv4()) || (all_ipv6 && ip.is_ipv6()))
            .collect();

        ifaces.sort_unstable();
    }

    ifaces
        .into_iter()
        .map(|addr| match addr {
            IpAddr::V4(_) => format!("{addr}"),
            IpAddr::V6(_) => format!("[{addr}]"),
        })
        .map(|url| format!("http://{url}:{port}"))
        .collect::<Vec<_>>()
}
