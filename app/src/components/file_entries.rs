mod icon;

use std::path::PathBuf;

use icon::Icon;
use leptos::{either::Either, prelude::*};
use leptos_router::components::A;

use crate::{
    server::Entries,
    utils::{encode_path, format_bytes},
};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq)]
pub enum EntryType {
    Folder,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entry {
    type_: EntryType,
    href: String,
    name: String,
    size: Option<String>,
    relative_time: String,
}

#[component]
pub fn EntryComponent(entry: Entry) -> impl IntoView {
    let Entry {
        type_,
        href,
        name,
        size,
        relative_time,
    } = entry;

    let inner = view! {
      <div class="grid gap-2 w-full entry grid-cols-(--entry-cols-mobile) md:grid-cols-(--entry-cols)">
        <Icon type_=type_ name=name.clone() />
        <span class="flex overflow-x-hidden items-center">{name}</span>
        <span class="flex justify-end items-center">{size}</span>
        <span class="hidden items-center md:flex">{relative_time}</span>
      </div>
    };

    if type_ == EntryType::Folder {
        Either::Left(view! {
          <A href=href exact=true>
            {inner}
          </A>
        })
    } else {
        Either::Right(view! {
          <A href=href attr:download>
            {inner}
          </A>
        })
    }
}

#[component]
pub fn FileEntries(path: Signal<PathBuf>, entries: Entries) -> impl IntoView {
    use crate::server::ServerEntry;
    if entries.is_empty() {
        return Either::Left(view! { <div class="file-view">"The folder is empty"</div> });
    }

    let path = path.get_untracked();

    let mut entries = entries
        .into_iter()
        .map(|entry| match entry {
            ServerEntry::File {
                name,
                size,
                last_modified,
            } => Entry {
                type_: EntryType::File,
                href: format!("/files/{}", encode_path(path.join(&name))),
                name: name.clone(),
                size: Some(format_bytes(size)),
                relative_time: last_modified.humanize(),
            },
            ServerEntry::Folder {
                name,
                last_modified,
            } => Entry {
                type_: EntryType::Folder,
                href: format!("/index/{}", encode_path(path.join(&name))),
                name: name.clone(),
                size: None,
                relative_time: last_modified.humanize(),
            },
        })
        .collect::<Vec<_>>();

    entries.sort_unstable();

    Either::Right(view! {
      <div class="file-view">
        <For each=move || entries.clone() key=|entry| entry.href.clone() let:entry>
          <EntryComponent entry=entry />
        </For>
      </div>
    })
}
