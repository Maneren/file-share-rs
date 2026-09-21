#![cfg(feature = "ssr")]

//! Pure filter/sort/paginate logic for directory listings.
//!
//! No I/O here — takes already-collected [`Entries`] plus a [`ListQuery`]
//! and returns a [`ListingPage`]. Fully unit-tested below.

use std::cmp::Ordering;

use nucleo::{Config as NucleoConfig, Matcher, Utf32Str};

use super::models::{Entries, ListQuery, ListingPage, ServerEntry, SortColumn, SortDir};

/// Folders sort before files; the caller reverses the whole ordering for
/// descending sorts, which puts them last there.
fn folders_first(a: &ServerEntry, b: &ServerEntry) -> Ordering {
    a.is_folder().cmp(&b.is_folder()).reverse()
}

/// Filter, sort and paginate collected directory entries.
///
/// - Dotfiles are skipped (and counted) unless `show_hidden`.
/// - `initial` keeps names starting with that letter (case-insensitive).
/// - A non-empty `search` keeps nucleo fuzzy matches, ranked by score.
/// - Otherwise entries sort by `sort_column`: folders first for name and size
///   (last when descending), purely by time for modified.
/// - Pagination applies last; `total` counts everything before it.
pub(crate) fn filter_sort_page(entries: Entries, query: &ListQuery) -> ListingPage {
    // Lowercase names are cached once so sorting never allocates per
    // comparison.
    struct SortRow {
        lower_name: String,
        entry: ServerEntry,
        /// Fuzzy-match score; `None` when no search is active.
        score: Option<u16>,
    }

    let initial_needle = query.initial.map(|c| c.to_lowercase().collect::<String>());
    let searching = !query.search.trim().is_empty();
    let mut matcher = Matcher::new(NucleoConfig::DEFAULT);
    let search_lower = query.search.to_lowercase();
    let mut needle_buf = Vec::new();
    let needle = Utf32Str::new(&search_lower, &mut needle_buf);
    let mut haystack_buf = Vec::new();

    let mut hidden_count = 0;
    // Initials drive the letter buttons. Collected on the hidden-filtered
    // set so navigation between letters never traps the user.
    let mut initials = Vec::new();
    // Single pass: each name is lowercased exactly once and shared by the
    // initial filter, the initials collection and the fuzzy matcher.
    let mut rows = Vec::with_capacity(entries.len());
    for entry in entries {
        if !query.show_hidden && entry.name().starts_with('.') {
            hidden_count += 1;
            continue;
        }
        let lower_name = entry.name().to_lowercase();
        if let Some(c) = lower_name.chars().next().map(|c| c.to_ascii_uppercase())
            && c.is_ascii_alphabetic()
        {
            initials.push(c);
        }
        if initial_needle
            .as_ref()
            .is_some_and(|prefix| !lower_name.starts_with(prefix))
        {
            continue;
        }
        let score = searching
            .then(|| {
                haystack_buf.clear();
                matcher.fuzzy_match(Utf32Str::new(&lower_name, &mut haystack_buf), needle)
            })
            .flatten();
        if !searching || score.is_some() {
            rows.push(SortRow {
                lower_name,
                entry,
                score,
            });
        }
    }
    initials.sort_unstable();
    initials.dedup();

    rows.sort_by(|a, b| {
        // Search results rank by relevance; the column sort does not apply.
        if searching {
            return b
                .score
                .cmp(&a.score)
                .then_with(|| a.lower_name.cmp(&b.lower_name))
                .then_with(|| a.entry.name().cmp(b.entry.name()));
        }
        let asc = match query.sort_column {
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
        if query.sort_dir == SortDir::Desc {
            asc.reverse()
        } else {
            asc
        }
    });

    let total = rows.len();
    let entries = rows
        .into_iter()
        .skip(query.offset)
        .take(query.limit)
        .map(|row| row.entry)
        .collect();
    ListingPage {
        entries,
        total,
        hidden_count,
        initials,
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::time::SystemTime;

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
        entries
            .iter()
            .map(|e| match e {
                ServerEntry::Folder { name, .. } | ServerEntry::File { name, .. } => name.as_str(),
            })
            .collect()
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

    fn query(sort_column: SortColumn, sort_dir: SortDir) -> ListQuery {
        ListQuery {
            sort_column,
            sort_dir,
            ..Default::default()
        }
    }

    #[test]
    fn name_sort_folders_first() {
        let page = filter_sort_page(
            fixture(),
            &ListQuery {
                limit: 100,
                ..query(SortColumn::Name, SortDir::Asc)
            },
        );
        assert_eq!(page.total, 5);
        assert_eq!(
            names(&page.entries),
            ["adir", "bdir", "apple.txt", "Cherry.md", "zebra.bin"]
        );
        assert_eq!(page.initials, ['A', 'B', 'C', 'Z']);
    }

    #[test]
    fn name_sort_desc_folders_last() {
        let page = filter_sort_page(
            fixture(),
            &ListQuery {
                limit: 100,
                ..query(SortColumn::Name, SortDir::Desc)
            },
        );
        assert_eq!(
            names(&page.entries),
            ["zebra.bin", "Cherry.md", "apple.txt", "bdir", "adir"]
        );
    }

    #[test]
    fn size_sort_folders_first() {
        let page = filter_sort_page(
            fixture(),
            &ListQuery {
                limit: 100,
                ..query(SortColumn::Size, SortDir::Asc)
            },
        );
        assert_eq!(
            names(&page.entries),
            ["adir", "bdir", "Cherry.md", "apple.txt", "zebra.bin"]
        );
    }

    #[test]
    fn time_sort_mixes_folders_and_files() {
        let page = filter_sort_page(
            fixture(),
            &ListQuery {
                limit: 100,
                ..query(SortColumn::Modified, SortDir::Asc)
            },
        );
        assert_eq!(
            names(&page.entries),
            ["apple.txt", "bdir", "zebra.bin", "adir", "Cherry.md"]
        );
    }

    #[test]
    fn initial_filter_is_case_insensitive() {
        let page = filter_sort_page(
            fixture(),
            &ListQuery {
                initial: Some('a'),
                limit: 100,
                ..query(SortColumn::Name, SortDir::Asc)
            },
        );
        assert_eq!(page.total, 2);
        assert_eq!(names(&page.entries), ["adir", "apple.txt"]);

        let page = filter_sort_page(
            fixture(),
            &ListQuery {
                initial: Some('C'),
                limit: 100,
                ..query(SortColumn::Name, SortDir::Asc)
            },
        );
        assert_eq!(page.total, 1);
        assert_eq!(names(&page.entries), ["Cherry.md"]);
    }

    #[test]
    fn fuzzy_search_finds_and_ranks() {
        let page = filter_sort_page(
            fixture(),
            &ListQuery {
                search: "cher".into(),
                limit: 100,
                ..query(SortColumn::Name, SortDir::Asc)
            },
        );
        assert_eq!(page.total, 1);
        assert_eq!(names(&page.entries), ["Cherry.md"]);

        let page = filter_sort_page(
            fixture(),
            &ListQuery {
                search: "ir".into(),
                limit: 100,
                ..query(SortColumn::Name, SortDir::Asc)
            },
        );
        assert_eq!(page.total, 2);
        assert_eq!(names(&page.entries), ["adir", "bdir"]);
    }

    #[test]
    fn size_sort_desc_files_first() {
        let page = filter_sort_page(
            fixture(),
            &ListQuery {
                limit: 100,
                ..query(SortColumn::Size, SortDir::Desc)
            },
        );
        assert_eq!(
            names(&page.entries),
            ["zebra.bin", "apple.txt", "Cherry.md", "bdir", "adir"]
        );
    }

    #[test]
    fn time_sort_desc_mixes_folders_and_files() {
        let page = filter_sort_page(
            fixture(),
            &ListQuery {
                limit: 100,
                ..query(SortColumn::Modified, SortDir::Desc)
            },
        );
        assert_eq!(
            names(&page.entries),
            ["Cherry.md", "adir", "zebra.bin", "bdir", "apple.txt"]
        );
    }

    #[test]
    fn search_combines_with_initial_filter() {
        let page = filter_sort_page(
            fixture(),
            &ListQuery {
                search: "ir".into(),
                initial: Some('a'),
                limit: 100,
                ..query(SortColumn::Name, SortDir::Asc)
            },
        );
        assert_eq!(page.total, 1);
        assert_eq!(names(&page.entries), ["adir"]);
    }

    #[test]
    fn pagination_past_end_is_empty() {
        let page = filter_sort_page(
            fixture(),
            &ListQuery {
                limit: 100,
                offset: 99,
                ..query(SortColumn::Name, SortDir::Asc)
            },
        );
        assert_eq!(page.total, 5);
        assert!(page.entries.is_empty());
    }

    #[test]
    fn hidden_search_counts_skipped() {
        let entries = vec![file(".secret", 1, 1), file("visible.txt", 1, 1)];
        let page = filter_sort_page(
            entries,
            &ListQuery {
                search: "sec".into(),
                limit: 100,
                ..query(SortColumn::Name, SortDir::Asc)
            },
        );
        assert_eq!(page.total, 0);
        assert_eq!(page.hidden_count, 1);
        assert!(page.entries.is_empty());
    }

    #[test]
    fn pagination_slices_after_sort() {
        let page = filter_sort_page(
            fixture(),
            &ListQuery {
                limit: 2,
                offset: 2,
                ..query(SortColumn::Name, SortDir::Asc)
            },
        );
        assert_eq!(page.total, 5);
        assert_eq!(names(&page.entries), ["apple.txt", "Cherry.md"]);
    }

    #[test]
    fn listing_query_page_serde_roundtrip() {
        // The client fetches these over the wire; a missing/broken impl
        // would surface only at runtime in the browser.
        let query = ListQuery {
            path: PathBuf::new(),
            sort_column: SortColumn::Size,
            sort_dir: SortDir::Desc,
            search: "vid".into(),
            initial: Some('V'),
            show_hidden: true,
            limit: 100,
            offset: 200,
        };
        let back: ListQuery =
            serde_json::from_str(&serde_json::to_string(&query).unwrap()).unwrap();
        assert_eq!(back, query);

        let page = ListingPage {
            entries: fixture(),
            total: 5,
            hidden_count: 1,
            initials: vec!['A', 'Z'],
        };
        let back: ListingPage =
            serde_json::from_str(&serde_json::to_string(&page).unwrap()).unwrap();
        assert_eq!(back.total, 5);
        assert_eq!(back.hidden_count, 1);
        assert_eq!(back.initials, ['A', 'Z']);
        assert_eq!(names(&back.entries).len(), 5);
    }

    #[test]
    fn hidden_files_filtered_unless_shown() {
        let hidden = || {
            vec![
                file(".secret", 1, 1),
                file("visible.txt", 1, 1),
                folder(".hdir", 1),
            ]
        };
        let page = filter_sort_page(
            hidden(),
            &ListQuery {
                limit: 100,
                ..query(SortColumn::Name, SortDir::Asc)
            },
        );
        assert_eq!(page.total, 1);
        assert_eq!(page.hidden_count, 2);
        assert_eq!(names(&page.entries), ["visible.txt"]);
        assert_eq!(page.initials, ['V']);

        let page = filter_sort_page(
            hidden(),
            &ListQuery {
                show_hidden: true,
                limit: 100,
                ..query(SortColumn::Name, SortDir::Asc)
            },
        );
        assert_eq!(page.total, 3);
        assert_eq!(page.hidden_count, 0);
        assert_eq!(names(&page.entries), [".hdir", ".secret", "visible.txt"]);
        // Dotfiles start with `.`, which has no letter button.
        assert_eq!(page.initials, ['V']);
    }
}
