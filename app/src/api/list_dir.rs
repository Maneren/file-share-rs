use leptos::prelude::*;

use super::models::{ListQuery, ListingPage};

#[server(name = ListDir, prefix = "/api", endpoint = "list_dir")]
pub async fn list_dir(query: ListQuery) -> Result<ListingPage, ServerFnError> {
    use std::{path::PathBuf, sync::Arc};

    use crate::{
        api::models::ServerEntry,
        config::{AppConfig, PATH_NOT_FOUND_MESSAGE},
        fs_guard::resolve_contained_path,
    };

    fn read_dir_error(path: &PathBuf, e: impl std::fmt::Display) -> ServerFnError {
        leptos::logging::warn!("Failed to read directory {path:?}: {e}");
        ServerFnError::ServerError("Failed to read directory".into())
    }

    let base_path = expect_context::<Arc<AppConfig>>().target_dir.clone();

    let Some(path) = resolve_contained_path(&base_path, &query.path).await else {
        leptos::logging::warn!(
            "Attempt to access invalid or missing path: {:?}",
            query.path
        );
        return Err(ServerFnError::ServerError(PATH_NOT_FOUND_MESSAGE.into()));
    };

    let mut entries = Vec::new();

    let mut directory = tokio::fs::read_dir(&path)
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
            leptos::logging::warn!("Skipping {path:?}/{name}: cannot read metadata");
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            leptos::logging::warn!("Skipping {path:?}/{name}: cannot read modification time");
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

    let page = super::listing::filter_sort_page(entries, &query);
    Ok(page)
}
