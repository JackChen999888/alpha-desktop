//! Independent personal productivity features. No background transmission of user content.
pub mod agent;
pub mod appearance;
pub mod assistant;
pub mod audio;
pub mod bridge;
pub mod email;
pub mod journal;
pub mod translate;
pub mod ui;

pub fn root() -> std::path::PathBuf {
    crate::model::config_dir().join("pro")
}
pub fn save<T: serde::Serialize>(path: &std::path::Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    crate::model::atomic_write(path, &bytes).map_err(|e| e.to_string())
}
pub fn load<T: serde::de::DeserializeOwned + Default>(path: &std::path::Path) -> T {
    std::fs::read(path)
        .ok()
        .and_then(|v| serde_json::from_slice(&v).ok())
        .unwrap_or_default()
}
