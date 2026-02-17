mod icon;

use std::path::PathBuf;

use icon::Icon;
use leptos::{either::Either, prelude::*};
use leptos_router::components::A;

use crate::{
    server::{Entries, ServerEntry},
    utils::{format_bytes, format_download_link, format_url_path},
};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq)]
pub enum EntryType {
    Folder,
    File,
}

#[component]
fn EntryComponent(
    type_: EntryType,
    href: String,
    name: String,
    size: Option<String>,
    relative_time: String,
) -> impl IntoView {
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
pub fn FileEntries(path: Signal<PathBuf>, mut entries: Entries) -> impl IntoView {
    if entries.is_empty() {
        return Either::Left(view! { <div class="file-view">"The folder is empty"</div> });
    }
    entries.sort_unstable();

    let path = path.get_untracked();

    Either::Right(view! {
      <div class="file-view">
        {entries
          .into_iter()
          .map(|entry| match entry {
            ServerEntry::File { name, size, last_modified } => {
              view! {
                <EntryComponent
                  type_=EntryType::File
                  href=format_download_link(&path, &name)
                  name=name
                  size=Some(format_bytes(size))
                  relative_time=last_modified.humanize()
                />
              }
            }
            ServerEntry::Folder { name, last_modified } => {
              view! {
                <EntryComponent
                  type_=EntryType::Folder
                  href=format_url_path(&path, &name)
                  name=name
                  size=None
                  relative_time=last_modified.humanize()
                />
              }
            }
          })
          .collect_view()}
      </div>
    })
}
