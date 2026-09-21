//! Upload-progress registry (SSR-only) + `file_progress` poll RPC.
//!
//! Tracks in-flight browser uploads by id so the client can poll
//! `file_progress` while `upload_file` streams chunks. Moved out of
//! `components/upload/` — this is server state, not UI.
//!
//! The registry itself is `#[cfg(feature = "ssr")]`; the `#[server]`
//! function stays available on all targets so the WASM client can call it.

use cfg_if::cfg_if;
use leptos::prelude::*;
use server_fn::codec::{StreamingText, TextStream};

cfg_if! { if #[cfg(feature = "ssr")] {
    use std::{collections::HashMap, sync::LazyLock};

    use async_broadcast::{Receiver, Sender, broadcast};
    use futures::StreamExt;
    use tokio::sync::Mutex;
    use tokio_stream::Stream;
}}

#[cfg(feature = "ssr")]
struct FileHandle {
    total: usize,
    tx: Sender<usize>,
    rx: Receiver<usize>,
}

#[cfg(feature = "ssr")]
impl Default for FileHandle {
    fn default() -> Self {
        let (mut tx, rx) = broadcast(8);
        tx.set_overflow(true);
        Self { total: 0, tx, rx }
    }
}

#[cfg(feature = "ssr")]
static FILES: LazyLock<Mutex<HashMap<String, FileHandle>>> = LazyLock::new(Default::default);

#[cfg(feature = "ssr")]
pub async fn add_chunk(id: &str, len: usize) {
    let mut lock = FILES.lock().await;
    let entry = lock.entry(id.to_owned()).or_insert_with(|| {
        leptos::logging::log!("[{id}]\tinserting channel (chunk)");
        FileHandle::default()
    });

    entry.total += len;
    let new_total = entry.total;

    // we're about to do an async broadcast, so we don't want to hold a lock
    // across it
    let tx = entry.tx.clone();
    drop(lock);

    tx.broadcast(new_total)
        .await
        .expect("couldn't send a message over channel");
}

#[cfg(feature = "ssr")]
pub async fn progress_stream(id: String) -> impl Stream<Item = Result<String, ServerFnError>> {
    let mut lock = FILES.lock().await;
    let entry = lock.entry(id.clone()).or_insert_with(|| {
        leptos::logging::log!("[{id}]\tinserting channel (progress)");
        FileHandle::default()
    });

    entry
        .rx
        .clone()
        .map(move |bytes| format!("{id}\0{bytes}\n"))
        .map(Ok)
}

#[cfg(feature = "ssr")]
pub async fn finish(filename: &str) {
    let mut lock = FILES.lock().await;

    if let Some(entry) = lock.get_mut(filename) {
        entry.tx.close();
        entry.rx.close();
        leptos::logging::log!("[{filename}]\tstream closed");
    }

    lock.remove(filename);
}

#[allow(clippy::unused_async)]
#[server(output = StreamingText)]
pub async fn file_progress(id: String) -> Result<TextStream, ServerFnError> {
    Ok(TextStream::new(progress_stream(id.clone()).await))
}
