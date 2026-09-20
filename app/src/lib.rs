#![allow(non_snake_case)]

use std::{path::PathBuf, sync::Arc};

pub mod archive;
mod components;
mod config;
mod error_template;
mod server;
#[cfg(feature = "ssr")]
mod state;
pub mod utils;

use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    components::{Route, Router, Routes},
    hooks::use_params,
    params::Params,
};
use leptos_router_macro::path;
use urlencoding::decode;

pub use crate::config::{
    AppConfig, PATH_NOT_FOUND_MESSAGE, UPLOAD_DISABLED_MESSAGE, UPLOAD_READ_ERROR_MESSAGE,
    UPLOAD_STORE_ERROR_MESSAGE,
};
#[cfg(feature = "ssr")]
pub use crate::state::AppState;
use crate::{
    components::{Breadcrumbs, EmptyState, FileEntries, Loading, UploadBar},
    error_template::{AppError, ErrorTemplate},
    server::{ListQuery, NewFolder, SortColumn, SortDir, list_dir},
    utils::page_window,
};

#[derive(PartialEq, Eq, Params, Debug)]
struct PathQuery {
    path: String,
}

/// Entries per directory page.
const PAGE_SIZE: usize = 100;

#[component]
#[allow(clippy::must_use_candidate)]
pub fn FilesPage() -> impl IntoView {
    let path_query = use_params::<PathQuery>();

    let path = Memo::new(move |_| {
        path_query
            .read()
            .as_ref()
            .ok()
            .and_then(|query| decode(&query.path).ok())
            .map_or_default(|path| PathBuf::from(path.as_ref()))
    });

    let create_folder_action = ServerAction::<NewFolder>::new();

    let page = RwSignal::new(0usize);
    let sort_column = RwSignal::new(SortColumn::Name);
    let sort_dir = RwSignal::new(SortDir::Asc);
    let search = RwSignal::new(String::new());
    let initial = RwSignal::new(None::<char>);
    let show_hidden = RwSignal::new(false);

    // Reset paging on navigation, new folders, or changed sort/filter.
    // Setting an already-zero page does not notify, so this only
    // refetches when shrinking a page.
    Effect::new(move |_| {
        let _ = (
            path.get(),
            create_folder_action.version().get(),
            sort_column.get(),
            sort_dir.get(),
            search.get(),
            initial.get(),
            show_hidden.get(),
        );
        page.set(0);
    });

    let listing = Resource::new(
        move || {
            (
                path.get(),
                create_folder_action.version().get(),
                page.get(),
                sort_column.get(),
                sort_dir.get(),
                search.get(),
                initial.get(),
                show_hidden.get(),
            )
        },
        |(path, _, page, sort_column, sort_dir, search, initial, show_hidden)| async move {
            // NOTE: the `Ok` error type must be annotated. Nothing else
            // pins it, and an ambiguous error type surfaces as bogus
            // `FnMut`/`IntoView` errors on the `Transition` below instead
            // of an inference error.
            Ok::<_, ServerFnError>(
                list_dir(ListQuery {
                    path,
                    sort_column,
                    sort_dir,
                    search,
                    initial,
                    show_hidden,
                    limit: PAGE_SIZE,
                    offset: page * PAGE_SIZE,
                })
                .await?,
            )
        },
    );

    // Clamp the page when filtering shrinks the listing below it.
    // Converges: setting the already-correct page does not notify.
    Effect::new(move |_| {
        if let Some(Ok(page_data)) = listing.get() {
            let pages = page_data.total.div_ceil(PAGE_SIZE);
            if pages > 0 && page.get() >= pages {
                page.set(pages - 1);
            }
        }
    });

    let path_signal = Signal::from(path);

    let app_config = expect_context::<Arc<AppConfig>>();
    let allow_upload = app_config.allow_upload;

    let has_active_filter = Memo::new(move |_| !search.get().is_empty() || initial.get().is_some());
    let clear_filter = Callback::new(move |()| {
        search.set(String::new());
        initial.set(None);
    });
    let show_hidden_files = Callback::new(move |()| show_hidden.set(true));

    let upload_bar = allow_upload.then(|| {
        view! { <UploadBar path=path_signal create_folder_action=create_folder_action /> }
    });

    let toggle_sort = move |column: SortColumn| {
        if sort_column.get() == column {
            sort_dir.update(|dir| {
                *dir = match dir {
                    SortDir::Asc => SortDir::Desc,
                    SortDir::Desc => SortDir::Asc,
                };
            });
        } else {
            sort_column.set(column);
            sort_dir.set(SortDir::Asc);
        }
    };
    let indicator = move |column: SortColumn| {
        (sort_column.get() == column)
            .then(|| match sort_dir.get() {
                SortDir::Asc => "▲",
                SortDir::Desc => "▼",
            })
            .unwrap_or_default()
    };

    view! {
      <div class="p-3 App">
        {upload_bar} <Breadcrumbs path=path_signal />
        <div class="flex flex-wrap items-center gap-2 py-1">
          <input
            type="search"
            placeholder="Search…"
            aria-label="Search files"
            class="input input-sm input-bordered grow"
            prop:value=move || search.get()
            on:input=move |ev| search.set(event_target_value(&ev))
          />
          <Show when=move || !search.get().is_empty()>
            <button
              class="btn btn-sm btn-ghost"
              on:click=move |_| {
                search.set(String::new());
                initial.set(None);
              }
            >
              "Clear"
            </button>
          </Show>
          <label class="flex items-center gap-2 text-sm cursor-pointer">
            <input
              type="checkbox"
              class="toggle toggle-sm"
              aria-label="Show hidden files"
              prop:checked=move || show_hidden.get()
              on:change=move |ev| show_hidden.set(event_target_checked(&ev))
            />
            "Hidden"
          </label>
        </div>
        <div class="flex flex-wrap gap-1 py-1" role="group" aria-label="Filter by initial letter">
          <button
            class="btn btn-xs"
            class:btn-active=move || initial.get().is_none()
            on:click=move |_| initial.set(None)
          >
            "All"
          </button>
          <For each=|| 'A'..='Z' key=|letter| *letter let:letter>
            <button
              class="btn btn-xs"
              class:btn-active=move || initial.get() == Some(letter)
              on:click=move |_| initial.set(Some(letter))
            >
              {letter.to_string()}
            </button>
          </For>
        </div>
        <div class="grid gap-2 mb-1 border-b grid-cols-(--entry-cols-mobile) border-base-content md:grid-cols-(--entry-cols)">
          <span></span>
          <button
            class="flex items-center gap-1 text-left"
            on:click=move |_| toggle_sort(SortColumn::Name)
          >
            "Name"
            <span>{move || indicator(SortColumn::Name)}</span>
          </button>
          <button
            class="flex justify-end items-center gap-1"
            on:click=move |_| toggle_sort(SortColumn::Size)
          >
            "Size"
            <span>{move || indicator(SortColumn::Size)}</span>
          </button>
          <button
            class="hidden items-center gap-1 md:flex"
            on:click=move |_| toggle_sort(SortColumn::Modified)
          >
            "Last Modified"
            <span>{move || indicator(SortColumn::Modified)}</span>
          </button>
        </div>
        <Transition fallback=Loading>
          {move || Suspend::new(async move {
            match listing.await {
              Ok(page_data) => {
                if page_data.entries.is_empty() {
                  view! {
                    <EmptyState
                      has_filter=has_active_filter.get()
                      hidden_count=page_data.hidden_count
                      allow_upload=allow_upload
                      on_clear=clear_filter
                      on_show_hidden=show_hidden_files
                    />
                  }
                    .into_any()
                } else {
                  view! {
                    <FileEntries path=path_signal entries=page_data.entries />
                    <Pagination page=page total=page_data.total />
                  }
                    .into_any()
                }
              }
              Err(e) => view! { <p class="text-lg">{format!("{e}")}</p> }.into_any(),
            }
          })}
        </Transition>
      </div>
    }
}

#[component]
fn Pagination(page: RwSignal<usize>, total: usize) -> impl IntoView {
    let pages = total.div_ceil(PAGE_SIZE);

    view! {
      <div class="flex flex-col items-center gap-1 my-2">
        <span class="text-sm opacity-70">
          {move || {
            if total == 0 {
              "No entries".to_owned()
            } else {
              let start = page.get() * PAGE_SIZE + 1;
              let end = ((page.get() + 1) * PAGE_SIZE).min(total);
              format!("Showing {start}–{end} of {total}")
            }
          }}
        </span>
        <Show when=move || { pages > 1 }>
          <div class="join">
            <button
              class="join-item btn btn-sm"
              disabled=move || page.get() == 0
              on:click=move |_| page.set(0)
            >
              "«"
            </button>
            <button
              class="join-item btn btn-sm"
              disabled=move || page.get() == 0
              on:click=move |_| page.update(|page| *page = page.saturating_sub(1))
            >
              "‹"
            </button>
            {move || {
              page_window(page.get(), pages)
                .into_iter()
                .map(|item| match item {
                  None => {
                    view! { <span class="join-item btn btn-sm btn-disabled">"…"</span> }
                      .into_any()
                  }
                  Some(p) => {
                    let label = (p + 1).to_string();
                    if p == page.get() {
                      view! { <button class="join-item btn btn-sm btn-active">{label}</button> }
                        .into_any()
                    } else {
                      view! {
                        <button class="join-item btn btn-sm" on:click=move |_| page.set(p)>
                          {label}
                        </button>
                      }
                        .into_any()
                    }
                  }
                })
                .collect_view()
            }}
            <button
              class="join-item btn btn-sm"
              disabled=move || { page.get() + 1 >= pages }
              on:click=move |_| page.update(|page| *page += 1)
            >
              "›"
            </button>
            <button
              class="join-item btn btn-sm"
              disabled=move || { page.get() + 1 >= pages }
              on:click=move |_| page.set(pages - 1)
            >
              "»"
            </button>
          </div>
        </Show>
      </div>
    }
}

#[must_use]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
      <!DOCTYPE html>
      <html lang="en">
        <head>
          <meta charset="utf-8" />
          <meta name="viewport" content="width=device-width, initial-scale=1" />
          <link rel="shortcut icon" type="image/ico" href="/favicon.ico" />
          <AutoReload options=options.clone() />
          <HydrationScripts options islands=true />
          <MetaTags />
        </head>
        <body>
          <App />
        </body>
      </html>
    }
}

#[component]
#[allow(clippy::must_use_candidate)]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
      <Router>
        <Stylesheet id="leptos" href="/pkg/file-share.css" />
        <Title text="File Share" />
        <Routes fallback=|| {
          let mut outside_errors = Errors::default();
          outside_errors.insert_with_default_key(AppError::NotFound);
          view! { <ErrorTemplate outside_errors /> }.into_view()
        }>
          <Route path=path!("/index/*path") view=FilesPage />
        </Routes>
      </Router>
    }
}
