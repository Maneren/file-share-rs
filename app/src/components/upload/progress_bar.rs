use leptos::prelude::*;
use web_time::Instant;

use crate::utils::format_bytes;

#[derive(Debug, Clone, Copy)]
pub struct Progress {
    pub size: u64,
    pub start_time: Instant,
    pub uploaded: RwSignal<u64>,
}

#[component]
pub fn ProgressBar(
    #[prop(into)] size: Signal<u64>,
    #[prop(into)] start_time: Signal<Instant>,
    #[prop(into)] uploaded: Signal<u64>,
) -> impl IntoView {
    let percent = move || uploaded() * 100 / size();
    let speed = move || {
        format_bytes(((uploaded() * 1000) as f64 / start_time().elapsed().as_millis_f64()) as u64)
    };

    view! {
      <div class="flex flex-row gap-5 justify-between items-baseline m-2 w-full">
        <span>Uploading {move || format!("{: >3}", percent())}%</span>
        <div class="h-3 rounded-full bg-neutral grow">
          <div
            class="h-full rounded-full transition-all ease-linear bg-info duration-50"
            style:width=move || format!("{: >3}%", percent())
          />
        </div>
        <span class="w-28 text-right">{speed}/s</span>
      </div>
    }
}
