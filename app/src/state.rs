use axum::extract::FromRef;
use leptos::prelude::LeptosOptions;

use crate::AppConfig;

#[derive(FromRef, Clone, Debug)]
pub struct AppState {
    pub app_config: AppConfig,
    pub leptos_options: LeptosOptions,
}
