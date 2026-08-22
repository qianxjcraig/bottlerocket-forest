//! Shared forest configuration lookup for command handlers.

use crate::domain::ForestConfig;
use std::path::{Path, PathBuf};

/// Loads a forest configuration from an explicit path.
pub fn load_config(path: &Path) -> miette::Result<ForestConfig> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| miette::miette!("Failed to read {}: {}", path.display(), e))?;
    toml::from_str(&content)
        .map_err(|e| miette::miette!("Failed to parse {}: {}", path.display(), e))
}

/// Finds `forester.toml` by walking up from the current directory.
pub fn find_config() -> miette::Result<(PathBuf, ForestConfig)> {
    let cwd = std::env::current_dir()
        .map_err(|e| miette::miette!("Failed to get current directory: {}", e))?;
    let mut dir = cwd;
    loop {
        let config_path = dir.join("forester.toml");
        if config_path.exists() {
            let config = load_config(&config_path)?;
            return Ok((dir, config));
        }
        if !dir.pop() {
            return Err(miette::miette!("No forester.toml found"));
        }
    }
}

/// Resolves a forest from an optional explicit config path.
pub fn resolve_forest(config: Option<PathBuf>) -> miette::Result<(PathBuf, ForestConfig)> {
    match config {
        Some(path) => {
            let cfg = load_config(&path)?;
            let root = path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();
            Ok((root, cfg))
        }
        None => find_config(),
    }
}
