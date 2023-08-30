use chrono::{DateTime, Local, Utc};
use chrono_humanize::Humanize;
use leptos::*;

use crate::{components::Icon, utils::SystemTime};

#[component]
pub fn Entry<'a>(
  name: &'a str,
  icon: &'a str,
  #[prop(optional)] size: Option<String>,
  last_modified: Option<SystemTime>,
) -> impl IntoView {
  let last_modified = last_modified.map(DateTime::<Utc>::from);

  let modification_time = last_modified.map(|date_time| {
    date_time
      .with_timezone(&Local)
      .format("%Y-%m-%d %H:%M:%S")
      .to_string()
  });
  let relative_time = last_modified.map(|time| time.humanize());

  view! {
    <div class="entry w-full grid grid-cols-entry-mobile md:grid-cols-entry gap-2">
      <Icon icon=icon/>
      <span class="flex items-center overflow-x-hidden">{name.to_string()}</span>
      <span class="flex items-center justify-end">{size}</span>
      <span class="hidden md:flex items-center justify-center">{modification_time.clone()}</span>
      <span class="hidden md:flex items-center">{relative_time.clone()}</span>
    </div>
  }
}
