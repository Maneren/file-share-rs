use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub target_dir: PathBuf,
    pub allow_upload: bool,
}
