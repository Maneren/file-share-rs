use leptos::*;

use crate::{components::Entry, utils::SystemTime};

#[component]
pub fn Folder<'a>(
  name: &'a str,
  icon: &'a str,
  target: &'a str,
  #[prop(optional)] last_modified: Option<SystemTime>,
) -> impl IntoView {
  view! {
    <a href=format!("?path={target}")>
      <Entry name=name icon=icon last_modified=last_modified/>
    </a>
  }
}
