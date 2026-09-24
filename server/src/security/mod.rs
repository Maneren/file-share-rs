//! Server-only hardening, opt-in via CLI flags.
//!
//! Unlike [`AppConfig`](file_share_app::AppConfig), nothing here ever reaches
//! the client — the token in particular stays server-side.

mod auth;
mod config;

pub use auth::{login, rate_limit, require_auth};
pub use config::SecurityConfig;
