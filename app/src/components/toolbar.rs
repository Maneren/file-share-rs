use leptos::prelude::*;

/// Search box, hidden-file toggle and initial-letter strip.
#[component]
pub fn Toolbar(
    search: RwSignal<String>,
    initial: RwSignal<Option<char>>,
    show_hidden: RwSignal<bool>,
    initials: Memo<Vec<char>>,
    on_clear: Callback<()>,
) -> impl IntoView {
    view! {
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
          <button class="btn btn-sm btn-ghost" on:click=move |_| on_clear.run(())>
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
        <Suspense fallback=|| {
          view! { <span class="text-xs opacity-50">"A–Z"</span> }
        }>
          {move || {
            let present = initials.get();
            // Read here, not in the caller: the resource must be first
            // touched inside suspense, or hydration warns about reads
            // outside a suspense boundary.
            view! {
              <For each=|| 'A'..='Z' key=|letter| *letter let:letter>
                <button
                  class="btn btn-xs"
                  class:btn-active=move || initial.get() == Some(letter)
                  disabled={
                    let present = present.clone();
                    move || !present.contains(&letter)
                  }
                  on:click=move |_| initial.set(Some(letter))
                >
                  {letter.to_string()}
                </button>
              </For>
            }
          }}
        </Suspense>
      </div>
    }
}
