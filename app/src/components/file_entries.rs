mod icon;

use std::path::PathBuf;

use icon::{Icon, get_file_icon, get_folder_icon};
use leptos::{either::Either, prelude::*};

use crate::{
    server::{Entries, ServerEntry},
    utils::{format_bytes, format_file_href, format_folder_href},
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
    icon: &'static str,
    size: Option<String>,
    relative_time: String,
) -> impl IntoView {
    let inner = view! {
      <div class="grid gap-2 w-full entry grid-cols-(--entry-cols-mobile) md:grid-cols-(--entry-cols)">
        <Icon icon=icon />
        <span
          class="flex overflow-hidden text-ellipsis whitespace-nowrap items-center"
          title=name.clone()
        >
          {name.clone()}
        </span>
        <span class="flex justify-end items-center">{size}</span>
        <span class="hidden items-center md:flex" title=relative_time.clone()>
          {relative_time.clone()}
        </span>
      </div>
    };

    if type_ == EntryType::Folder {
        Either::Left(view! { <a href=href>{inner}</a> })
    } else {
        Either::Right(view! {
          <a href=href download>
            {inner}
          </a>
        })
    }
}

#[component]
pub fn FileEntries(path: PathBuf, entries: Entries) -> impl IntoView {
    let path = StoredValue::new(path);
    let entries = StoredValue::new(entries);

    view! {
      <div class="file-view">
        <For
          each=move || entries.get_value()
          key=|entry| match entry {
            ServerEntry::File { name, .. } => format!("f:{name}"),
            ServerEntry::Folder { name, .. } => format!("d:{name}"),
          }
          let:entry
        >
          {match entry {
            ServerEntry::File { name, size, last_modified } => {
              let base = path.get_value();
              let icon = get_file_icon(&name);
              view! {
                <EntryComponent
                  type_=EntryType::File
                  href=format_file_href(&base, &name)
                  name=name
                  icon=icon
                  size=Some(format_bytes(size))
                  relative_time=last_modified.humanize()
                />
              }
                .into_any()
            }
            ServerEntry::Folder { name, last_modified } => {
              let base = path.get_value();
              let icon = get_folder_icon(&name);
              view! {
                <EntryComponent
                  type_=EntryType::Folder
                  href=format_folder_href(&base, &name)
                  name=name
                  icon=icon
                  size=None
                  relative_time=last_modified.humanize()
                />
              }
                .into_any()
            }
          }}
        </For>
      </div>
    }
}
