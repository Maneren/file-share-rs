//! Binary entry: config, state, router, per-address serving.
//!
//! HTTP wiring lives in [`router`], socket work in [`serve`],
//! address display in [`net`], CLI parsing in [`cli`],
//! file/archive/upload handlers in [`fileserv`].

#![warn(clippy::pedantic)]
#![recursion_limit = "256"]

pub mod cli;
pub mod fileserv;
pub mod net;
pub mod router;
pub mod serve;
pub mod state;

use std::{fs::create_dir_all, io::IsTerminal, net::SocketAddr, process, sync::Arc};

use axum_server::Handle;
use colored::Colorize;
use file_share_app::{App, AppConfig};
use leptos::{logging::error, prelude::get_configuration};
use leptos_axum::generate_route_list;
use tokio::{signal, spawn, task::JoinSet};

use crate::{
    cli::{Config, get_config},
    net::{display_targets, print_qr_codes},
    router::create_router,
    serve::serve_address,
    state::AppState,
};

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

    let is_terminal = IsTerminal::is_terminal(&std::io::stdout());

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
