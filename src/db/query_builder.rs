use std::collections::BTreeMap;
use crate::db::types::{ColumnSchema, DbQueryResult, Value};
use crate::ui::screens::data_grid::PAGE_SIZE;

// ── SQL statement helpers ─────────────────────────────────────────────────────

pub fn split_sql_statements(sql: &str) -> Vec<String> {
    // Strip -- comment lines before splitting to avoid charset issues with
    // Unicode characters in comments. Also strip inline trailing comments.
    let cleaned: String = sql
        .lines()
        .map(|line| {
            let t = line.trim_start();
            if t.starts_with("--") {
                return "";
            }
            if let Some(pos) = line.find("--") {
                let before = &line[..pos];
                let in_string = before.chars().filter(|&c| c == '\'').count() % 2 != 0;
                if !in_string { return &line[..pos]; }
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n");

    cleaned
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn is_select_query(sql: &str) -> bool {
    let upper = sql.trim_start().to_uppercase();
    upper.starts_with("SELECT")
        || upper.starts_with("WITH")
        || upper.starts_with("EXPLAIN")
        || upper.starts_with("SHOW")
        || upper.starts_with("DESCRIBE")
        || upper.starts_with("PRAGMA")
}

// ── WHERE / ORDER BY builders ─────────────────────────────────────────────────

pub fn build_where(filters: &BTreeMap<String, String>, schema: &[ColumnSchema]) -> String {
    if filters.is_empty() { return String::new(); }
    let clauses: Vec<String> = filters.iter()
        .map(|(col, val)| {
            let type_name = schema.iter()
                .find(|cs| cs.name == *col)
                .map(|cs| cs.type_name.as_str())
                .unwrap_or("");
            let tn = type_name.to_uppercase();

            if tn.contains("BOOL") || tn == "TINYINT(1)" {
                let b = matches!(
                    val.to_lowercase().as_str(),
                    "true" | "t" | "1" | "yes" | "on"
                );
                format!("\"{}\" = {}", col, if b { "TRUE" } else { "FALSE" })
            } else if (tn.contains("INT") || tn.contains("FLOAT") || tn.contains("REAL")
                || tn.contains("NUMERIC") || tn.contains("DECIMAL") || tn.contains("DOUBLE")
                || tn.contains("NUMBER"))
                && val.parse::<f64>().is_ok()
            {
                format!("\"{}\" = {}", col, val)
            } else {
                let escaped = val.replace('\'', "''");
                format!("\"{}\" LIKE '%{}%'", col, escaped)
            }
        })
        .collect();
    format!(" WHERE {}", clauses.join(" AND "))
}

pub fn build_data_query(
    table: &str,
    filters: &BTreeMap<String, String>,
    offset: usize,
    schema: &[ColumnSchema],
    order_by: Option<(&str, bool)>,
    limit: Option<usize>,
) -> String {
    let wh = build_where(filters, schema);
    let ob = order_by.map_or(String::new(), |(col, asc)| {
        let safe = col.replace('"', "");
        format!(" ORDER BY \"{}\" {}", safe, if asc { "ASC" } else { "DESC" })
    });
    let lim = limit.unwrap_or(PAGE_SIZE);
    format!("SELECT * FROM \"{table}\"{wh}{ob} LIMIT {lim} OFFSET {offset}")
}

pub fn build_count_query(
    table: &str,
    filters: &BTreeMap<String, String>,
    schema: &[ColumnSchema],
) -> String {
    let wh = build_where(filters, schema);
    format!("SELECT COUNT(*) AS _count FROM \"{table}\"{wh}")
}

pub fn build_fk_query(ref_table: &str, ref_col: &str, fk_val: &str) -> String {
    let safe_t = ref_table.replace('"', "");
    let safe_c = ref_col.replace('"', "");
    let safe_v = fk_val.replace('\'', "''");
    format!("SELECT * FROM \"{safe_t}\" WHERE \"{safe_c}\" = '{safe_v}' LIMIT {PAGE_SIZE}")
}

pub fn build_fk_count_query(ref_table: &str, ref_col: &str, fk_val: &str) -> String {
    let safe_t = ref_table.replace('"', "");
    let safe_c = ref_col.replace('"', "");
    let safe_v = fk_val.replace('\'', "''");
    format!("SELECT COUNT(*) AS _count FROM \"{safe_t}\" WHERE \"{safe_c}\" = '{safe_v}'")
}

pub fn parse_count(result: &DbQueryResult) -> u64 {
    result.rows.first()
        .and_then(|r| r.values.first())
        .map(|v| match v {
            Value::Int(n)  => *n as u64,
            Value::Text(s) => s.parse().unwrap_or(0),
            _              => 0,
        })
        .unwrap_or(0)
}
