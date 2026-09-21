//! Pagination controls for the listing browser.
//!
//! Page-window math lives in [`crate::pagination::page_window`]; `PAGE_SIZE`
//! lives here so the server `limit` and the UI stay in sync via one import.

use leptos::prelude::*;

use crate::pagination::page_window;

/// Entries per directory page.
pub const PAGE_SIZE: usize = 50;

#[component]
pub fn Pagination(page: RwSignal<usize>, total: usize) -> impl IntoView {
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
                                    view! {
                                        <span class="join-item btn btn-sm btn-disabled">"…"</span>
                                    }
                                        .into_any()
                                }
                                Some(p) => {
                                    let label = (p + 1).to_string();
                                    if p == page.get() {
                                        view! {
                                            <button class="join-item btn btn-sm btn-primary">
                                                {label}
                                            </button>
                                        }
                                            .into_any()
                                    } else {
                                        view! {
                                            <button
                                                class="join-item btn btn-sm"
                                                on:click=move |_| page.set(p)
                                            >
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
