use std::path::PathBuf;

use leptos::*;

use crate::app::utils::os_to_string;

#[component]
pub fn Breadcrumbs(path: Signal<PathBuf>) -> impl IntoView {
  let breadcrumbs = move || {
    path.with(|path| {
      path
        .iter()
        .scan(PathBuf::new(), |path, part| {
          path.push(part);
          let path = format!("/index/{}", path.display());

          Some(view! {
            <li>
              <a href=path>{os_to_string(part)}</a>
            </li>
          })
        })
        .collect_view()
    })
  };

  let home_icon = view! {
    <a href="/index">
      <svg class="h-6 w-6 fill-current" viewBox="0 0 24 24">
        <path d="M12,3L20,9V21H15V14H9V21H4V9L12,3Z" />
      </svg>
    </a>
  };

  view! {
    <div class="text-lg breadcrumbs max-w-full">
      <ul class="h-8">
        <li>{home_icon}</li>
        {breadcrumbs}
      </ul>
    </div>
  }
}
