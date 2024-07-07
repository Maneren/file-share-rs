use std::{collections::HashMap, path::PathBuf};

use futures::StreamExt;
use leptos::{
  component, create_node_ref, create_signal,
  ev::SubmitEvent,
  html::{Form, Input},
  logging, server, spawn_local, view, IntoView, NodeRef, RwSignal, ServerFnError, Signal,
  SignalUpdate, SignalWith,
};
use server_fn::codec::{MultipartData, MultipartFormData, StreamingText, TextStream};
use web_sys::FormData;

#[cfg(feature = "ssr")]
mod progress;

#[component]
pub fn FileUpload(path: Signal<PathBuf>) -> impl IntoView {
  let upload_input: NodeRef<Input> = create_node_ref();
  let upload_path = move || path.with(|path| format!("/upload/{}", path.display()));

  let file_names_input: NodeRef<Input> = create_node_ref();

  let on_file_upload_submit = move |ev: SubmitEvent| {
    let files = upload_input().unwrap().files().unwrap();
    if files.length() == 0 {
      ev.prevent_default();
    }

    let file_names = (0..files.length())
      .map(|i| files.item(i).unwrap())
      .map(|file| file.name())
      .collect::<Vec<_>>()
      .join("\0");

    file_names_input().unwrap().set_value(&file_names);
  };

  view! {
    <form
      class="flex flex-wrap grow-[2] gap-2"
      action=upload_path
      method="POST"
      enctype="multipart/form-data"
      on:submit=on_file_upload_submit
    >
      <input
        type="file"
        name="uploads"
        class="file-input file-input-bordered grow-[3]"
        multiple
        node_ref=upload_input
      />
      <input type="hidden" name="path" value=upload_path/>
      <input type="hidden" name="file-list" node_ref=file_names_input/>
      <button type="submit" class="btn btn-primary grow-[1]">
        Upload
      </button>
    </form>

    <FileUploadWithProgress path=path/>
  }
}

#[server(input = MultipartFormData)]
pub async fn upload_file(data: MultipartData) -> Result<(), ServerFnError> {
  use axum::extract::Query;
  use leptos_axum::extract;
  use server_fn::ServerFnError::*;
  use tokio::{fs::File, io::AsyncWriteExt};

  // let Query(path): Query<String> = extract().await?;

  let Some(mut data) = data.into_inner() else {
    return Err(ServerError("No data".into()));
  };

  while let Ok(Some(mut field)) = data.next_field().await {
    let Some(name) = field.file_name().map(ToOwned::to_owned) else {
      return Err(ServerError("Missing file name in multipart".into()));
    };

    // let path = upload_path.join(&name);

    // let file = File::open(path).await?;

    while let Ok(Some(chunk)) = field.chunk().await {
      let len = chunk.len();

      logging::log!("[{name}]\t{len}");

      progress::add_chunk(&name, len).await;
      // file.write_all(&chunk).await?;
    }

    logging::log!("[{name}]\tfinished");
    progress::finish_file(&name);
  }

  Ok(())
}

#[server(output = StreamingText)]
pub async fn file_progress(filenames: Vec<String>) -> Result<TextStream, ServerFnError> {
  use futures_concurrency::stream::Merge;

  logging::log!("checking progress for {}", filenames.join(", "));

  let progress_streams = progress::progress_streams(&filenames)
    .into_iter()
    .map(|(filename, stream)| stream.map(move |bytes| Ok(format!("{filename}\0{bytes}\n"))))
    .collect::<Vec<_>>();

  logging::log!("sending {} streams", progress_streams.len());

  Ok(TextStream::new(progress_streams.merge()))
}

#[component]
pub fn FileUploadWithProgress(path: Signal<PathBuf>) -> impl IntoView {
  #[derive(Debug, Clone, Copy, Default)]
  struct UploadProgress {
    pub size: usize,
    pub uploaded: RwSignal<usize>,
  }

  // const BASE_PATH: Path = Path::from("/upload");

  // let upload_path = move || path.with(|path| BASE_PATH.join(path));

  let (uploading_files, set_uploading_files) = create_signal(HashMap::new());

  // let total = || {
  //   uploading_files()
  //     .values()
  //     .map(|f: &UploadProgress| f.size)
  //     .sum::<usize>()
  // };
  let (total, set_total) = create_signal(0);

  let file_ref: NodeRef<Input> = create_node_ref();
  let form_ref: NodeRef<Form> = create_node_ref();

  let on_submit = move |ev: SubmitEvent| {
    ev.prevent_default();

    if !uploading_files.with(|map| map.is_empty()) {
      logging::warn!("Upload already in progress. Aborting.");
      return;
    }

    let form = form_ref().unwrap();
    let form_data = FormData::new_with_form(&form).unwrap();

    let file_list = file_ref().unwrap().files().unwrap();

    let files = (0..file_list.length())
      .map(|i| file_list.item(i).unwrap())
      .collect::<Vec<_>>();

    set_uploading_files.update(|map| {
      *map = files
        .iter()
        .map(|f| (f.name(), Default::default()))
        .collect();
    });

    let filenames = files.iter().map(|f| f.name()).collect::<Vec<_>>();

    logging::log!("submitting '{}'", filenames.join(", "));

    spawn_local(async move {
      let mut progress = file_progress(filenames)
        .await
        .expect("couldn't initialize stream")
        .into_inner();

      while let Some(Ok(chunk)) = progress.next().await {
        let messages = chunk
          .split('\n')
          .filter_map(|line| line.split_once('\0'))
          .filter_map(|(file, size)| size.parse::<usize>().ok().map(|size| (file, size)));

        for (file, size) in messages {
          logging::log!("[{file}]\t{size}");
        }
      }
    });
    spawn_local(async move {
      upload_file(form_data.into())
        .await
        .expect("couldn't upload file");
    });
  };

  fn FileProgress(filename: String, progress: UploadProgress) -> impl IntoView {
    let UploadProgress { size, uploaded } = progress;
    view! { <p>{filename}: <progress max=size.to_string() value=uploaded></progress></p> }
  }

  view! {
    <form
      class="flex flex-wrap grow-[2] gap-2"
      method="POST"
      // action=upload_path
      enctype="multipart/form-data"
      node_ref=form_ref
      on:submit=on_submit
    >
      <input
        type="hidden"
        name="path"
        value=move || path.with(|path| path.to_string_lossy().into_owned())
      />
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

    // {move || filename().map(|filename: &str| view! { <p>Uploading {filename}</p> })}
    // {move || {
    //   max()
    //     .map(|max| {
    //       view! { <progress max=max value=move || current().unwrap_or_default()></progress> }
    //     })
    // }}
    // 

    {move || {
        uploading_files()
            .iter()
            .map(|(filename, progress)| FileProgress(filename.to_owned(), *progress))
            .collect::<Vec<_>>()
    }}
  }
}
