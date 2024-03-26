use async_broadcast::{broadcast, Receiver, Sender};
use dashmap::DashMap;
use futures::Stream;
use leptos::logging;
use once_cell::sync::Lazy;

struct FileHandle {
  total: usize,
  tx: Sender<usize>,
  rx: Receiver<usize>,
}

impl Default for FileHandle {
  fn default() -> Self {
    let (tx, rx) = broadcast(128);
    Self { total: 0, tx, rx }
  }
}

static FILES: Lazy<DashMap<String, FileHandle>> = Lazy::new(DashMap::new);

pub async fn add_chunk(filename: &str, len: usize) {
  logging::log!("[{filename}]\trecieved {len} B");

  let mut entry = FILES.entry(filename.to_owned()).or_insert_with(|| {
    logging::log!("[{filename}]\tinserting channel (chunk)");
    Default::default()
  });

  entry.total += len;
  let new_total = entry.total;

  // we're about to do an async broadcast, so we don't want to hold a lock across it
  let tx = entry.tx.clone();
  drop(entry);

  logging::log!("[{filename}]\tbroadcasting new total {new_total}");

  // now we send the message and don't have to worry about it
  tx.broadcast(new_total)
    .await
    .expect("couldn't send a message over channel");
}

pub fn progress_streams(filenames: &[String]) -> Vec<(String, impl Stream<Item = usize>)> {
  filenames
    .iter()
    .map(|filename| {
      let entry = FILES.entry(filename.to_owned()).or_insert_with(|| {
        logging::log!("[{filename}]\tinserting channel (progress)");
        Default::default()
      });

      (filename.to_owned(), entry.rx.clone())
    })
    .inspect(|(filename, _)| logging::log!("[{filename}]\tsending stream"))
    .collect()
}

pub fn finish_file(filename: &str) {
  FILES.remove(filename);
}
