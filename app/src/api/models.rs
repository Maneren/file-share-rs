use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::time::SystemTime;

pub type Entries = Vec<ServerEntry>;

/// Parameters for a directory listing: which slice of the sorted entries
/// to return. Sorting stays server-side so pages are stable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ListQuery {
    pub path: PathBuf,
    pub sort_column: SortColumn,
    pub sort_dir: SortDir,
    /// Fuzzy name filter (nucleo); empty disables it.
    pub search: String,
    /// Initial-letter filter; `None` disables it.
    pub initial: Option<char>,
    /// Show dotfiles. Hidden files are skipped (and counted) otherwise.
    pub show_hidden: bool,
    pub limit: usize,
    pub offset: usize,
}

/// Column to sort a listing by.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SortColumn {
    #[default]
    Name,
    Size,
    Modified,
}

/// Sort direction. Also flips folder grouping: folders come first
/// ascending, last descending.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SortDir {
    #[default]
    Asc,
    Desc,
}

/// One page of a directory listing plus the total entry count, so the UI
/// can render page controls without a separate count round-trip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingPage {
    pub entries: Entries,
    pub total: usize,
    /// Dotfiles skipped by the hidden filter (0 when shown).
    pub hidden_count: usize,
    /// Uppercase initials present in the folder (after the hidden filter,
    /// before search/initial filters), for the letter buttons.
    pub initials: Vec<char>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Ord, Eq)]
pub enum ServerEntry {
    Folder {
        name: String,
        last_modified: SystemTime,
    },
    File {
        name: String,
        size: u64,
        last_modified: SystemTime,
    },
}

#[cfg(feature = "ssr")]
impl ServerEntry {
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Folder { name, .. } | Self::File { name, .. } => name,
        }
    }

    pub(crate) fn is_folder(&self) -> bool {
        matches!(self, Self::Folder { .. })
    }

    /// File size, or 0 for folders (only compared within the folder group).
    pub(crate) fn file_size(&self) -> u64 {
        match self {
            Self::File { size, .. } => *size,
            Self::Folder { .. } => 0,
        }
    }

    pub(crate) fn modified(&self) -> SystemTime {
        match self {
            Self::Folder { last_modified, .. } | Self::File { last_modified, .. } => *last_modified,
        }
    }
}
