use std::{collections::HashMap, sync::LazyLock};

use async_broadcast::{Receiver, Sender, broadcast};
use futures::StreamExt;
use leptos::{logging, prelude::*};
use tokio::sync::Mutex;
use tokio_stream::Stream;

struct FileHandle {
    total: usize,
    tx: Sender<usize>,
    rx: Receiver<usize>,
}

impl Default for FileHandle {
    fn default() -> Self {
        let (mut tx, rx) = broadcast(8);
        tx.set_overflow(true);
        Self { total: 0, tx, rx }
    }
}

static FILES: LazyLock<Mutex<HashMap<String, FileHandle>>> = LazyLock::new(Default::default);

pub async fn add_chunk(id: &str, len: usize) {
    let mut lock = FILES.lock().await;
    let entry = lock.entry(id.to_owned()).or_insert_with(|| {
        logging::log!("[{id}]\tinserting channel (chunk)");
        FileHandle::default()
    });

    entry.total += len;
    let new_total = entry.total;

    // we're about to do an async broadcast, so we don't want to hold a lock across
    // it
    let tx = entry.tx.clone();
    drop(lock);

    tx.broadcast(new_total)
        .await
        .expect("couldn't send a message over channel");
}

pub async fn progress_stream(id: String) -> impl Stream<Item = Result<String, ServerFnError>> {
    let mut lock = FILES.lock().await;
    let entry = lock.entry(id.clone()).or_insert_with(|| {
        logging::log!("[{id}]\tinserting channel (progress)");
        FileHandle::default()
    });

    entry
        .rx
        .clone()
        .map(move |bytes| format!("{id}\0{bytes}\n"))
        .map(Ok)
}

pub async fn finish(filename: &str) {
    let mut lock = FILES.lock().await;

    if let Some(entry) = lock.get_mut(filename) {
        entry.tx.close();
        entry.rx.close();
        logging::log!("[{filename}]\tstream closed");
    }

    lock.remove(filename);
}
