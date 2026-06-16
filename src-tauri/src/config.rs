use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub pexels_api_key: Option<String>,
}

pub fn load(dir: &Path) -> AppResult<AppConfig> {
    let path = dir.join("config.json");
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let text = std::fs::read_to_string(&path)?;
    serde_json::from_str(&text).map_err(|e| AppError::Other(format!("parse config: {e}")))
}

pub fn save(dir: &Path, cfg: &AppConfig) -> AppResult<()> {
    std::fs::create_dir_all(dir)?;
    let text =
        serde_json::to_string_pretty(cfg).map_err(|e| AppError::Other(format!("write config: {e}")))?;
    std::fs::write(dir.join("config.json"), text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("deskhub_cfg_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn load_missing_returns_default() {
        let dir = temp_dir();
        let cfg = load(&dir).unwrap();
        assert!(cfg.pexels_api_key.is_none());
    }

    #[test]
    fn save_then_load_roundtrips_key() {
        let dir = temp_dir();
        let cfg = AppConfig {
            pexels_api_key: Some("abc123".into()),
        };
        save(&dir, &cfg).unwrap();
        let loaded = load(&dir).unwrap();
        assert_eq!(loaded.pexels_api_key.as_deref(), Some("abc123"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
