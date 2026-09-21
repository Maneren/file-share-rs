use leptos::prelude::*;

use crate::api::{SortColumn, SortDir};

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
        if sort_column.get() == column {
            match sort_dir.get() {
                SortDir::Asc => "▲",
                SortDir::Desc => "▼",
            }
        } else {
            " "
        }
    };

    view! {
        <div class="grid gap-2 mb-1 border-b grid-cols-(--entry-cols-mobile) border-base-content md:grid-cols-(--entry-cols)">
            <span></span>
            {[SortColumn::Name, SortColumn::Size, SortColumn::Modified]
                .map(|column| {
                    view! {
                        <button
                            class="flex items-center gap-3"
                            on:click=move |_| toggle_sort(column)
                        >
                            {match column {
                                SortColumn::Name => "Name",
                                SortColumn::Size => "Size",
                                SortColumn::Modified => "Last Modified",
                            }}
                            <span class="ml-2">{move || indicator(column)}</span>
                        </button>
                    }
                })}
        </div>
    }
}
