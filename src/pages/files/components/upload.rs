use std::{
  collections::HashMap,
  hash::{DefaultHasher, Hash, Hasher},
  path::PathBuf,
};

use futures::StreamExt;
use leptos::{
  ev::SubmitEvent,
  html::{Form, Input},
  *,
};
use server_fn::codec::{MultipartData, MultipartFormData, StreamingText, TextStream};

#[cfg(feature = "ssr")]
mod progress;

#[server(input = MultipartFormData)]
pub async fn upload_file(data: MultipartData) -> Result<(), ServerFnError> {
  use server_fn::ServerFnError::ServerError;
  use tokio::{fs::OpenOptions, io::AsyncWriteExt};

  use crate::config::get_target_dir;

  async fn collect_field_with_name(
    data: &mut multer::Multipart<'static>,
    name: &str,
  ) -> Result<String, ServerFnError> {
    let Ok(Some(mut field)) = data.next_field().await else {
      logging::error!("no field");
      return Err(ServerError("No field.".into()));
    };

    if !field.name().is_some_and(|n| n == name) {
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

  let base_path = {
    let req_path = collect_field_with_name(&mut data, "path").await?;
    get_target_dir().join(req_path.trim())
  };

  let id = collect_field_with_name(&mut data, "id").await?;

  logging::log!("Base path: {base_path:?}; id: {id:?}");

  while let Ok(Some(mut field)) = data.next_field().await {
    let Some(name) = field.file_name().map(ToOwned::to_owned) else {
      logging::error!("no file name");
      return Err(ServerError("Missing file name in multipart".into()));
    };

    let path = base_path.join(&name);
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
    progress::finish_file(&name).await;
  }

  Ok(())
}

#[allow(clippy::unused_async)]
#[server(output = StreamingText)]
pub async fn file_progress(id: String) -> Result<TextStream, ServerFnError> {
  Ok(TextStream::new(progress::progress_stream(id.clone())))
}

#[derive(Debug, Clone, Copy, Default)]
struct Progress {
  pub size: u64,
  pub uploaded: RwSignal<u64>,
}

async fn update_progress(id: String, uploading_files: RwSignal<HashMap<String, Progress>>) {
  let mut progress = file_progress(id.clone())
    .await
    .expect("couldn't initialize stream")
    .into_inner();

  while let Some(Ok(chunk)) = progress.next().await {
    let messages = chunk
      .split('\n')
      .filter_map(|line| line.split_once('\0'))
      .filter_map(|(id, size)| size.parse::<u64>().ok().map(|size| (id, size)));

    update!(|uploading_files| {
      for (id, size) in messages {
        let Some(&mut Progress { uploaded, .. }) = uploading_files.get_mut(id) else {
          logging::warn!("Got progress for unknown id '{id}'");
          continue;
        };

        update!(|uploaded| *uploaded = size);

        if uploaded.get_untracked() >= size {
          logging::log!("[{id}]\tfinished");
          uploading_files.remove(id);
        }
      }
    });
  }

  logging::log!("[{id}]\tfinished (stream)");
}

#[component]
pub fn FileUpload(path: Signal<PathBuf>) -> impl IntoView {
  let uploading_files = create_rw_signal(HashMap::<String, Progress>::new());

  let file_ref: NodeRef<Input> = create_node_ref();
  let form_ref: NodeRef<Form> = create_node_ref();

  let on_submit = move |ev: SubmitEvent| {
    ev.prevent_default();

    let form = form_ref().unwrap();
    let form_data = web_sys::FormData::new_with_form(&form).unwrap();

    let file_list = file_ref().unwrap().files().unwrap();

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

    if with!(|uploading_files| uploading_files.contains_key(&id)) {
      logging::warn!("Upload already in progress. Aborting.");
      return;
    }

    _ = form_data.set_with_str("id", &id);

    update!(|uploading_files| {
      _ = uploading_files.insert(
        id.clone(),
        Progress {
          size: total,
          uploaded: create_rw_signal(0),
        },
      );
    });

    spawn_local(update_progress(id, uploading_files));

    spawn_local(async move {
      upload_file(form_data.into())
        .await
        .expect("couldn't upload file");
    });
  };

  let ProgressBar = |Progress { size, uploaded }| view! { <p>Uploading: <progress max=size.to_string() value=uploaded></progress></p> };

  view! {
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
      <input type="hidden" name="id" value=""/>
      <input
        type="file"
        name="uploads"
        class="file-input file-input-bordered grow-[3]"
        multiple
        ref=file_ref
      />
      <button type="submit" class="btn btn-primary grow-[1]">
        Upload
      </button>
    </form>

    {move || {
        uploading_files
            .with(|map| map.values().map(|progress| ProgressBar(*progress)).collect::<Vec<_>>())
    }}
  }
}
