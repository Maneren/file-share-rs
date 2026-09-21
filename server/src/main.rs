#![warn(clippy::pedantic)]
#![recursion_limit = "256"]

pub mod config;
pub mod fileserv;

use std::{
    fs::create_dir_all,
    io::{self, IsTerminal},
    net::{IpAddr, SocketAddr, TcpListener},
    process,
    sync::Arc,
};

use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderValue, header},
    middleware,
    response::Redirect,
    routing::{get, post},
};
use axum_server::{Handle, bind, from_tcp};
use colored::Colorize;
use file_share_app::{App, AppConfig, AppState, shell};
use if_addrs::{Interface, get_if_addrs};
use leptos::{
    logging::{error, warn},
    prelude::{get_configuration, provide_context},
};
use leptos_axum::{AxumRouteListing, LeptosRoutes, generate_route_list};
use qr_code::QrCode;
use socket2::{Domain, Socket, Type};
use tokio::{signal, spawn, task::JoinSet};
use tower::Layer as _;
use tower_http::{
    compression::{
        CompressionLayer,
        predicate::{DefaultPredicate, NotForContentType, Predicate as _},
    },
    services::ServeDir,
    set_header::SetResponseHeaderLayer,
};

use crate::{
    config::{Config, get_config},
    fileserv::{
        file_and_error_handler, file_upload_with_path, file_upload_without_path, gate_shared_files,
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

    let targets = display_targets(&interfaces, port);

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
        targets
            .iter()
            .map(|target| format!("   {}", target.url).green().bold().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );

    let is_terminal = IsTerminal::is_terminal(&io::stdout());

    if qr && is_terminal {
        print_qr_codes(&targets);
    }

    if is_terminal {
        println!("Quit by pressing CTRL-C");
    }

    let handle = Handle::new();
    let shutdown_handle = handle.clone();
    spawn(async move {
        if signal::ctrl_c().await.is_ok() {
            shutdown_handle.shutdown();
        }
    });

    let mut join_set = JoinSet::new();
    for addr in socket_addresses {
        serve_address(&mut join_set, app.clone(), handle.clone(), addr);
    }

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(())) => continue,
            Ok(Err(e)) => error!("{e}"),
            Err(e) => error!("Server task failed: {e}"),
        }
        process::exit(1);
    }
}

fn serve_address(
    servers: &mut JoinSet<Result<(), String>>,
    app: Router,
    handle: Handle<SocketAddr>,
    addr: SocketAddr,
) {
    if is_v6_wildcard(&addr) {
        // Pre-bound v6-only socket (see `bind_v6_wildcard`); the bind is
        // synchronous so a failure here is reported before serving.
        let listener = match bind_v6_wildcard(addr) {
            Ok(listener) => listener,
            Err(e) => {
                error!("Failed to bind server socket at {addr}: {e}");
                process::exit(1);
            },
        };
        let server = match from_tcp(listener) {
            Ok(server) => server,
            Err(e) => {
                error!("Failed to start server at {addr}: {e}");
                process::exit(1);
            },
        };
        servers.spawn(async move {
            server
                .handle(handle)
                .serve(app.into_make_service())
                .await
                .map_err(|e| format!("Failed to serve at {addr}: {e}"))
        });
    } else {
        servers.spawn(async move {
            bind(addr)
                .handle(handle)
                .serve(app.into_make_service())
                .await
                .map_err(|e| format!("Failed to start server at {addr}: {e}"))
        });
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
        .layer(DefaultBodyLimit::disable())
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

/// Interface-name prefixes of virtual container bridges (Docker, libvirt,
/// veth pairs). Their addresses are never reachable from other machines,
/// so they are skipped when expanding wildcard binds for display.
const VIRTUAL_IFACE_PREFIXES: &[&str] = &["docker", "veth", "br-", "virbr"];

/// Unicast scopes with no usable route for LAN clients (ARP/ND link-local).
fn is_link_local(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_unicast_link_local(),
    }
}

/// Addresses worth showing to the user: drops loopback, link-local,
/// multicast and unspecified addresses.
fn is_usable_address(ip: &IpAddr) -> bool {
    !(ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() || is_link_local(ip))
}

fn is_v6_wildcard(addr: &SocketAddr) -> bool {
    matches!(addr.ip(), IpAddr::V6(ip) if ip.is_unspecified())
}

/// Pre-bind the IPv6 wildcard on a v6-only socket.
///
/// A dual-stack wildcard would clash with the IPv4 wildcard on Linux
/// (`EADDRINUSE`), so the v6 socket is pinned to v6-only: both wildcards
/// then always coexist, and any bind failure is genuine.
fn bind_v6_wildcard(addr: SocketAddr) -> io::Result<TcpListener> {
    let socket = Socket::new(Domain::IPV6, Type::STREAM, None)?;
    socket.set_only_v6(true)?;
    socket.set_reuse_address(true)?;
    socket.bind(&addr.into())?;
    socket.listen(1024)?;
    socket.set_nonblocking(true)?;
    Ok(socket.into())
}

/// A served address plus its printable URL.
struct DisplayTarget {
    ip: IpAddr,
    url: String,
}

fn format_url(ip: IpAddr, port: u16) -> String {
    match ip {
        IpAddr::V4(_) => format!("http://{ip}:{port}"),
        IpAddr::V6(_) => format!("http://[{ip}]:{port}"),
    }
}

/// Concrete addresses to show: explicitly configured addresses as-is, plus
/// usable LAN addresses discovered for wildcards (deduped).
fn display_targets(interfaces: &[IpAddr], port: u16) -> Vec<DisplayTarget> {
    let (wildcards, explicit): (Vec<IpAddr>, Vec<IpAddr>) =
        interfaces.iter().copied().partition(IpAddr::is_unspecified);

    let mut targets: Vec<DisplayTarget> = explicit
        .into_iter()
        .map(|ip| DisplayTarget {
            ip,
            url: format_url(ip, port),
        })
        .collect();

    // Replace wildcard addresses with usable local interface addresses.
    if !wildcards.is_empty() {
        let want_v4 = wildcards.iter().any(IpAddr::is_ipv4);
        let want_v6 = wildcards.iter().any(IpAddr::is_ipv6);

        match get_if_addrs() {
            Ok(ifaces) => {
                let mut found: Vec<IpAddr> = ifaces
                    .iter()
                    .filter(|iface| {
                        let ip = iface.ip();
                        ((want_v4 && ip.is_ipv4()) || (want_v6 && ip.is_ipv6()))
                            && is_usable_address(&ip)
                            && !VIRTUAL_IFACE_PREFIXES
                                .iter()
                                .any(|prefix| iface.name.starts_with(prefix))
                    })
                    .map(Interface::ip)
                    .collect();
                found.sort_unstable();
                found.dedup();
                found.retain(|ip| !targets.iter().any(|target| target.ip == *ip));
                targets.extend(found.into_iter().map(|ip| DisplayTarget {
                    ip,
                    url: format_url(ip, port),
                }));
            },
            Err(e) => {
                warn!("Failed to list network interfaces, showing configured addresses only: {e}");
            },
        }
    }

    targets
}

fn print_qr_codes(targets: &[DisplayTarget]) {
    for target in targets {
        // Loopback/link-local addresses are useless on another device.
        if !is_usable_address(&target.ip) {
            continue;
        }
        match QrCode::new(&target.url) {
            Ok(qr) => {
                println!(
                    "\n QR code for {}:\n{}",
                    target.url.green().bold(),
                    qr.to_string(false, 1)
                );
            },
            // A single bad address must not hide the rest; only a failed
            // bind is fatal.
            Err(e) => warn!("Failed to render QR code for {}: {e}", target.url),
        }
    }
}
