//! File/archive/upload HTTP handlers.
//!
//! Split from the former `fileserv.rs` god-file:
//! - [`static_files`] — embedded frontend + Leptos fallback
//! - [`archive_handler`] — `GET /archive` streaming
//! - [`archive_io`] — tar/zip writers
//! - [`upload`] — `POST /upload` (curl API)
//! - [`gate`] — symlink-containment middleware
//! - [`responses`] — shared error responses

pub mod archive_handler;
pub mod archive_io;
pub mod gate;
pub mod responses;
pub mod static_files;
pub mod upload;

pub use archive_handler::{
    ArchiveQuery, Method, handle_archive_with_path, handle_archive_without_path,
};
pub use gate::gate_shared_files;
pub use static_files::file_and_error_handler;
pub use upload::{file_upload, file_upload_with_path, file_upload_without_path};
