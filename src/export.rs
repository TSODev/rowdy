use std::path::PathBuf;
use crate::db::types::{DbQueryResult, Value};

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

pub fn export_json(result: &DbQueryResult, table_name: &str) -> Result<PathBuf, std::io::Error> {
    let path = export_path(table_name, "json");
    let mut out = String::from("[\n");

    for (i, row) in result.rows.iter().enumerate() {
        out.push_str("  {");
        let fields: Vec<String> = result.columns.iter().zip(row.values.iter())
            .map(|(col, val)| format!("\"{}\": {}", json_escape(&col.name), value_json(val)))
            .collect();
        out.push_str(&fields.join(", "));
        out.push('}');
        if i < result.rows.len() - 1 { out.push(','); }
        out.push('\n');
    }
    out.push(']');

    write_file(&path, out)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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

fn value_str(v: &Value) -> String {
    match v {
        Value::Null      => String::new(),
        Value::Bool(b)   => b.to_string(),
        Value::Int(i)    => i.to_string(),
        Value::Float(f)  => f.to_string(),
        Value::Text(s)   => s.clone(),
        Value::Bytes(b)  => b.iter().map(|x| format!("{x:02x}")).collect(),
    }
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn value_json(v: &Value) -> String {
    match v {
        Value::Null      => "null".to_string(),
        Value::Bool(b)   => b.to_string(),
        Value::Int(i)    => i.to_string(),
        Value::Float(f)  => f.to_string(),
        Value::Text(s)   => format!("\"{}\"", json_escape(s)),
        Value::Bytes(b)  => {
            let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
            format!("\"{hex}\"")
        }
    }
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
     .replace('"',  "\\\"")
     .replace('\n', "\\n")
     .replace('\r', "\\r")
     .replace('\t', "\\t")
}
