//! Axum server state shared across handlers and Leptos routes.
//!
//! Moved out of `file-share-app` — this wires Axum (`FromRef`) and Leptos
//! server options, so it belongs to the server binary, not the shared UI crate.

use std::sync::Arc;

use axum::extract::FromRef;
use file_share_app::AppConfig;
use leptos::prelude::LeptosOptions;

use crate::security::SecurityConfig;

#[derive(FromRef, Clone, Debug)]
pub struct AppState {
    pub app_config: Arc<AppConfig>,
    pub leptos_options: Arc<LeptosOptions>,
    /// Server-only hardening (auth token, caps). Never sent to the client.
    pub security: Arc<SecurityConfig>,
}

impl FromRef<AppState> for LeptosOptions {
    fn from_ref(state: &AppState) -> Self {
        (*state.leptos_options).clone()
    }
}
