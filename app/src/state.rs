use std::path::PathBuf;

use axum::extract::FromRef;
use leptos::prelude::LeptosOptions;

#[derive(FromRef, Clone, Debug)]
pub struct AppState {
  pub target_dir: PathBuf,
  pub leptos_options: LeptosOptions,
}
