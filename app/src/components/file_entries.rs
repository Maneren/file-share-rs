mod icon;

use std::path::PathBuf;

use icon::Icon;
use leptos::{either::Either, prelude::*};
use leptos_router::components::A;

use crate::{server::Entries, utils::format_bytes};

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

pub fn EntryComponent(data: Entry) -> impl IntoView {
    let Entry {
        type_,
        href,
        name,
        size,
        relative_time,
    } = data;

    let inner = view! {
      <div class="entry w-full grid grid-cols-entry-mobile md:grid-cols-entry gap-2">
        <Icon type_=type_ name=name.clone() />
        <span class="flex items-center overflow-x-hidden">{name}</span>
        <span class="flex items-center justify-end">{size}</span>
        <span class="hidden md:flex items-center">{relative_time}</span>
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
                href: format!("/files/{}", path.join(&name).display()),
                name: name.clone(),
                size: Some(format_bytes(size)),
                relative_time: last_modified.humanize(),
            },
            ServerEntry::Folder {
                name,
                last_modified,
            } => Entry {
                type_: EntryType::Folder,
                href: format!("/index/{}", path.join(&name).display()),
                name: name.clone(),
                size: None,
                relative_time: last_modified.humanize(),
            },
        })
        .collect::<Vec<_>>();

    entries.sort_unstable();

    Either::Right(
        view! { <div class="file-view">{entries.into_iter().map(EntryComponent).collect_view()}</div> },
    )
}
