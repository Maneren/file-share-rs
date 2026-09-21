//! Client-side polling hook for upload progress.
//!
//! Subscribes to the [`crate::api::upload_progress::file_progress`] text
//! stream and folds samples into [`super::state::Progress`].

use leptos::{logging, prelude::*};
use web_time::Instant;

use super::state::Progress;
use crate::api::upload_progress::file_progress;

pub async fn subscribe_upload_progress(id: String, upload: RwSignal<Option<(String, Progress)>>) {
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

            for (id, size) in messages {
                if id != stored_id {
                    logging::warn!("Got progress for unknown id '{id}'");
                    continue;
                }

                uploaded.update(|uploaded| {
                    if uploaded.len() >= 10 {
                        uploaded.pop_front();
                    }

                    uploaded.push_back((size, Instant::now()));
                });
            }
        });
    }

    logging::log!("[{id}]\tfinished (stream)");

    upload.write().take();
}
