use std::{net::IpAddr, path::PathBuf};

use clap::{ArgAction, Parser};
use port_check::{free_local_port, is_local_port_free};
use rfd::AsyncFileDialog;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Path to the directory to share
    #[arg(default_value = ".")]
    pub target_dir: PathBuf,

    /// Port to listen on
    ///
    /// Use `0` to auto-pick a free port; any other busy port is an error.
    #[arg(short, long, default_value = "18765")]
    pub port: u16,

    /// Show QR codes that link to the site
    #[arg(short, long)]
    pub qr: bool,

    /// IP address(es) of interfaces on which file-share will be available
    ///
    /// Accepts comma separated list of both IPv4 and IPv6 addresses.
    /// `0.0.0.0`/`::` listen on all interfaces (IPv6 served on a v6-only
    /// socket so both wildcards coexist on dual-stack systems)
    #[arg(short, long, num_args = 1.., value_delimiter = ',', default_value = "0.0.0.0,::")]
    pub interfaces: Vec<IpAddr>,

    /// Open a GUI file picker to choose the target directory
    ///
    /// Overrides `TARGET_DIR`
    #[arg(short = 'P', long, action = ArgAction::SetTrue)]
    pub picker: bool,

    /// Allow client to upload files
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub upload: bool,

    /// Require a shared secret for every request
    ///
    /// Clients send it as `Authorization: Bearer <TOKEN>` (curl) or log in
    /// once via the browser form at `/login` (sets a cookie, so the web UI
    /// keeps working). Disabled when absent.
    #[arg(long, value_name = "TOKEN")]
    pub auth_token: Option<String>,

    /// Max sustained requests per second per client IP (burst = same value)
    ///
    /// Excess requests get `429 Too Many Requests`. Disabled when absent.
    #[arg(long, value_name = "RPS")]
    pub rate_limit: Option<u32>,

    /// Max request body size, e.g. `100MB`, `1GB`
    ///
    /// Bounds `/upload` and the browser upload endpoint; without it request
    /// bodies are unlimited (back-compat).
    #[arg(long, value_name = "SIZE", value_parser = parse_size)]
    pub max_upload_size: Option<u64>,

    /// Max total bytes in one generated archive, e.g. `2GB`
    ///
    /// The archive stream aborts once exceeded. Disabled when absent.
    #[arg(long, value_name = "SIZE", value_parser = parse_size)]
    pub max_archive_size: Option<u64>,

    /// Max directory depth included in archives
    ///
    /// Deeper trees are rejected before streaming starts. Disabled when absent.
    #[arg(long, value_name = "N")]
    pub max_archive_depth: Option<usize>,

    /// Max seconds spent generating one archive
    ///
    /// The stream aborts once exceeded. Disabled when absent.
    #[arg(long, value_name = "SECS")]
    pub archive_timeout: Option<u64>,
}

/// Parse a byte size like `1024`, `100MB`, `1.5GB` (case-insensitive).
///
/// Exact integer math (up to 6 fractional digits), so `1.5GB` is precisely
/// `1610612736` with no float rounding.
///
/// # Errors
///
/// Returns a message when the value has no numeric prefix, has an unknown
/// suffix, or overflows `u64`.
pub fn parse_size(input: &str) -> Result<u64, String> {
    let invalid = || format!("Invalid size: '{input}'");
    let input = input.trim();
    let split = input
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(input.len());
    let (number, suffix) = input.split_at(split);
    let multiplier: u128 = match suffix.trim().to_ascii_uppercase().as_str() {
        "" | "B" => 1,
        "K" | "KB" => 1024,
        "M" | "MB" => 1024 * 1024,
        "G" | "GB" => 1024 * 1024 * 1024,
        "T" | "TB" => 1024 * 1024 * 1024 * 1024,
        _ => return Err(format!("Unknown size suffix in '{input}'")),
    };
    let (whole, frac) = match number.split_once('.') {
        Some((whole, frac)) => (whole, frac),
        None => (number, ""),
    };
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !frac.bytes().all(|b| b.is_ascii_digit())
        || frac.len() > 6
    {
        return Err(invalid());
    }
    let whole: u128 = whole.parse().map_err(|_| invalid())?;
    // Fixed-point fraction scaled to micros.
    let mut frac_value: u128 = if frac.is_empty() {
        0
    } else {
        frac.parse().map_err(|_| invalid())?
    };
    for _ in frac.len()..6 {
        frac_value *= 10;
    }
    whole
        .checked_mul(multiplier)
        .and_then(|base| {
            frac_value
                .checked_mul(multiplier)
                .map(|extra| base + extra / 1_000_000)
        })
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or_else(invalid)
}

#[derive(Debug, Clone)]
pub struct Config {
    pub target_dir: PathBuf,
    pub allow_upload: bool,
    pub port: u16,
    pub qr: bool,
    pub interfaces: Vec<IpAddr>,
    pub auth_token: Option<String>,
    pub rate_limit: Option<u32>,
    pub max_upload_size: Option<u64>,
    pub max_archive_size: Option<u64>,
    pub max_archive_depth: Option<usize>,
    pub archive_timeout: Option<u64>,
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
    let Cli {
        target_dir,
        port,
        qr,
        interfaces,
        picker,
        upload,
        auth_token,
        rate_limit,
        max_upload_size,
        max_archive_size,
        max_archive_depth,
        archive_timeout,
    } = Cli::parse();

    let target_dir = if picker {
        AsyncFileDialog::new()
            .set_title("Select directory to share")
            .pick_folder()
            .await
            .ok_or("No directory selected")?
            .path()
            .to_path_buf()
    } else {
        target_dir
    };

    let canonical_target_dir = target_dir.canonicalize().map_err(|e| e.to_string())?;

    if !canonical_target_dir.is_dir() {
        return Err(format!("`{}` is not a directory", target_dir.display()));
    }

    let port = if port == 0 {
        free_local_port().ok_or("Couldn't find an open port")?
    } else if is_local_port_free(port) {
        port
    } else {
        return Err(format!("Port {port} is already in use"));
    };

    if let Some(0) = rate_limit {
        return Err("--rate-limit must be at least 1".to_string());
    }

    Ok(Config {
        target_dir: canonical_target_dir,
        allow_upload: upload,
        port,
        qr,
        interfaces,
        auth_token,
        rate_limit,
        max_upload_size,
        max_archive_size,
        max_archive_depth,
        archive_timeout,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_size;

    #[test]
    fn parses_byte_sizes() {
        assert_eq!(parse_size("1024"), Ok(1024));
        assert_eq!(parse_size("100MB"), Ok(100 * 1024 * 1024));
        assert_eq!(parse_size("1.5gb"), Ok(1_610_612_736));
        assert_eq!(parse_size("2K"), Ok(2048));
        assert_eq!(parse_size("1T"), Ok(1024 * 1024 * 1024 * 1024));
        assert!(parse_size("10XB").is_err());
        assert!(parse_size("abc").is_err());
    }
}
