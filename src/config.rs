use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Config {
    #[serde(default)]
    pub connections: Vec<ConnectionProfile>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ConnectionProfile {
    pub name: String,
    #[serde(rename = "type")]
    pub db_type: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_connect: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_disconnect: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = config_path();
        if !path.exists() {
            return Ok(Config::default());
        }
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn delete_profile(url: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = Self::load().unwrap_or_default();
        config.connections.retain(|p| p.url != url);
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, toml::to_string_pretty(&config)?)?;
        Ok(())
    }

    pub fn save_profile(profile: ConnectionProfile) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = Self::load().unwrap_or_default();
        // replace existing entry with same URL, or append
        if let Some(existing) = config.connections.iter_mut().find(|p| p.url == profile.url) {
            *existing = profile;
        } else {
            config.connections.push(profile);
        }
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, toml::to_string_pretty(&config)?)?;
        Ok(())
    }
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".config")
        .join("rowdy")
        .join("config.toml")
}
