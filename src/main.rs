#![warn(clippy::pedantic)]

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
  use std::{
    fs::create_dir_all,
    io,
    net::{IpAddr, SocketAddr},
    process,
  };

  use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router, Server,
  };
  use colored::Colorize;
  use file_share::{
    app::App,
    config::{get_config, AppState, Cli},
    fileserv::{
      file_and_error_handler, file_upload_with_path, file_upload_without_path,
      handle_archive_with_path, handle_archive_without_path,
    },
  };
  use if_addrs::Interface;
  use leptos::{get_configuration, logging::error};
  use leptos_axum::{generate_route_list, LeptosRoutes};
  use tower_http::services::ServeDir;

  simple_logger::init_with_level(log::Level::Warn).expect("couldn't initialize logging");

  let conf = get_configuration(None).await.unwrap();
  let leptos_options = conf.leptos_options;
  let routes = generate_route_list(App);

  let Cli {
    target_dir,
    port,
    qr,
    interfaces,
  } = get_config();

  let app_state = AppState {
    leptos_options,
    target_dir: target_dir.clone(),
  };

  let app = Router::new()
    .route("/api/*fn_name", post(leptos_axum::handle_server_fns))
    .route("/archive/*path", get(handle_archive_with_path))
    .route("/archive/", get(handle_archive_without_path))
    .route("/upload/*path", post(file_upload_with_path))
    .route("/upload/", post(file_upload_without_path))
    .layer(DefaultBodyLimit::disable())
    .leptos_routes(&app_state, routes, App)
    .nest_service("/files", ServeDir::new(&target_dir))
    .fallback(file_and_error_handler)
    .with_state(app_state);

  let socket_addresses = interfaces
    .iter()
    .map(|&interface| SocketAddr::new(interface, port))
    .collect::<Vec<_>>();

  let display_urls = {
    let (wildcard, mut ifaces): (Vec<IpAddr>, Vec<IpAddr>) =
      interfaces.into_iter().partition(IpAddr::is_unspecified);

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
  };

  let display_sockets = socket_addresses
    .iter()
    .map(ToString::to_string)
    .collect::<Vec<_>>()
    .join(", ");

  let server = socket_addresses
    .iter()
    .map(Server::try_bind)
    .find_map(|result| {
      result
        .map_err(|e| error!("Failed to bind to socket: {e}"))
        .ok()
    });

  let Some(server) = server else {
    error!("Failed to bind to any socket");
    process::exit(1);
  };

  if let Err(e) = create_dir_all(&target_dir) {
    error!("Failed to create target directory: {e}");
    process::exit(1);
  }

  println!(
    "Serving files from {}",
    target_dir.to_string_lossy().yellow().bold()
  );
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
    for url in display_urls
      .iter()
      .filter(|url| !url.contains("127.0.0.1") && !url.contains("[::1]"))
    {
      match qr_code::QrCode::new(url) {
        Ok(qr) => {
          println!("QR code for {}:", url.green().bold());
          println!("{}", qr.to_string(false, 1));
        }
        Err(e) => {
          error!("Failed to render QR to terminal: {e}");
        }
      };
    }
  }

  if is_terminal {
    println!("Quit by pressing CTRL-C");
  }

  server.serve(app.into_make_service()).await.unwrap();
}
