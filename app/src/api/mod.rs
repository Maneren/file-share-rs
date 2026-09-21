//! Server functions (Leptos `#[server]`) and the pure listing logic.
//!
//! Split from the former `server.rs` god-file:
//! - [`models`] — shared DTOs (`ListQuery`, `ListingPage`, `ServerEntry`, …)
//! - [`listing`] — pure filter/sort/paginate (SSR, tested)
//! - [`list_dir`] — directory-listing RPC
//! - [`new_folder`] — create-folder RPC
//! - [`upload`] — browser upload RPC
//! - [`upload_progress`] — SSR progress registry + poll RPC

pub mod list_dir;
pub mod listing;
pub mod models;
pub mod new_folder;
pub mod upload;
pub mod upload_progress;

pub use list_dir::{ListDir, list_dir};
pub use models::{Entries, ListQuery, ListingPage, ServerEntry, SortColumn, SortDir};
pub use new_folder::{NewFolder, new_folder};
pub use upload::{UploadFile, upload_file};
pub use upload_progress::file_progress;
#[cfg(feature = "ssr")]
pub use upload_progress::{add_chunk, finish, progress_stream};
