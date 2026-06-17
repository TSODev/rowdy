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
        // match by name first (edit existing), then by URL, then append
        let pos = config.connections.iter().position(|p| p.name == profile.name)
            .or_else(|| config.connections.iter().position(|p| p.url == profile.url));
        if let Some(i) = pos {
            config.connections[i] = profile;
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

// ── URL utilities ─────────────────────────────────────────────────────────────

/// Mask `user:password@` and sensitive query parameters in a URL for display.
pub fn redact_url(url: &str) -> String {
    let mut result = url.to_string();

    if let Some(at_pos) = result.find('@') {
        if let Some(scheme_end) = result.find("://") {
            let authority_start = scheme_end + 3;
            if authority_start < at_pos {
                let authority = &result[authority_start..at_pos];
                if let Some(colon_pos) = authority.find(':') {
                    let abs_colon = authority_start + colon_pos;
                    result.replace_range(abs_colon + 1..at_pos, "***");
                }
            }
        }
    }

    let sensitive = ["authtoken", "token", "password", "pwd", "secret", "key", "auth"];
    if let Some(q_pos) = result.find('?') {
        let base = result[..q_pos + 1].to_string();
        let query = result[q_pos + 1..].to_string();
        let masked: Vec<String> = query.split('&').map(|pair| {
            if let Some(eq) = pair.find('=') {
                let k = pair[..eq].to_ascii_lowercase();
                if sensitive.iter().any(|s| k == *s) {
                    return format!("{}=***", &pair[..eq]);
                }
            }
            pair.to_string()
        }).collect();
        result = format!("{}{}", base, masked.join("&"));
    }

    result
}

/// Strip `?readonly=true` from a URL and return `(clean_url, was_readonly)`.
pub fn strip_readonly_param(url: &str) -> (String, bool) {
    let Some(q_pos) = url.find('?') else {
        return (url.to_string(), false);
    };
    let base = &url[..q_pos];
    let query = url[q_pos + 1..].replace('?', "&");
    let mut readonly = false;
    let remaining: Vec<&str> = query.split('&').filter(|pair| {
        if let Some(eq) = pair.find('=') {
            if pair[..eq].to_ascii_lowercase() == "readonly"
                && pair[eq + 1..].to_ascii_lowercase() == "true"
            {
                readonly = true;
                return false;
            }
        }
        true
    }).collect();
    let new_url = if remaining.is_empty() {
        base.to_string()
    } else {
        format!("{}?{}", base, remaining.join("&"))
    };
    (new_url, readonly)
}
