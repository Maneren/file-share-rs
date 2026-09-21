use leptos::prelude::*;

/// Fallback for empty listing pages: clear-action when filtered,
/// upload/hidden-file pointers when genuinely empty.
#[component]
pub fn EmptyState(
    #[prop(into)] has_filter: Signal<bool>,
    hidden_count: usize,
    allow_upload: bool,
    on_clear: Callback<()>,
    on_show_hidden: Callback<()>,
) -> impl IntoView {
    view! {
      <div class="flex flex-col items-center gap-2 py-10 text-center" role="status">
        <Show
          when=has_filter
          fallback=move || {
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
            }
          }
        >
          <p class="text-lg">"No files match the current filter."</p>
          <button class="btn btn-sm btn-primary" on:click=move |_| on_clear.run(())>
            "Clear search"
          </button>
        </Show>
      </div>
    }
}
