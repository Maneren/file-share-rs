mod icon;

use std::path::PathBuf;

use icon::Icon;
use leptos::{either::Either, prelude::*};
use leptos_router::components::A;

use crate::{
    components::StreamDownloadButton,
    server::{Entries, ServerEntry},
    utils::{format_bytes, format_file_href, format_folder_href, format_stream_href},
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
    #[prop(optional)] stream_url: Option<String>,
) -> impl IntoView {
    let inner = view! {
      <div class="grid gap-2 w-full entry grid-cols-(--entry-cols-mobile) md:grid-cols-(--entry-cols)">
        <Icon type_=type_ name=name.clone() />
        <span class="flex overflow-x-hidden items-center">{name.clone()}</span>
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
        // Plain `<A download>` stays as the fallback for old browsers; the
        // stream button next to it does fetch -> DecompressionStream ->
        // showSaveFilePicker without buffering the file in RAM.
        Either::Right(view! {
          <div class="flex gap-1 items-center min-w-0">
            <A href=href.clone() attr:download attr:class="grow min-w-0">
              {inner}
            </A>
            <StreamDownloadButton
              download_url=stream_url.unwrap_or_default()
              filename=name.clone()
            />
          </div>
        })
    }
}

#[component]
pub fn FileEntries(path: Signal<PathBuf>, entries: Entries) -> impl IntoView {
    if entries.is_empty() {
        return Either::Left(view! { <div class="file-view">"The folder is empty"</div> });
    }

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
                  href=format_file_href(&path, &name)
                  stream_url=format_stream_href(&path, &name)
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
                  href=format_folder_href(&path, &name)
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
