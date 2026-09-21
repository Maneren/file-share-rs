use std::path::PathBuf;

use leptos::prelude::*;

#[server(name = NewFolder, prefix = "/api", endpoint = "new_folder")]
pub async fn new_folder(name: String, path: PathBuf) -> Result<(), ServerFnError> {
    use std::sync::Arc;

    use crate::{
        config::{AppConfig, UPLOAD_DISABLED_MESSAGE},
        fs_guard::{is_safe_file_name, is_safe_relative_path, resolve_contained_path},
    };

    fn create_dir_error(path: &PathBuf, name: &str, e: impl std::fmt::Display) -> ServerFnError {
        leptos::logging::warn!("Failed to create folder {path:?}/{name}: {e}");
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

    tokio::fs::create_dir(parent.join(&name))
        .await
        .map_err(|e| create_dir_error(&path, &name, e))?;

    Ok(())
}
