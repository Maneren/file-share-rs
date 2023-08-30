use leptos::*;

#[allow(clippy::needless_lifetimes)]
#[component]
pub fn Icon<'a>(icon: &'a str) -> impl IntoView {
  view! { <img class="icon h-10 w-10" src=format!("/icons/{icon}.svg") alt=format!("{icon} icon")/> }
}
