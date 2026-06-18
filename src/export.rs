use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::pin::Pin;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::db::traits::SqlClient;
use crate::db::types::{Column, ColumnSchema, DbQueryResult, Value};

// ── Public API ────────────────────────────────────────────────────────────────

pub fn export_csv(result: &DbQueryResult, table_name: &str) -> Result<PathBuf, std::io::Error> {
    let path = export_path(table_name, "csv");
    let mut out = String::new();

    let headers: Vec<String> = result.columns.iter().map(|c| csv_field(&c.name)).collect();
    out.push_str(&headers.join(","));
    out.push('\n');

    for row in &result.rows {
        let fields: Vec<String> = row.values.iter().map(|v| csv_field(&value_str(v))).collect();
        out.push_str(&fields.join(","));
        out.push('\n');
    }

    write_file(&path, out)
}

/// Simple JSON export (no FK resolution) — used as fallback when no SQL client available.
pub fn export_json(result: &DbQueryResult, table_name: &str) -> Result<PathBuf, std::io::Error> {
    let path = export_path(table_name, "json");
    let mut rows: Vec<serde_json::Value> = Vec::with_capacity(result.rows.len());
    for row in &result.rows {
        let mut obj = serde_json::Map::new();
        for (col, val) in result.columns.iter().zip(row.values.iter()) {
            obj.insert(col.name.clone(), value_to_jsvalue(val));
        }
        rows.push(serde_json::Value::Object(obj));
    }
    let out = serde_json::to_string_pretty(&serde_json::Value::Array(rows))
        .map_err(std::io::Error::other)?;
    write_file(&path, out)
}

/// JSON export with recursive FK resolution.
/// For each FK column, fetches the referenced row and embeds it as `<col>__ref`.
/// Resolves FK chains up to `max_depth` levels (default: 3). Detects cycles.
pub async fn export_json_with_fk(
    client: Arc<dyn SqlClient>,
    result: &DbQueryResult,
    table_name: &str,
    schema: &[ColumnSchema],
    max_depth: u8,
) -> Result<PathBuf, std::io::Error> {
    let path = export_path(table_name, "json");
    let cache: Arc<Mutex<HashMap<String, Vec<ColumnSchema>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    cache.lock().await.insert(table_name.to_string(), schema.to_vec());

    let mut rows: Vec<serde_json::Value> = Vec::with_capacity(result.rows.len());
    for row in &result.rows {
        let obj = resolve_row(
            Arc::clone(&client),
            result.columns.clone(),
            row.values.clone(),
            schema.to_vec(),
            Arc::clone(&cache),
            HashSet::new(),
            max_depth,
        )
        .await;
        rows.push(serde_json::Value::Object(obj));
    }

    let out = serde_json::to_string_pretty(&serde_json::Value::Array(rows))
        .map_err(std::io::Error::other)?;
    write_file(&path, out)
}

// ── Recursive FK resolver ─────────────────────────────────────────────────────

type SchemaCache = Arc<Mutex<HashMap<String, Vec<ColumnSchema>>>>;

/// Builds a JSON object for one row, embedding `<col>__ref` objects for FK columns.
/// Uses `Box::pin` to allow async recursion in Rust.
fn resolve_row(
    client: Arc<dyn SqlClient>,
    columns: Vec<Column>,
    values: Vec<Value>,
    schema: Vec<ColumnSchema>,
    cache: SchemaCache,
    visited: HashSet<String>,
    depth: u8,
) -> Pin<Box<dyn Future<Output = serde_json::Map<String, serde_json::Value>> + Send>> {
    Box::pin(async move {
        let mut obj = serde_json::Map::new();

        for (col, val) in columns.iter().zip(values.iter()) {
            obj.insert(col.name.clone(), value_to_jsvalue(val));

            if depth == 0 || matches!(val, Value::Null) {
                continue;
            }

            let fk = match schema.iter().find(|cs| cs.name == col.name).and_then(|cs| cs.fk.as_ref()) {
                Some(f) => f.clone(),
                None => continue,
            };

            let fk_val_str = value_str(val);
            let visit_key = format!("{}.{}={}", fk.table, fk.column, fk_val_str);
            if visited.contains(&visit_key) {
                continue; // cycle — stop here
            }

            // Get schema for the referenced table (cache to avoid duplicate queries)
            let ref_schema = {
                let cached = cache.lock().await.get(&fk.table).cloned();
                if let Some(s) = cached {
                    s
                } else {
                    match client.get_schema(&fk.table).await {
                        Ok(s) => {
                            cache.lock().await.insert(fk.table.clone(), s.clone());
                            s
                        }
                        Err(_) => continue,
                    }
                }
            };

            // Fetch the single referenced row
            let safe_t = fk.table.replace('"', "");
            let safe_c = fk.column.replace('"', "");
            let safe_v = fk_val_str.replace('\'', "''");
            let q = format!("SELECT * FROM \"{safe_t}\" WHERE \"{safe_c}\" = '{safe_v}' LIMIT 1");

            let ref_result = match client.fetch_all(&q).await {
                Ok(r) => r,
                Err(_) => continue,
            };
            let ref_row = match ref_result.rows.first() {
                Some(r) => r,
                None => continue,
            };

            let mut new_visited = visited.clone();
            new_visited.insert(visit_key);

            let ref_obj = resolve_row(
                Arc::clone(&client),
                ref_result.columns.clone(),
                ref_row.values.clone(),
                ref_schema,
                Arc::clone(&cache),
                new_visited,
                depth - 1,
            )
            .await;

            obj.insert(format!("{}__ref", col.name), serde_json::Value::Object(ref_obj));
        }

        obj
    })
}

// ── Value converters ──────────────────────────────────────────────────────────

fn value_to_jsvalue(v: &Value) -> serde_json::Value {
    match v {
        Value::Null    => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(i)  => serde_json::Value::Number((*i).into()),
        Value::Float(f) => {
            serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        Value::Text(s) => {
            // Inline already-serialized JSON blobs (JSONB columns, etc.)
            if (s.starts_with('{') || s.starts_with('[')) && s.len() < 65536
                && let Ok(v) = serde_json::from_str(s) {
                    return v;
                }
            serde_json::Value::String(s.clone())
        }
        Value::Bytes(b) => {
            let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
            serde_json::Value::String(hex)
        }
        Value::NestedDoc(s) | Value::NestedArray(s) => {
            serde_json::from_str(s).unwrap_or(serde_json::Value::String(s.clone()))
        }
    }
}

fn value_str(v: &Value) -> String {
    match v {
        Value::Null                              => String::new(),
        Value::Bool(b)                           => b.to_string(),
        Value::Int(i)                            => i.to_string(),
        Value::Float(f)                          => f.to_string(),
        Value::Text(s)                           => s.clone(),
        Value::Bytes(b)                          => b.iter().map(|x| format!("{x:02x}")).collect(),
        Value::NestedDoc(s) | Value::NestedArray(s) => s.clone(),
    }
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ── File helpers ──────────────────────────────────────────────────────────────

fn write_file(path: &PathBuf, content: String) -> Result<PathBuf, std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(path.clone())
}

fn export_path(table_name: &str, ext: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let safe: String = table_name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    PathBuf::from(home).join(format!("rowdy_{safe}_{ts}.{ext}"))
}
