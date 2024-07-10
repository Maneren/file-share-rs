use std::sync::LazyLock;

use async_broadcast::{broadcast, Receiver, Sender};
use dashmap::DashMap;
use futures::{Stream, StreamExt};
use leptos::*;

struct FileHandle {
  total: usize,
  tx: Sender<Option<usize>>,
  rx: Receiver<Option<usize>>,
}

impl Default for FileHandle {
  fn default() -> Self {
    let (mut tx, rx) = broadcast(16);
    tx.set_overflow(true);
    Self { total: 0, tx, rx }
  }
}

static FILES: LazyLock<DashMap<String, FileHandle>> = LazyLock::new(DashMap::new);

pub async fn add_chunk(id: &str, len: usize) {
  let mut entry = FILES.entry(id.to_owned()).or_insert_with(|| {
    logging::log!("[{id}]\tinserting channel (chunk)");
    Default::default()
  });

  entry.total += len;
  let new_total = entry.total;

  // we're about to do an async broadcast, so we don't want to hold a lock across it
  let tx = entry.tx.clone();
  drop(entry);

  logging::log!("[{id}]\tbroadcasting new total {new_total}");

  tx.broadcast(Some(new_total))
    .await
    .expect("couldn't send a message over channel");
}

pub fn progress_stream(id: String) -> impl Stream<Item = Result<String, ServerFnError>> {
  let entry = FILES.entry(id.to_owned()).or_insert_with(|| {
    logging::log!("[{id}]\tinserting channel (progress)");
    Default::default()
  });

  entry
    .rx
    .clone()
    .map(move |bytes| match bytes {
      Some(bytes) => format!("{id}\0{bytes}\n"),
      None => format!("{id}\n"),
    })
    .map(Ok)
}

pub async fn finish_file(filename: &str) {
  if let Some(entry) = FILES.get_mut(filename) {
    entry
      .tx
      .broadcast(None)
      .await
      .expect("couldn't send a message over channel");

    entry.tx.close();
    entry.rx.close();
  }

  FILES.remove(filename);
}
