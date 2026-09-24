//! Socket binding and per-address serving tasks.
//!
//! Split out of `main.rs` — transport only, no routing or display.

use std::{
    io,
    net::{SocketAddr, TcpListener},
    process,
};

use axum::Router;
use axum_server::{Handle, bind, from_tcp};
use leptos::logging::error;
use socket2::{Domain, Socket, Type};
use tokio::task::JoinSet;

#[must_use]
pub fn is_v6_wildcard(addr: &SocketAddr) -> bool {
    matches!(addr.ip(), std::net::IpAddr::V6(ip) if ip.is_unspecified())
}

/// Pre-bind the IPv6 wildcard on a v6-only socket.
///
/// A dual-stack wildcard would clash with the IPv4 wildcard on Linux
/// (`EADDRINUSE`), so the v6 socket is pinned to v6-only: both wildcards
/// then always coexist, and any bind failure is genuine.
///
/// # Errors
///
/// Returns the underlying socket `io::Error` when creation, option-setting,
/// binding or listening fails.
pub fn bind_v6_wildcard(addr: SocketAddr) -> io::Result<TcpListener> {
    let socket = Socket::new(Domain::IPV6, Type::STREAM, None)?;
    socket.set_only_v6(true)?;
    socket.set_reuse_address(true)?;
    socket.bind(&addr.into())?;
    socket.listen(1024)?;
    socket.set_nonblocking(true)?;
    Ok(socket.into())
}

pub fn serve_address(
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
                .serve(app.into_make_service_with_connect_info::<SocketAddr>())
                .await
                .map_err(|e| format!("Failed to serve at {addr}: {e}"))
        });
    } else {
        servers.spawn(async move {
            bind(addr)
                .handle(handle)
                .serve(app.into_make_service_with_connect_info::<SocketAddr>())
                .await
                .map_err(|e| format!("Failed to start server at {addr}: {e}"))
        });
    }
}
