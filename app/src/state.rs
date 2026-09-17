use std::sync::Arc;

use axum::extract::FromRef;
use leptos::prelude::LeptosOptions;

use crate::AppConfig;

#[derive(FromRef, Clone, Debug)]
pub struct AppState {
    pub app_config: Arc<AppConfig>,
    pub leptos_options: Arc<LeptosOptions>,
}

impl FromRef<AppState> for LeptosOptions {
    fn from_ref(state: &AppState) -> Self {
        (*state.leptos_options).clone()
    }
}
