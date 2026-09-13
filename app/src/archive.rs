//! Archive methods shared between the server and the web UI.
//!
//! Kept dependency-free on purpose so it compiles for both the SSR server
//! target and the WASM frontend target. The (tokio-based) code that actually
//! writes archives lives in the server crate.

use std::fmt;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    #[default]
    Tar,
    TarGz,
    TarZstd,
    Zip,
}

impl Method {
    /// Every archive method the server can produce. The folder-download
    /// dropdown renders from this list so the UI can never drift out of sync.
    pub const ALL: [Method; 4] = [Method::Tar, Method::TarGz, Method::TarZstd, Method::Zip];

    /// URL query value and file extension, e.g. `tar.zst`.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Tar => "tar",
            Method::TarGz => "tar.gz",
            Method::TarZstd => "tar.zst",
            Method::Zip => "zip",
        }
    }

    #[must_use]
    pub fn mimetype(&self) -> &'static str {
        match self {
            Method::Tar => "application/x-tar",
            Method::TarGz => "application/gzip",
            Method::TarZstd => "application/zstd",
            Method::Zip => "application/zip",
        }
    }

    /// Wire compression the client must undo with `DecompressionStream`, or
    /// `None` when the format cannot be stream-extracted in the browser (zip
    /// keeps its central directory at the end of the file).
    #[must_use]
    pub fn wire_compression(&self) -> Option<&'static str> {
        match self {
            Method::Tar => Some("none"),
            Method::TarGz => Some("gzip"),
            Method::TarZstd => Some("zstd"),
            Method::Zip => None,
        }
    }

    #[must_use]
    pub fn is_streamable(&self) -> bool {
        self.wire_compression().is_some()
    }

    /// Flags for extracting this archive with CLI `tar` over a curl pipe,
    /// e.g. `curl … | tar --zstd -x -C .`. `None` for non-tar formats (zip),
    /// which cannot be extracted from a pipe — download the file instead.
    #[must_use]
    pub fn tar_extract_flags(&self) -> Option<&'static str> {
        match self {
            Method::Tar => Some("-x"),
            Method::TarGz => Some("-xz"),
            Method::TarZstd => Some("--zstd -x"),
            Method::Zip => None,
        }
    }
}

impl TryFrom<&str> for Method {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "tar" => Ok(Method::Tar),
            "tar.gz" => Ok(Method::TarGz),
            "tar.zst" => Ok(Method::TarZstd),
            "zip" => Ok(Method::Zip),
            _ => Err(()),
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
