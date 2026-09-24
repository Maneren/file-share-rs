//! Page components: top-level routes.

mod error;
mod files;
mod login;

pub use error::{AppError, ErrorTemplate};
pub use files::FilesPage;
pub use login::LoginPage;
