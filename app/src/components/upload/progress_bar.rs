use std::collections::VecDeque;

use leptos::prelude::*;
use web_time::Instant;

use crate::utils::format_bytes;

#[derive(Debug, Clone, Copy)]
pub struct Progress {
    pub size: u64,
    pub start_time: Instant,
    pub uploaded: RwSignal<VecDeque<(u64, Instant)>>,
}

#[component]
pub fn ProgressBar(
    #[prop(into)] size: Signal<u64>,
    #[prop(into)] start_time: Signal<Instant>,
    #[prop(into)] uploaded: Signal<VecDeque<(u64, Instant)>>,
) -> impl IntoView {
    let start_time = *start_time.read();

    let percent = move || {
        uploaded.with(|queue| queue.iter().last().map(|(size, _)| *size).unwrap_or(0)) * 100
            / size()
    };
    let average_speed = move || {
        uploaded.with(|queue| {
            queue
                .iter()
                .map(move |(size, time)| (*size as f64) / (*time - start_time).as_secs_f64())
                .sum::<f64>()
                / queue.len() as f64
        })
    };
    let formatted_speed = move || format_bytes(average_speed() as u64);

    view! {
      <div class="flex flex-row gap-5 justify-between items-baseline m-2 w-full">
        <span>Uploading {move || format!("{: >3}", percent())}%</span>
        <div class="h-3 rounded-full bg-neutral grow">
          <div
            class="h-full rounded-full transition-all ease-linear bg-info duration-50"
            style:width=move || format!("{: >3}%", percent())
          />
        </div>
        <span class="w-28 text-right">{formatted_speed}/s</span>
      </div>
    }
}
