use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub download_path: Option<String>,
    pub year: Option<u32>,
    pub concurrency: Option<usize>,
}

impl Config {
    pub fn from_current(download_path: &Path, year: Option<u32>, concurrency: usize) -> Self {
        Self {
            download_path: Some(download_path.to_string_lossy().into_owned()),
            year,
            concurrency: Some(concurrency),
        }
    }
}

pub fn resolve_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix('~') {
        if let Some(home) = dirs::home_dir() {
            let rest = rest.strip_prefix('/').unwrap_or(rest);
            return home.join(rest);
        }
    }
    path.to_path_buf()
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("moocs-collect").join("settings.json"))
}

pub fn load() -> Config {
    let Some(path) = config_path() else {
        return Config::default();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(config: &Config) {
    let Some(path) = config_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = serde_json::to_string_pretty(config).map(|json| std::fs::write(path, json));
}
