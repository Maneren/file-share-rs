use leptos::prelude::*;

use crate::server::{SortColumn, SortDir};

/// Sortable listing header. Clicking a column selects it, clicking again
/// flips the direction.
#[component]
pub fn SortHeader(sort_column: RwSignal<SortColumn>, sort_dir: RwSignal<SortDir>) -> impl IntoView {
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
    }
}
