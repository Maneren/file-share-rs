#![allow(non_snake_case)]

//! Shared Leptos app: routing shell, pages, components and server functions.
//!
//! Module map:
//! - [`app`] — root `App` component + HTML `shell`
//! - [`pages`] — `FilesPage`, error template
//! - [`components`] — UI islands/components
//! - [`api`] — server functions + listing models/logic
//! - [`config`] — shared `AppConfig` + client messages
//! - [`archive`] — shared archive `Method`
//! - [`time`], [`paths`], [`format`], [`pagination`], [`fs_guard`] — focused
//!   helpers
//! - [`utils`] — backwards-compat re-export shim

pub mod api;
pub mod app;
pub mod archive;
mod components;
mod config;
pub mod format;
pub mod fs_guard;
pub mod messages;
pub mod pagination;
pub mod paths;
pub mod time;
pub mod utils;

pub mod pages;

pub use crate::{
    app::{App, shell},
    config::{
        AppConfig, PATH_NOT_FOUND_MESSAGE, UPLOAD_DISABLED_MESSAGE, UPLOAD_READ_ERROR_MESSAGE,
        UPLOAD_STORE_ERROR_MESSAGE,
    },
    pages::{AppError, ErrorTemplate, FilesPage, LoginPage},
};
