use std::path::PathBuf;
#[cfg(feature = "ssr")]
use std::{cmp::Ordering, sync::Arc};

cfg_if! { if #[cfg(feature = "ssr")] {
    use leptos::logging::warn;
    use tokio::fs;

    use crate::{
        config::{AppConfig, PATH_NOT_FOUND_MESSAGE, UPLOAD_DISABLED_MESSAGE},
        utils::resolve_contained_path,
    };
    use nucleo::{Config as NucleoConfig, Matcher, Utf32Str};
}}

use cfg_if::cfg_if;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::utils::SystemTime;

pub type Entries = Vec<ServerEntry>;

/// Parameters for a directory listing: which slice of the sorted entries
/// to return. Sorting stays server-side so pages are stable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListQuery {
    pub path: PathBuf,
    pub sort_column: SortColumn,
    pub sort_dir: SortDir,
    /// Fuzzy name filter (nucleo); empty disables it.
    pub search: String,
    /// Initial-letter filter; `None` disables it.
    pub initial: Option<char>,
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
    fn name(&self) -> &str {
        match self {
            Self::Folder { name, .. } | Self::File { name, .. } => name,
        }
    }

    fn is_folder(&self) -> bool {
        matches!(self, Self::Folder { .. })
    }

    /// File size, or 0 for folders (only compared within the folder group).
    fn file_size(&self) -> u64 {
        match self {
            Self::File { size, .. } => *size,
            Self::Folder { .. } => 0,
        }
    }

    fn modified(&self) -> SystemTime {
        match self {
            Self::Folder { last_modified, .. } | Self::File { last_modified, .. } => *last_modified,
        }
    }
}

/// Folders sort before files; the caller reverses the whole ordering for
/// descending sorts, which puts them last there.
#[cfg(feature = "ssr")]
fn folders_first(a: &ServerEntry, b: &ServerEntry) -> Ordering {
    a.is_folder().cmp(&b.is_folder()).reverse()
}

#[server(name = ListDir, prefix = "/api", endpoint = "list_dir")]
pub async fn list_dir(query: ListQuery) -> Result<ListingPage, ServerFnError> {
    fn read_dir_error(path: &PathBuf, e: impl std::fmt::Display) -> ServerFnError {
        warn!("Failed to read directory {path:?}: {e}");
        ServerFnError::ServerError("Failed to read directory".into())
    }

    let ListQuery {
        path,
        sort_column,
        sort_dir,
        search,
        initial,
        limit,
        offset,
    } = query;
    let base_path = expect_context::<Arc<AppConfig>>().target_dir.clone();

    let Some(path) = resolve_contained_path(&base_path, &path).await else {
        warn!("Attempt to access invalid or missing path: {path:?}");
        return Err(ServerFnError::ServerError(PATH_NOT_FOUND_MESSAGE.into()));
    };

    let mut entries = Vec::new();

    let mut directory = fs::read_dir(&path)
        .await
        .map_err(|e| read_dir_error(&path, e))?;

    while let Some(entry) = directory
        .next_entry()
        .await
        .map_err(|e| read_dir_error(&path, e))?
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        // One unreadable entry must not fail the whole listing, and its
        // OS error stays server-side.
        let Ok(metadata) = entry.metadata().await else {
            warn!("Skipping {path:?}/{name}: cannot read metadata");
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            warn!("Skipping {path:?}/{name}: cannot read modification time");
            continue;
        };
        let last_modified = modified.into();

        if metadata.is_dir() {
            entries.push(ServerEntry::Folder {
                name,
                last_modified,
            });
        } else if metadata.is_file() {
            entries.push(ServerEntry::File {
                name,
                size: metadata.len(),
                last_modified,
            });
        }
    }

    let (entries, total) = filter_sort_page(
        entries,
        sort_column,
        sort_dir,
        &search,
        initial,
        limit,
        offset,
    );
    Ok(ListingPage { entries, total })
}

/// Filter, sort and paginate collected directory entries.
///
/// - `initial` keeps names starting with that letter (case-insensitive).
/// - A non-empty `search` keeps nucleo fuzzy matches, ranked by score.
/// - Otherwise entries sort by `sort_column`: folders first for name and size
///   (last when descending), purely by time for modified.
/// - Pagination applies last; `total` counts everything before it.
#[cfg(feature = "ssr")]
fn filter_sort_page(
    entries: Entries,
    sort_column: SortColumn,
    sort_dir: SortDir,
    search: &str,
    initial: Option<char>,
    limit: usize,
    offset: usize,
) -> (Entries, usize) {
    let mut entries = entries;
    if let Some(initial) = initial {
        let needle = initial.to_lowercase().collect::<String>();
        entries.retain(|entry| entry.name().to_lowercase().starts_with(&needle));
    }

    // Lowercase names are cached once so sorting never allocates per
    // comparison.
    struct SortRow {
        lower_name: String,
        entry: ServerEntry,
        /// Fuzzy-match score; `None` when no search is active.
        score: Option<u16>,
    }

    let searching = !search.trim().is_empty();
    let mut matcher = Matcher::new(NucleoConfig::DEFAULT);
    let search_lower = search.to_lowercase();
    let mut needle_buf = Vec::new();
    let needle = Utf32Str::new(&search_lower, &mut needle_buf);
    let mut haystack_buf = Vec::new();

    let mut rows = Vec::with_capacity(entries.len());
    for entry in entries {
        let lower_name = entry.name().to_lowercase();
        let score = searching
            .then(|| {
                haystack_buf.clear();
                matcher.fuzzy_match(Utf32Str::new(&lower_name, &mut haystack_buf), needle)
            })
            .flatten();
        // Without a search everything matches; with one, only matches do.
        if !searching || score.is_some() {
            rows.push(SortRow {
                lower_name,
                entry,
                score,
            });
        }
    }

    rows.sort_by(|a, b| {
        // Search results rank by relevance; the column sort does not apply.
        if searching {
            return b
                .score
                .cmp(&a.score)
                .then_with(|| a.lower_name.cmp(&b.lower_name))
                .then_with(|| a.entry.name().cmp(b.entry.name()));
        }
        let asc = match sort_column {
            SortColumn::Name => folders_first(&a.entry, &b.entry)
                .then_with(|| a.lower_name.cmp(&b.lower_name))
                .then_with(|| a.entry.name().cmp(b.entry.name())),
            SortColumn::Size => folders_first(&a.entry, &b.entry)
                .then_with(|| a.entry.file_size().cmp(&b.entry.file_size()))
                .then_with(|| a.lower_name.cmp(&b.lower_name))
                .then_with(|| a.entry.name().cmp(b.entry.name())),
            // Time mixes files and folders; the folder tiebreak below is
            // only a final resort for identical names and mtimes.
            SortColumn::Modified => a
                .entry
                .modified()
                .cmp(&b.entry.modified())
                .then_with(|| a.lower_name.cmp(&b.lower_name))
                .then_with(|| a.entry.name().cmp(b.entry.name()))
                .then_with(|| folders_first(&a.entry, &b.entry)),
        };
        if sort_dir == SortDir::Desc {
            asc.reverse()
        } else {
            asc
        }
    });

    let total = rows.len();
    let entries = rows
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|row| row.entry)
        .collect();
    (entries, total)
}

#[server(name = NewFolder, prefix = "/api", endpoint = "new_folder")]
pub async fn new_folder(name: String, path: PathBuf) -> Result<(), ServerFnError> {
    use crate::utils::{is_safe_file_name, is_safe_relative_path};

    fn create_dir_error(path: &PathBuf, name: &str, e: impl std::fmt::Display) -> ServerFnError {
        warn!("Failed to create folder {path:?}/{name}: {e}");
        ServerFnError::ServerError("Failed to create folder".into())
    }

    let app_config = expect_context::<Arc<AppConfig>>();

    if !app_config.allow_upload {
        return Err(ServerFnError::ServerError(UPLOAD_DISABLED_MESSAGE.into()));
    }

    if !is_safe_relative_path(&path) || !is_safe_file_name(&name) {
        return Err(ServerFnError::ServerError("Invalid path or name".into()));
    }

    // Resolve the parent through the real filesystem so a symlinked
    // directory cannot redirect the new folder outside the share.
    // `name` is a single normal component, so joining it cannot escape.
    let Some(parent) = resolve_contained_path(&app_config.target_dir, &path).await else {
        return Err(ServerFnError::ServerError("Invalid path".into()));
    };

    fs::create_dir(parent.join(&name))
        .await
        .map_err(|e| create_dir_error(&path, &name, e))?;

    Ok(())
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;
    use crate::utils::SystemTime;

    fn file(name: &str, size: u64, modified: i64) -> ServerEntry {
        ServerEntry::File {
            name: name.into(),
            size,
            last_modified: SystemTime(modified, 0),
        }
    }

    fn folder(name: &str, modified: i64) -> ServerEntry {
        ServerEntry::Folder {
            name: name.into(),
            last_modified: SystemTime(modified, 0),
        }
    }

    fn names(entries: &Entries) -> Vec<&str> {
        entries.iter().map(ServerEntry::name).collect()
    }

    fn fixture() -> Entries {
        vec![
            file("zebra.bin", 50_000, 100),
            folder("bdir", 50),
            file("apple.txt", 6, 10),
            folder("adir", 200),
            file("Cherry.md", 2, 300),
        ]
    }

    #[test]
    fn name_sort_folders_first() {
        let (entries, total) =
            filter_sort_page(fixture(), SortColumn::Name, SortDir::Asc, "", None, 100, 0);
        assert_eq!(total, 5);
        assert_eq!(
            names(&entries),
            ["adir", "bdir", "apple.txt", "Cherry.md", "zebra.bin"]
        );
    }

    #[test]
    fn name_sort_desc_folders_last() {
        let (entries, _) =
            filter_sort_page(fixture(), SortColumn::Name, SortDir::Desc, "", None, 100, 0);
        assert_eq!(
            names(&entries),
            ["zebra.bin", "Cherry.md", "apple.txt", "bdir", "adir"]
        );
    }

    #[test]
    fn size_sort_folders_first() {
        let (entries, _) =
            filter_sort_page(fixture(), SortColumn::Size, SortDir::Asc, "", None, 100, 0);
        assert_eq!(
            names(&entries),
            ["adir", "bdir", "Cherry.md", "apple.txt", "zebra.bin"]
        );
    }

    #[test]
    fn time_sort_mixes_folders_and_files() {
        let (entries, _) = filter_sort_page(
            fixture(),
            SortColumn::Modified,
            SortDir::Asc,
            "",
            None,
            100,
            0,
        );
        assert_eq!(
            names(&entries),
            ["apple.txt", "bdir", "zebra.bin", "adir", "Cherry.md"]
        );
    }

    #[test]
    fn initial_filter_is_case_insensitive() {
        let (entries, total) = filter_sort_page(
            fixture(),
            SortColumn::Name,
            SortDir::Asc,
            "",
            Some('a'),
            100,
            0,
        );
        assert_eq!(total, 2);
        assert_eq!(names(&entries), ["adir", "apple.txt"]);

        let (entries, total) = filter_sort_page(
            fixture(),
            SortColumn::Name,
            SortDir::Asc,
            "",
            Some('C'),
            100,
            0,
        );
        assert_eq!(total, 1);
        assert_eq!(names(&entries), ["Cherry.md"]);
    }

    #[test]
    fn fuzzy_search_finds_and_ranks() {
        let (entries, total) = filter_sort_page(
            fixture(),
            SortColumn::Name,
            SortDir::Asc,
            "cher",
            None,
            100,
            0,
        );
        assert_eq!(total, 1);
        assert_eq!(names(&entries), ["Cherry.md"]);

        let (entries, total) = filter_sort_page(
            fixture(),
            SortColumn::Name,
            SortDir::Asc,
            "ir",
            None,
            100,
            0,
        );
        assert_eq!(total, 2);
        assert_eq!(names(&entries), ["adir", "bdir"]);
    }

    #[test]
    fn pagination_slices_after_sort() {
        let (entries, total) =
            filter_sort_page(fixture(), SortColumn::Name, SortDir::Asc, "", None, 2, 2);
        assert_eq!(total, 5);
        assert_eq!(names(&entries), ["apple.txt", "Cherry.md"]);
    }
}
