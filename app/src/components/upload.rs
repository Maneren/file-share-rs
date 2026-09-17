#[cfg(feature = "ssr")]
use std::sync::Arc;
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
};

use leptos::{
    ev::SubmitEvent,
    html::{Form, Input},
    logging,
    prelude::*,
    task::spawn_local,
};
use server_fn::codec::{MultipartData, MultipartFormData, StreamingText, TextStream};
use web_time::Instant;

mod form;
#[cfg(feature = "ssr")]
pub mod progress;
pub mod progress_bar;
pub mod use_upload_progress;

use form::UploadForm;
use progress_bar::{Progress, ProgressBar};
use use_upload_progress::update_progress;

#[server(input = MultipartFormData)]
pub async fn upload_file(data: MultipartData) -> Result<(), ServerFnError> {
    use server_fn::ServerFnError::ServerError;
    use tokio::{fs::OpenOptions, io::AsyncWriteExt};

    use crate::{
        AppConfig,
        utils::{is_safe_file_name, is_safe_relative_path},
    };

    fn read_upload_error(e: impl std::fmt::Display) -> ServerFnError {
        logging::error!("Failed to read upload: {e}");
        server_fn::ServerFnError::ServerError("Failed to read upload".into())
    }

    fn store_upload_error(name: &str, e: impl std::fmt::Display) -> ServerFnError {
        logging::error!("[{name}]\tfailed to store upload: {e}");
        server_fn::ServerFnError::ServerError("Failed to store upload".into())
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
        return Err(ServerError("Uploads are disabled".into()));
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

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .await
            .map_err(|e| store_upload_error(&name, e))?;

        logging::log!("[{name}]\topen");

        while let Some(chunk) = field.chunk().await.map_err(read_upload_error)? {
            let len = chunk.len();

            progress::add_chunk(&id, len).await;
            file.write_all(&chunk)
                .await
                .map_err(|e| store_upload_error(&name, e))?;
        }

        logging::log!("[{name}]\tfinished");
    }

    logging::log!("[{id}]\tfinished");
    progress::finish(&id).await;

    Ok(())
}

#[allow(clippy::unused_async)]
#[server(output = StreamingText)]
pub async fn file_progress(id: String) -> Result<TextStream, ServerFnError> {
    Ok(TextStream::new(progress::progress_stream(id.clone()).await))
}

#[island]
pub fn FileUpload(path: PathBuf) -> impl IntoView {
    let current_upload = RwSignal::new(None::<(String, Progress)>);

    let file_ref: NodeRef<Input> = NodeRef::new();
    let form_ref: NodeRef<Form> = NodeRef::new();

    let on_submit = move |ev: SubmitEvent| {
        ev.prevent_default();

        let form = form_ref.get().unwrap();
        let form_data = web_sys::FormData::new_with_form(&form).unwrap();

        let file_list = file_ref.get().unwrap().files().unwrap();

        let files = (0..file_list.length())
            .map(|i| file_list.get(i).unwrap())
            .collect::<Vec<_>>();

        if files.is_empty() {
            logging::warn!("No files selected. Aborting.");
            return;
        }

        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        let total = files.iter().map(|f| f.size() as u64).sum::<u64>();

        let id = {
            let mut hasher = DefaultHasher::default();

            for file in &files {
                file.name().hash(&mut hasher);
            }

            hasher.finish().to_string()
        };

        if current_upload.with(Option::is_some) {
            logging::warn!("Upload already in progress. Aborting.");
            return;
        }

        _ = form_data.set_with_str("id", &id);

        let _ = current_upload.write().insert((
            id.clone(),
            Progress {
                size: total,
                start_time: Instant::now(),
                uploaded: RwSignal::default(),
            },
        ));

        spawn_local(update_progress(id.clone(), current_upload));

        spawn_local(async move {
            upload_file(form_data.into())
                .await
                .expect("couldn't upload file");

            logging::log!("[{id}]\tfinished (upload)");
        });
    };

    view! {
      <div class="flex flex-col gap-2 grow">
        <UploadForm path=path file_ref=file_ref form_ref=form_ref on_submit=on_submit />

        {move || {
          current_upload
            .read()
            .as_ref()
            .map(|(_, progress)| {
              view! {
                <ProgressBar
                  size=progress.size
                  start_time=progress.start_time
                  uploaded=progress.uploaded.read_only()
                />
              }
            })
        }}
      </div>
    }
}
