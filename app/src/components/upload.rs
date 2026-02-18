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

    use crate::AppConfig;

    async fn collect_field_with_name(
        data: &mut multer::Multipart<'static>,
        name: &str,
    ) -> Result<String, ServerFnError> {
        let Ok(Some(mut field)) = data.next_field().await else {
            logging::error!("no field");
            return Err(ServerError("No field.".into()));
        };

        if field.name().is_none_or(|n| n != name) {
            return Err(ServerError(format!("Missing field '{name}'.")));
        }

        let mut buffer = String::new();
        while let Ok(Some(chunk)) = field.chunk().await {
            buffer.push_str(&String::from_utf8_lossy(&chunk));
        }

        Ok(buffer)
    }

    let app_config = expect_context::<AppConfig>();

    if !app_config.allow_upload {
        return Err(ServerError("Uploads are disabled".into()));
    }

    let Some(mut data) = data.into_inner() else {
        unreachable!("should always return Some on the server side");
    };

    let base_req_path = {
        let req_path = collect_field_with_name(&mut data, "path").await?;
        app_config.target_dir.join(req_path.trim())
    };

    let id = collect_field_with_name(&mut data, "id").await?;

    logging::log!("[{id}]\tbase path: {base_req_path:?}");

    while let Ok(Some(mut field)) = data.next_field().await {
        let Some(name) = field.file_name().map(ToOwned::to_owned) else {
            logging::error!("no file name");
            return Err(ServerError("Missing file name in multipart".into()));
        };

        let path = base_req_path.join(&name);
        logging::log!("[{name}]\tpath: {path:?}");

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .await?;

        logging::log!("[{name}]\topen");

        while let Ok(Some(chunk)) = field.chunk().await {
            let len = chunk.len();

            progress::add_chunk(&id, len).await;
            file.write_all(&chunk).await?;
        }

        logging::log!("[{name}]\tfinished");
    }

    logging::log!("[{id}]\tfinished");
    progress::finish(&id);

    Ok(())
}

#[allow(clippy::unused_async)]
#[server(output = StreamingText)]
pub async fn file_progress(id: String) -> Result<TextStream, ServerFnError> {
    Ok(TextStream::new(progress::progress_stream(id.clone())))
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

        if current_upload.with(|upload| upload.is_some()) {
            logging::warn!("Upload already in progress. Aborting.");
            return;
        }

        _ = form_data.set_with_str("id", &id);

        let _ = current_upload.write().insert((
            id.clone(),
            Progress {
                size: total,
                start_time: Instant::now(),
                uploaded: Default::default(),
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
