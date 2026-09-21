use std::path::PathBuf;

use leptos::prelude::*;

use crate::{
    api::{ListQuery, SortColumn, SortDir, list_dir},
    components::{
        EmptyState, FileEntries, ListingToolbar, Loading, PAGE_SIZE, Pagination, SortHeader,
    },
};

/// The interactive listing browser: toolbar, sorting, filtering,
/// entry list, pagination and empty states.
#[island]
pub fn ListingBrowser(path: PathBuf, allow_upload: bool) -> impl IntoView {
    let path = StoredValue::new(path);

    let page = RwSignal::new(0usize);
    let sort_column = RwSignal::new(SortColumn::Name);
    let sort_dir = RwSignal::new(SortDir::Asc);
    let search = RwSignal::new(String::new());
    let initial = RwSignal::new(None::<char>);
    let show_hidden = RwSignal::new(false);

    // Reset paging on changed sort/filter. Setting an already-zero page
    // does not notify, so this only refetches when shrinking a page.
    Effect::new(move |_| {
        let _ = (
            sort_column.get(),
            sort_dir.get(),
            search.get(),
            initial.get(),
            show_hidden.get(),
        );
        page.set(0);
    });

    let listing = Resource::new(
        move || ListQuery {
            path: path.get_value(),
            sort_column: sort_column.get(),
            sort_dir: sort_dir.get(),
            search: search.get(),
            initial: initial.get(),
            show_hidden: show_hidden.get(),
            limit: PAGE_SIZE,
            offset: page.get() * PAGE_SIZE,
        },
        list_dir,
    );

    let has_active_filter =
        Signal::derive(move || !search.get().is_empty() || initial.get().is_some());
    let clear_filter = Callback::new(move |()| {
        search.set(String::new());
        initial.set(None);
    });
    let show_hidden_files = Callback::new(move |()| show_hidden.set(true));

    view! {
        <div class="browser">
            <ListingToolbar
                search=search
                initial=initial
                show_hidden=show_hidden
                listing=listing
                on_clear=clear_filter
            />
            <SortHeader sort_column=sort_column sort_dir=sort_dir />
            <Transition fallback=Loading>
                {move || Suspend::new(async move {
                    match listing.await {
                        Ok(page_data) => {
                            if page_data.entries.is_empty() {
                                view! {
                                    <EmptyState
                                        has_filter=has_active_filter
                                        hidden_count=page_data.hidden_count
                                        allow_upload=allow_upload
                                        on_clear=clear_filter
                                        on_show_hidden=show_hidden_files
                                    />
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <FileEntries path=path.get_value() entries=page_data.entries />
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
