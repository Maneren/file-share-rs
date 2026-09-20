use leptos::{either::Either, prelude::*};

/// Fallback shown when a listing page is empty.
///
/// Distinguishes a filtered-out view (with an action to clear the filter)
/// from a genuinely empty folder (pointing at the upload controls and any
/// hidden files).
#[component]
pub fn EmptyState(
    has_filter: bool,
    hidden_count: usize,
    allow_upload: bool,
    on_clear: Callback<()>,
    on_show_hidden: Callback<()>,
) -> impl IntoView {
    view! {
      <div class="flex flex-col items-center gap-2 py-10 text-center" role="status">
        {if has_filter {
          Either::Left(
            view! {
              <p class="text-lg">"No files match the current filter."</p>
              <button class="btn btn-sm btn-primary" on:click=move |_| on_clear.run(())>
                "Clear search"
              </button>
            },
          )
        } else {
          Either::Right(
            view! {
              <p class="text-lg">"This folder is empty."</p>
              <Show when=move || { hidden_count > 0 }>
                <button class="btn btn-sm btn-primary" on:click=move |_| on_show_hidden.run(())>
                  {move || {
                    if hidden_count == 1 {
                      "Show 1 hidden file".to_owned()
                    } else {
                      format!("Show {hidden_count} hidden files")
                    }
                  }}
                </button>
              </Show>
              <Show when=move || allow_upload>
                <p class="text-sm opacity-70">"Upload files or create a folder to get started."</p>
              </Show>
            },
          )
        }}
      </div>
    }
}
