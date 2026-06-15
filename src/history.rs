use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const MAX_ENTRIES: usize = 200;

#[derive(Debug, Deserialize, Serialize, Default)]
struct HistoryFile {
    #[serde(default)]
    entries: Vec<String>,
}

pub struct QueryHistory {
    pub entries: Vec<String>, // most-recent first
    cursor: Option<usize>,    // None = not navigating
}

impl QueryHistory {
    pub fn load() -> Self {
        let entries = history_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| toml::from_str::<HistoryFile>(&s).ok())
            .map(|f| f.entries)
            .unwrap_or_default();
        Self { entries, cursor: None }
    }

    pub fn push(&mut self, query: String) {
        let query = query.trim().to_string();
        if query.is_empty() { return; }
        // Remove duplicate if it's already the most recent
        if self.entries.first().map(|e| e == &query).unwrap_or(false) {
            self.reset_cursor();
            return;
        }
        // Remove any older duplicate to avoid clutter
        self.entries.retain(|e| e != &query);
        self.entries.insert(0, query);
        self.entries.truncate(MAX_ENTRIES);
        self.reset_cursor();
        self.save();
    }

    /// Navigate to older entry. Returns the entry text if available.
    pub fn prev(&mut self) -> Option<&str> {
        if self.entries.is_empty() { return None; }
        let next = self.cursor.map_or(0, |c| (c + 1).min(self.entries.len() - 1));
        self.cursor = Some(next);
        self.entries.get(next).map(|s| s.as_str())
    }

    /// Navigate to newer entry. Returns None when back at the start (empty editor).
    pub fn next(&mut self) -> Option<&str> {
        match self.cursor {
            None | Some(0) => {
                self.cursor = None;
                None
            }
            Some(c) => {
                self.cursor = Some(c - 1);
                self.entries.get(c - 1).map(|s| s.as_str())
            }
        }
    }

    pub fn reset_cursor(&mut self) {
        self.cursor = None;
    }

    fn save(&self) {
        let Some(path) = history_path() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let file = HistoryFile { entries: self.entries.clone() };
        if let Ok(s) = toml::to_string_pretty(&file) {
            let _ = std::fs::write(path, s);
        }
    }
}

fn history_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".config").join("rowdy").join("history.toml"))
}
