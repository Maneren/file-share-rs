use leptos::prelude::*;
use server_fn::codec::{MultipartData, MultipartFormData};

#[server(input = MultipartFormData)]
pub async fn upload_file(data: MultipartData) -> Result<(), ServerFnError> {
    use std::{path::PathBuf, sync::Arc};

    use leptos::logging;
    use server_fn::ServerFnError::ServerError;
    use tokio::{
        fs::OpenOptions,
        io::{AsyncWriteExt, BufWriter},
    };

    use super::upload_progress::{add_chunk, finish};
    use crate::{
        AppConfig,
        config::{UPLOAD_DISABLED_MESSAGE, UPLOAD_READ_ERROR_MESSAGE, UPLOAD_STORE_ERROR_MESSAGE},
        fs_guard::{is_safe_file_name, is_safe_relative_path, remove_partial_upload},
    };

    fn read_upload_error(e: impl std::fmt::Display) -> ServerFnError {
        logging::error!("Failed to read upload: {e}");
        ServerError(UPLOAD_READ_ERROR_MESSAGE.into())
    }

    fn store_upload_error(name: &str, e: impl std::fmt::Display) -> ServerFnError {
        logging::error!("[{name}]\tfailed to store upload: {e}");
        ServerError(UPLOAD_STORE_ERROR_MESSAGE.into())
    }

    async fn collect_field_with_name(
        data: &mut multer::Multipart<'static>,
        name: &str,
    ) -> Result<String, ServerFnError> {
        let Some(mut field) = data.next_field().await.map_err(read_upload_error)? else {
            logging::error!("no field");
            return Err(ServerError("No field.".into()));
        };

        if field.name().is_none_or(|n| n != name) {
            return Err(ServerError(format!("Missing field '{name}'.")));
        }

        let mut buffer = String::new();
        while let Some(chunk) = field.chunk().await.map_err(read_upload_error)? {
            buffer.push_str(&String::from_utf8_lossy(&chunk));
        }

        Ok(buffer)
    }

    let app_config = expect_context::<Arc<AppConfig>>();

    if !app_config.allow_upload {
        return Err(ServerError(UPLOAD_DISABLED_MESSAGE.into()));
    }

    let Some(mut data) = data.into_inner() else {
        unreachable!("should always return Some on the server side");
    };

    let base_req_path = {
        let req_path = collect_field_with_name(&mut data, "path").await?;
        let trimmed = req_path.trim();
        let trimmed_path = PathBuf::from(trimmed);
        if !is_safe_relative_path(&trimmed_path) {
            return Err(ServerError(format!("Invalid path: {trimmed}")));
        }
        app_config.target_dir.join(trimmed)
    };

    let id = collect_field_with_name(&mut data, "id").await?;

    logging::log!("[{id}]\tbase path: {base_req_path:?}");

    while let Some(mut field) = data.next_field().await.map_err(read_upload_error)? {
        let Some(name) = field.file_name().map(str::to_owned) else {
            logging::error!("no file name");
            return Err(ServerError("Missing file name in multipart".into()));
        };

        if !is_safe_file_name(&name) {
            return Err(ServerError(format!("Invalid file name: {name}")));
        }

        let path = base_req_path.join(&name);
        logging::log!("[{name}]\tpath: {path:?}");

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .await
            .map_err(|e| store_upload_error(&name, e))?;

        logging::log!("[{name}]\topen");

        // Multipart chunks are only a few KiB; buffer to avoid a syscall
        // per chunk.
        let mut file = BufWriter::with_capacity(1024 * 1024, file);

        while let Some(chunk) = field.chunk().await.map_err(read_upload_error)? {
            let len = chunk.len();

            if let Err(e) = file.write_all(&chunk).await {
                let err = store_upload_error(&name, e);
                drop(file);
                remove_partial_upload(&path).await;
                return Err(err);
            }

            add_chunk(&id, len).await;
        }
        if let Err(e) = file.flush().await {
            let err = store_upload_error(&name, e);
            drop(file);
            remove_partial_upload(&path).await;
            return Err(err);
        }

        logging::log!("[{name}]\tfinished");
    }

    logging::log!("[{id}]\tfinished");
    finish(&id).await;

    Ok(())
}
