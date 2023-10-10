use std::{iter, path::PathBuf};

use leptos::*;

use crate::pages::files::utils::os_to_string;

#[component]
pub fn Breadcrumbs(path: Signal<PathBuf>) -> impl IntoView {
  let breadcrumbs = move || {
    path.with(|path| {
      let leading_breadcrumb = ("/".into(), "/index".into());

      let path_breadcrumbs = path.iter().scan(PathBuf::new(), |path, part| {
        let part_str = os_to_string(part);
        path.push(part);
        let path = format!("/index/{}", path.display());

        Some((part_str, path))
      });

      iter::once(leading_breadcrumb)
        .chain(path_breadcrumbs)
        .map(|(name, path)| {
          view! {
            <li>
              <a href=path>{name}</a>
            </li>
          }
        })
        .collect_view()
    })
  };

  view! {
    <div class="text-lg breadcrumbs max-w-full">
      <ul>{breadcrumbs}</ul>
    </div>
  }
}
