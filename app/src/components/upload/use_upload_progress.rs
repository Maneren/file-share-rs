use leptos::{logging, prelude::*};

use super::{file_progress, progress_bar::Progress};

pub async fn update_progress(id: String, upload: RwSignal<Option<(String, Progress)>>) {
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
