use std::{net::IpAddr, path::PathBuf, sync::OnceLock};

use cfg_if::cfg_if;
use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
  /// Target directory to serve
  #[arg(default_value = ".")]
  pub target_dir: PathBuf,

  /// Port to serve on
  #[arg(short, long, default_value = "3000")]
  pub port: u16,

  /// Show QR code with link to the site
  #[arg(short, long)]
  pub qr: bool,

  /// IP address(es) on which file-share will be available
  #[arg(short, long, num_args = 1.., value_delimiter = ',', default_value = "0.0.0.0,::")]
  pub interfaces: Vec<IpAddr>,
}

/// Get the config from CLI arguments.
///
/// # Panics
///
/// Panics if `CWD`/`target_dir` is unreadable or when there's no free port.
pub fn get_config() -> Cli {
  cfg_if! {
    if #[cfg(debug_assertions)] {
      use std::net::{Ipv4Addr, Ipv6Addr};

      let target_dir = std::env::current_dir().unwrap().join("files");
      let port = 3000;
      let qr = false;

      let interfaces = vec![
        IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)),
        IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
      ];
    } else {
      let Cli { target_dir, port, qr, interfaces } = Cli::parse();
      let target_dir = target_dir.canonicalize().expect("target_dir is a valid path");
    }
  }

  let port = if port == 0 || !port_check::is_local_port_free(port) {
    port_check::free_local_port().expect("no free port found")
  } else {
    port
  };
  let _ = TARGET_DIR.set(target_dir.clone());

  Cli {
    target_dir,
    port,
    qr,
    interfaces,
  }
}

pub static TARGET_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Get the target directory from config.
///
/// # Panics
///
/// Panics if `TARGET_DIR` isn't initialized.
pub fn get_target_dir() -> &'static PathBuf {
  TARGET_DIR.get().unwrap()
}

use axum::extract::FromRef;
use leptos::LeptosOptions;

/// This takes advantage of Axum's `SubStates` feature by deriving `FromRef`. This is the only way to have more than one
/// item in Axum's State. Leptos requires you to have leptosOptions in your State struct for the leptos route handlers
#[derive(FromRef, Debug, Clone)]
pub struct AppState {
  pub leptos_options: LeptosOptions,
  pub target_dir: PathBuf,
}
