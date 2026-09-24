//! Archive methods shared between the server and the web UI.
//!
//! Kept dependency-free on purpose so it compiles for both the SSR server
//! target and the WASM frontend target. The (tokio-based) code that actually
//! writes archives lives in the server crate.

use std::{error::Error, fmt, str::FromStr};

use serde::{Deserialize, Deserializer};

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
    pub const ALL: [Self; 4] = [Self::Tar, Self::TarGz, Self::TarZstd, Self::Zip];

    /// URL query value and file extension, e.g. `tar.zst`.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Tar => "tar",
            Self::TarGz => "tar.gz",
            Self::TarZstd => "tar.zst",
            Self::Zip => "zip",
        }
    }

    #[must_use]
    pub const fn mimetype(&self) -> &'static str {
        match self {
            Self::Tar => "application/x-tar",
            Self::TarGz => "application/gzip",
            Self::TarZstd => "application/zstd",
            Self::Zip => "application/zip",
        }
    }

    /// Wire compression the client must undo with `DecompressionStream`, or
    /// `None` when the format cannot be stream-extracted in the browser (zip
    /// keeps its central directory at the end of the file).
    #[must_use]
    pub const fn wire_compression(&self) -> Option<&'static str> {
        match self {
            Self::Tar => Some("none"),
            Self::TarGz => Some("gzip"),
            Self::TarZstd => Some("zstd"),
            Self::Zip => None,
        }
    }

    #[must_use]
    pub const fn is_streamable(&self) -> bool {
        self.wire_compression().is_some()
    }

    /// Flags for extracting this archive with CLI `tar` over a curl pipe,
    /// e.g. `curl … | tar --zstd -x -C .`. `None` for non-tar formats (zip),
    /// which cannot be extracted from a pipe — download the file instead.
    #[must_use]
    pub const fn tar_extract_flags(&self) -> Option<&'static str> {
        match self {
            Self::Tar => Some(""),
            Self::TarGz => Some("-z"),
            Self::TarZstd => Some("--zstd"),
            Self::Zip => None,
        }
    }
}

/// Error returned when parsing an archive [`Method`] from a query string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseMethodError(String);

impl fmt::Display for ParseMethodError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid archive method '{}', expected one of: tar, tar.gz, tar.zst, zip",
            self.0
        )
    }
}

impl Error for ParseMethodError {}

impl FromStr for Method {
    type Err = ParseMethodError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "tar" => Ok(Self::Tar),
            "tar.gz" => Ok(Self::TarGz),
            "tar.zst" => Ok(Self::TarZstd),
            "zip" => Ok(Self::Zip),
            _ => Err(ParseMethodError(value.to_owned())),
        }
    }
}

impl TryFrom<&str> for Method {
    type Error = ParseMethodError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl<'de> Deserialize<'de> for Method {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
