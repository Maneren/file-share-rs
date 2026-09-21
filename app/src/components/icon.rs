//! Generic SVG icon component.
//!
//! File/folder icon *lookup* lives in
//! [`super::file_entries::icons`]; this is just the renderer.

use leptos::prelude::*;

#[component]
pub fn Icon(icon: &'static str) -> impl IntoView {
    view! { <div class="icon" inner_html=icon /> }
}
