use std::path::PathBuf;

use leptos::prelude::*;

use super::models::{ListQuery, ListingPage};

#[server(name = List, prefix = "/api", endpoint = "list_dir")]
pub async fn list(query: ListQuery) -> Result<ListingPage, ServerFnError> {
    use std::{path::PathBuf, sync::Arc};

    use crate::{
        api::models::ServerEntry,
        config::{Config, PATH_NOT_FOUND_MESSAGE},
        fs_guard::resolve_contained_path,
    };

    fn read_dir_error(path: &PathBuf, e: impl std::fmt::Display) -> ServerFnError {
        leptos::logging::warn!("Failed to read directory {path:?}: {e}");
        ServerFnError::ServerError("Failed to read directory".into())
    }

    let base_path = expect_context::<Arc<Config>>().target_dir.clone();

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

#[server(name = NewFolder, prefix = "/api", endpoint = "new_folder")]
pub async fn create(name: String, path: PathBuf) -> Result<(), ServerFnError> {
    use std::sync::Arc;

    use crate::{
        config::{Config, UPLOAD_DISABLED_MESSAGE},
        fs_guard::{is_safe_file_name, is_safe_relative_path, resolve_contained_path},
    };

    fn create_dir_error(path: &PathBuf, name: &str, e: impl std::fmt::Display) -> ServerFnError {
        leptos::logging::warn!("Failed to create folder {path:?}/{name}: {e}");
        ServerFnError::ServerError("Failed to create folder".into())
    }

    let app_config = expect_context::<Arc<Config>>();

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

    tokio::fs::create_dir(parent.join(&name))
        .await
        .map_err(|e| create_dir_error(&path, &name, e))?;

    Ok(())
}
