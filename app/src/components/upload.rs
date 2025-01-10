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

use crate::utils::format_bytes;

#[cfg(feature = "ssr")]
mod progress;

#[server(input = MultipartFormData)]
pub async fn upload_file(data: MultipartData) -> Result<(), ServerFnError> {
    use server_fn::ServerFnError::ServerError;
    use tokio::{fs::OpenOptions, io::AsyncWriteExt};

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

    let Some(mut data) = data.into_inner() else {
        unreachable!("should always return Some on the server side");
    };

    let base_req_path = {
        let base_path = expect_context::<PathBuf>().clone();
        let req_path = collect_field_with_name(&mut data, "path").await?;
        base_path.join(req_path.trim())
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

#[derive(Debug, Clone, Copy)]
struct Progress {
    pub size: u64,
    pub start_time: Instant,
    pub uploaded: RwSignal<u64>,
}

async fn update_progress(id: String, upload: RwSignal<Option<(String, Progress)>>) {
    use futures::StreamExt;

    let mut progress = file_progress(id.clone())
        .await
        .expect("couldn't initialize stream")
        .into_inner();

    while let Some(Ok(chunk)) = progress.next().await {
        let messages = chunk
            .split('\n')
            .filter_map(|line| line.split_once('\0'))
            .filter_map(|(id, size)| size.parse::<u64>().ok().map(|size| (id, size)));

        upload.with_untracked(|upload| {
            let Some((stored_id, Progress { uploaded, .. })) = upload.as_ref() else {
                return;
            };

            let uploaded = *uploaded;

            for (id, size) in messages {
                if id != stored_id {
                    logging::warn!("Got progress for unknown id '{id}'");
                    continue;
                }

                *uploaded.write() = size;
            }
        });
    }

    logging::log!("[{id}]\tfinished (stream)");

    upload.write().take();
}

#[component]
pub fn FileUpload(path: Signal<PathBuf>, #[prop(into)] on_upload: Callback<()>) -> impl IntoView {
    let current_upload = RwSignal::new(None::<(String, Progress)>);

    let file_ref: NodeRef<Input> = NodeRef::new();
    let form_ref: NodeRef<Form> = NodeRef::new();

    // TODO: convert progress to local resource

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

        if current_upload.read().is_some() {
            logging::warn!("Upload already in progress. Aborting.");
            return;
        }

        _ = form_data.set_with_str("id", &id);

        let _ = current_upload.write().insert((
            id.clone(),
            Progress {
                size: total,
                start_time: Instant::now(),
                uploaded: RwSignal::new(0),
            },
        ));

        spawn_local(update_progress(id.clone(), current_upload));

        spawn_local(async move {
            upload_file(form_data.into())
                .await
                .expect("couldn't upload file");

            logging::log!("[{id}]\tfinished (upload)");

            on_upload.run(());
        });
    };

    let ProgressBar = |Progress {
                           size,
                           start_time,
                           uploaded,
                       }| {
        let percent = move || *uploaded.read() * 100 / size;
        let speed = move || {
            format_bytes(((uploaded() * 1000) as f64 / start_time.elapsed().as_millis_f64()) as u64)
        };
        view! {
          <div class="m-2 flex flex-row items-baseline justify-between gap-5 w-full">
            <span>Uploading {move || format!("{: >3}", percent())}%</span>
            <div class="bg-neutral rounded-full flex-grow h-3">
              <div
                class="bg-info h-full transition-all ease-linear duration-50 rounded-full"
                style:width=move || format!("{: >3}%", percent())
              />
            </div>
            <span class="w-28 text-right">{speed}/s</span>
          </div>
        }
    };

    view! {
      <div class="flex flex-grow flex-col gap-2">
        <form
          class="flex flex-wrap grow-[2] gap-2"
          method="POST"
          enctype="multipart/form-data"
          node_ref=form_ref
          on:submit=on_submit
        >
          <input
            type="hidden"
            name="path"
            value=move || path.with(|path| path.to_string_lossy().into_owned())
          />
          // placeholder that is filled on submission
          <input type="hidden" name="id" value="" />
          <input
            type="file"
            name="uploads"
            class="file-input file-input-bordered grow-[3]"
            multiple
            node_ref=file_ref
          />
          // ref_=file_ref
          <button type="submit" class="btn btn-primary grow-[1]">
            Upload
          </button>
        </form>

        {move || { current_upload.read().as_ref().map(|(_, progress)| ProgressBar(*progress)) }}
      </div>
    }
}
