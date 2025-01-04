use std::{net::IpAddr, path::PathBuf};

use cfg_if::cfg_if;
use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
  /// Target directory to share
  #[arg(default_value = ".")]
  pub target_dir: PathBuf,

  /// Port to listen on
  #[arg(short, long, default_value = "3000")]
  pub port: u16,

  /// Show QR codes that link to the site
  #[arg(short, long)]
  pub qr: bool,

  /// IP address(es) on which file-share will be available
  ///
  /// Accepts comma separated list of both IPv4 and IPv6 addresses
  #[arg(short, long, num_args = 1.., value_delimiter = ',', default_value = "0.0.0.0,::")]
  pub interfaces: Vec<IpAddr>,

  /// Use a file picker to choose a target directory
  #[arg(short = 'P', long, default_value = "false")]
  pub picker: bool,
}

#[derive(Debug, Clone)]
pub struct Config {
  pub target_dir: PathBuf,
  pub port: u16,
  pub qr: bool,
  pub interfaces: Vec<IpAddr>,
}

/// Get the config from CLI arguments.
///
/// # Errors
///
/// Returns error if `CWD`/`target_dir` is unreadable or when there's no free
/// port.
///
/// # Panics
///
/// Panics if the current working directory is invalid or unreadable for current
/// process.
#[allow(clippy::unused_async)] // it's used only in release build
pub async fn get_config() -> Result<Config, String> {
  cfg_if! {
    if #[cfg(debug_assertions)] {
      use std::net::{Ipv4Addr, Ipv6Addr};

      let target_dir = std::env::current_dir().expect("CWD is a valid path").join("files");
      let port = 3000;
      let qr = false;

      let interfaces = vec![
        IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)),
        IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
      ];
    } else {
      let Cli { target_dir, port, qr, interfaces, picker } = Cli::parse();
      let target_dir = if picker {
        rfd::AsyncFileDialog::new()
          .set_title("Select directory to share")
          .pick_folder()
          .await
          .ok_or("No directory selected")?
          .path()
          .to_path_buf()
      } else {
        target_dir.canonicalize().map_err(|e| e.to_string())?
      };
    }
  }

  let port = (port != 0 && port_check::is_local_port_free(port))
    .then_some(port)
    .or_else(port_check::free_local_port)
    .ok_or("Couldn't find an open port")?;

  Ok(Config {
    target_dir,
    port,
    qr,
    interfaces,
  })
}
