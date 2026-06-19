use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Snippet {
    pub name: String,
    pub sql: String,
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct SnippetFile {
    #[serde(default)]
    snippets: Vec<Snippet>,
}

pub struct SnippetStore {
    pub snippets: Vec<Snippet>,
}

impl SnippetStore {
    pub fn load() -> Self {
        let snippets = snippets_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| toml::from_str::<SnippetFile>(&s).ok())
            .map(|f| f.snippets)
            .unwrap_or_default();
        Self { snippets }
    }

    pub fn add(&mut self, name: String, sql: String) {
        // Replace if same name already exists
        if let Some(existing) = self.snippets.iter_mut().find(|s| s.name == name) {
            existing.sql = sql;
        } else {
            self.snippets.push(Snippet { name, sql });
        }
        self.save();
    }

    pub fn delete(&mut self, idx: usize) {
        if idx < self.snippets.len() {
            self.snippets.remove(idx);
            self.save();
        }
    }

    fn save(&self) {
        let Some(path) = snippets_path() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let file = SnippetFile { snippets: self.snippets.clone() };
        if let Ok(s) = toml::to_string_pretty(&file) {
            let _ = std::fs::write(path, s);
        }
    }
}

fn snippets_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".config").join("rowdy").join("snippets.toml"))
}
