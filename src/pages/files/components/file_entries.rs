use std::path::PathBuf;

use leptos::*;
use leptos_router::A;

use crate::{
  pages::files::{server::Entries, utils::get_file_icon},
  utils::{format_bytes, SystemTime},
};

#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq)]
pub enum EntryType {
  File,
  Folder,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entry {
  type_: EntryType,
  href: String,
  icon: String,
  name: String,
  size: Option<String>,
  last_modified: SystemTime,
  relative_time: String,
}

#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn Icon(icon: String) -> impl IntoView {
  view! { <img class="icon h-10 w-10" src=format!("/icons/{icon}.svg") alt=format!("{icon} icon")/> }
}

pub fn EntryComponent(data: Entry) -> impl IntoView {
  let Entry {
    type_,
    href,
    icon,
    name,
    size,
    last_modified,
    relative_time,
  } = data;

  let inner = view! {
    <div class="entry w-full grid grid-cols-entry-mobile md:grid-cols-entry gap-2">
      <Icon icon=icon/>
      <span class="flex items-center overflow-x-hidden">{name}</span>
      <span class="flex items-center justify-end">{size}</span>
      <span class="hidden md:flex items-center justify-center">{last_modified}</span>
      <span class="hidden md:flex items-center">{relative_time}</span>
    </div>
  };

  if type_ == EntryType::Folder {
    view! { <A href=href>{inner}</A> }.into_view()
  } else {
    view! {
      <a href=href download>
        {inner}
      </a>
    }
    .into_view()
  }
}

#[component]
pub fn FileEntries(path: Signal<PathBuf>, entries: Entries) -> impl IntoView {
  use crate::pages::files::server::ServerEntry;
  if entries.is_empty() {
    return view! { <div class="file-view">"The folder is empty"</div> };
  }
  let path = path.with_untracked(|path| path.to_string_lossy().to_string());

  let mut entries = entries
    .into_iter()
    .map(|entry| match entry {
      ServerEntry::File {
        name,
        size,
        last_modified,
      } => Entry {
        type_: EntryType::File,
        href: format!("/files/{path}/{name}"),
        icon: get_file_icon(&name),
        name: name.clone(),
        size: Some(format_bytes(size)),
        last_modified,
        relative_time: last_modified.humanize(),
      },
      ServerEntry::Folder {
        name,
        last_modified,
      } => Entry {
        type_: EntryType::Folder,
        href: format!("/index/{path}/{name}"),
        icon: "folder".into(),
        name: name.clone(),
        size: None,
        last_modified,
        relative_time: last_modified.humanize(),
      },
    })
    .collect::<Vec<_>>();

  entries.sort_unstable();

  view! {
    <div class="file-view">
      {entries.into_iter().map(|entry| EntryComponent(entry)).collect_view()}
    </div>
  }
}
