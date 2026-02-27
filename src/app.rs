use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub repo_path: PathBuf,
    pub bundle_path: PathBuf,
    pub base_ref: String,
    pub tip_ref: Option<String>,
}
