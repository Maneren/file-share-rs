//! Server functions (Leptos `#[server]`) and the pure listing logic.
//!
//! Split from the former `server.rs` god-file:
//! - [`models`] — shared DTOs (`ListQuery`, `ListingPage`, `ServerEntry`, …)
//! - [`listing`] — pure filter/sort/paginate (SSR, tested)
//! - [`list_dir`] — directory-listing RPC
//! - [`new_folder`] — create-folder RPC
//!
//! Browser uploads go to `POST /upload` directly (see the server crate),
//! with native XHR progress — no upload server-fn needed.

pub mod dir;
pub mod listing;
pub mod models;

pub use dir::{List, NewFolder};
pub use models::{Entries, ListQuery, ListingPage, ServerEntry, SortColumn, SortDir};
