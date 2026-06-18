use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[cfg(feature = "secure-storage")]
const KEYRING_SERVICE: &str = "rowdy";
const KEYRING_PLACEHOLDER: &str = "__keyring__";

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

    pub fn delete_profile(name: &str, url: &str) -> Result<(), Box<dyn std::error::Error>> {
        delete_credential(name);
        let mut config = Self::load().unwrap_or_default();
        config.connections.retain(|p| p.url != url);
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, toml::to_string_pretty(&config)?)?;
        Ok(())
    }

    pub fn save_profile(mut profile: ConnectionProfile) -> Result<(), Box<dyn std::error::Error>> {
        let (sanitized_url, _stored) = store_credential(&profile.name, &profile.url);
        profile.url = sanitized_url;
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

// ── Credential storage ────────────────────────────────────────────────────────

/// Extract password from URL authority (`user:pass@host`) and replace with placeholder.
/// Returns `Some((credential, sanitized_url))` or `None` if no password found.
fn extract_url_password(url: &str) -> Option<(String, String)> {
    let scheme_end = url.find("://")?;
    let after_scheme = &url[scheme_end + 3..];
    let at_pos = after_scheme.find('@')?;
    let userinfo = &after_scheme[..at_pos];
    let colon_pos = userinfo.find(':')?;
    let password = &userinfo[colon_pos + 1..];
    if password.is_empty() || password == KEYRING_PLACEHOLDER {
        return None;
    }
    let abs_colon = scheme_end + 3 + colon_pos;
    let abs_at = scheme_end + 3 + at_pos;
    let mut sanitized = url.to_string();
    sanitized.replace_range(abs_colon + 1..abs_at, KEYRING_PLACEHOLDER);
    Some((password.to_string(), sanitized))
}

/// Extract sensitive query parameter (authToken, token, …) and replace with placeholder.
/// Returns `Some((credential, sanitized_url))` or `None` if nothing to extract.
fn extract_query_token(url: &str) -> Option<(String, String)> {
    let sensitive = ["authtoken", "token", "password", "pwd", "secret", "key", "auth"];
    let q_pos = url.find('?')?;
    let base = &url[..q_pos];
    let query = &url[q_pos + 1..];
    let mut credential: Option<String> = None;
    let sanitized_params: Vec<String> = query.split('&').map(|pair| {
        if credential.is_none() {
            if let Some(eq) = pair.find('=') {
                let k = pair[..eq].to_ascii_lowercase();
                let v = &pair[eq + 1..];
                if sensitive.iter().any(|s| k == *s) && v != KEYRING_PLACEHOLDER {
                    credential = Some(v.to_string());
                    return format!("{}={}", &pair[..eq], KEYRING_PLACEHOLDER);
                }
            }
        }
        pair.to_string()
    }).collect();
    let cred = credential?;
    Some((cred, format!("{}?{}", base, sanitized_params.join("&"))))
}

/// Store credentials from `url` in the OS keyring under `profile_name`.
/// Returns `(sanitized_url, keyring_ok)`.
/// If keyring is unavailable or fails, returns the original URL unchanged and `false`.
pub fn store_credential(profile_name: &str, url: &str) -> (String, bool) {
    let extraction = extract_url_password(url).or_else(|| extract_query_token(url));
    let Some((credential, sanitized)) = extraction else {
        return (url.to_string(), true); // nothing to store
    };
    store_in_keyring(profile_name, &credential, url, &sanitized)
}

#[cfg(feature = "secure-storage")]
fn store_in_keyring(profile_name: &str, credential: &str, url: &str, sanitized: &str) -> (String, bool) {
    let result = keyring::Entry::new(KEYRING_SERVICE, profile_name)
        .and_then(|e| {
            e.set_password(credential)?;
            // Verify the write is actually readable before trusting it
            e.get_password()?;
            Ok(())
        });
    match result {
        Ok(_) => (sanitized.to_string(), true),
        Err(_) => (url.to_string(), false),
    }
}

#[cfg(not(feature = "secure-storage"))]
fn store_in_keyring(_profile_name: &str, _credential: &str, url: &str, _sanitized: &str) -> (String, bool) {
    (url.to_string(), false)
}

/// Replace `__keyring__` placeholder in `url` with the credential stored for `profile_name`.
/// Returns `Ok(resolved_url)` or `Err(message)` if the keyring lookup fails.
pub fn resolve_credential(profile_name: &str, url: &str) -> Result<String, String> {
    if !url.contains(KEYRING_PLACEHOLDER) {
        return Ok(url.to_string());
    }
    #[cfg(feature = "secure-storage")]
    {
        keyring::Entry::new(KEYRING_SERVICE, profile_name)
            .and_then(|e| e.get_password())
            .map(|cred| url.replacen(KEYRING_PLACEHOLDER, &cred, 1))
            .map_err(|e| format!("Keyring error for '{}': {}", profile_name, e))
    }
    #[cfg(not(feature = "secure-storage"))]
    Err(format!("secure-storage feature not enabled (profile '{}')", profile_name))
}

/// Remove the keyring entry for `profile_name` (called on profile deletion).
pub fn delete_credential(profile_name: &str) {
    #[cfg(feature = "secure-storage")]
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, profile_name) {
        let _ = entry.delete_credential();
    }
    #[cfg(not(feature = "secure-storage"))]
    let _ = profile_name;
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
