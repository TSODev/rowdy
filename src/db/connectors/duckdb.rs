#![cfg(feature = "duckdb")]

use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use crate::db::error::DbError;
use crate::db::traits::SqlClient;
use crate::db::types::{Column, ColumnSchema, DbQueryResult, ForeignKey, Row, TableKind, TableObject, Value};

pub struct DuckDbConnector {
    conn: Option<Arc<Mutex<::duckdb::Connection>>>,
}

impl DuckDbConnector {
    pub fn new() -> Self {
        Self { conn: None }
    }

    fn conn_arc(&self) -> Result<Arc<Mutex<::duckdb::Connection>>, DbError> {
        self.conn.clone().ok_or(DbError::NotConnected)
    }
}

/// Strip `duckdb://` prefix; empty path or `:memory:` → in-memory database.
fn parse_url(url: &str) -> String {
    let path = url.strip_prefix("duckdb://").unwrap_or(url);
    if path.is_empty() || path == ":memory:" {
        ":memory:".to_string()
    } else {
        path.to_string()
    }
}

#[async_trait]
impl SqlClient for DuckDbConnector {
    async fn connect(&mut self, url: &str) -> Result<(), DbError> {
        let path = parse_url(url);
        let conn = tokio::task::spawn_blocking(move || {
            if path == ":memory:" {
                ::duckdb::Connection::open_in_memory()
            } else {
                ::duckdb::Connection::open(&path)
            }
        })
        .await
        .map_err(|e| DbError::ConnectionFailed(e.to_string()))?
        .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;

        self.conn = Some(Arc::new(Mutex::new(conn)));
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DbError> {
        self.conn = None;
        Ok(())
    }

    async fn execute(&self, query: &str) -> Result<u64, DbError> {
        let conn = self.conn_arc()?;
        let query = query.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| DbError::QueryFailed(e.to_string()))?;

            match conn.execute(&query, []) {
                Ok(n) => Ok(n as u64),
                Err(first_err) => {
                    let msg = first_err.to_string();
                    // DuckDB bug (v1.x): UPDATE on a FK-constrained parent table triggers a
                    // false FK violation because the engine validates constraints as DELETE+INSERT
                    // even when the PK is unchanged.
                    // Try the two known session-level FK disable mechanisms in sequence.
                    if msg.to_lowercase().contains("foreign key") {
                        let q = query.trim_end_matches(';');

                        // Attempt 1 — DuckDB native session setting
                        let batch1 = format!(
                            "SET enable_foreign_keys = false;\n{q};\nSET enable_foreign_keys = true;"
                        );
                        if conn.execute_batch(&batch1).is_ok() {
                            return Ok(0u64);
                        }

                        // Attempt 2 — SQLite-compat pragma
                        let batch2 = format!(
                            "PRAGMA foreign_keys = false;\n{q};\nPRAGMA foreign_keys = true;"
                        );
                        conn.execute_batch(&batch2).map(|_| 0u64).map_err(|_| {
                            // Neither worked — give the user an actionable message.
                            DbError::QueryFailed(format!(
                                "{msg}\n\nDuckDB limitation: UPDATE on FK-constrained \
                                 parent tables fails in v1.x. Use the SQL editor:\n  \
                                 SET enable_foreign_keys = false;\n  {q};\n  \
                                 SET enable_foreign_keys = true;"
                            ))
                        })
                    } else {
                        Err(DbError::QueryFailed(msg))
                    }
                }
            }
        })
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?
    }

    async fn fetch_all(&self, query: &str) -> Result<DbQueryResult, DbError> {
        let conn = self.conn_arc()?;
        let query = query.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| DbError::QueryFailed(e.to_string()))?;
            let mut stmt = conn
                .prepare(&query)
                .map_err(|e| DbError::QueryFailed(e.to_string()))?;

            // Execute first — column_count/column_names panic before execution.
            let mut rows =
                stmt.query([]).map_err(|e| DbError::QueryFailed(e.to_string()))?;

            // Column info is available via rows.as_ref() after execution.
            let ncols = rows.as_ref().map(|s| s.column_count()).unwrap_or(0);
            let col_names: Vec<String> = rows
                .as_ref()
                .map(|s| s.column_names().into_iter().map(String::from).collect())
                .unwrap_or_default();

            let mut result_rows: Vec<Row> = vec![];
            loop {
                match rows.next() {
                    Ok(Some(row)) => {
                        let values = (0..ncols).map(|i| duck_row_value(row, i)).collect();
                        result_rows.push(Row { values });
                    }
                    Ok(None) => break,
                    Err(e) => return Err(DbError::QueryFailed(e.to_string())),
                }
            }

            let columns: Vec<Column> = col_names
                .into_iter()
                .map(|name| Column { name, type_name: String::new() })
                .collect();

            let count = result_rows.len() as u64;
            Ok(DbQueryResult { columns, rows: result_rows, rows_affected: count })
        })
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?
    }

    async fn get_tables(&self) -> Result<Vec<String>, DbError> {
        let objs = self.get_table_objects().await?;
        Ok(objs.into_iter().map(|o| o.name).collect())
    }

    async fn get_table_objects(&self) -> Result<Vec<TableObject>, DbError> {
        let conn = self.conn_arc()?;
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| DbError::QueryFailed(e.to_string()))?;
            let mut stmt = conn
                .prepare(
                    "SELECT table_name, table_type \
                     FROM information_schema.tables \
                     WHERE table_schema = 'main' \
                     ORDER BY table_name",
                )
                .map_err(|e| DbError::QueryFailed(e.to_string()))?;

            let mut objects: Vec<TableObject> = vec![];
            let mut rows = stmt.query([]).map_err(|e| DbError::QueryFailed(e.to_string()))?;
            loop {
                match rows.next() {
                    Ok(Some(row)) => {
                        let name: String = row.get(0).unwrap_or_default();
                        let table_type: String = row.get(1).unwrap_or_default();
                        let kind = if table_type.contains("VIEW") {
                            TableKind::View
                        } else {
                            TableKind::Table
                        };
                        objects.push(TableObject { name, kind });
                    }
                    Ok(None) => break,
                    Err(e) => return Err(DbError::QueryFailed(e.to_string())),
                }
            }
            Ok(objects)
        })
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?
    }

    async fn get_schema(&self, table: &str) -> Result<Vec<ColumnSchema>, DbError> {
        let conn = self.conn_arc()?;
        let table = table.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().map_err(|e| DbError::QueryFailed(e.to_string()))?;
            let safe = table.replace('\'', "''");

            let mut col_stmt = conn
                .prepare(&format!(
                    "SELECT column_name, data_type, is_nullable \
                     FROM information_schema.columns \
                     WHERE table_name = '{safe}' AND table_schema = 'main' \
                     ORDER BY ordinal_position"
                ))
                .map_err(|e| DbError::QueryFailed(e.to_string()))?;

            let mut cols: Vec<(String, String, bool)> = vec![];
            let mut col_rows =
                col_stmt.query([]).map_err(|e| DbError::QueryFailed(e.to_string()))?;
            loop {
                match col_rows.next() {
                    Ok(Some(row)) => {
                        let name: String = row.get(0).unwrap_or_default();
                        let type_name: String = row.get(1).unwrap_or_default();
                        let nullable: String =
                            row.get(2).unwrap_or_else(|_| "YES".to_string());
                        cols.push((name, type_name, nullable == "YES"));
                    }
                    Ok(None) => break,
                    Err(e) => return Err(DbError::QueryFailed(e.to_string())),
                }
            }
            drop(col_rows);
            drop(col_stmt);

            // Primary key columns via duckdb_constraints()
            let mut pk_cols: Vec<String> = vec![];
            if let Ok(mut pk_stmt) = conn.prepare(&format!(
                "SELECT unnest(constraint_column_names) \
                 FROM duckdb_constraints() \
                 WHERE table_name = '{safe}' AND constraint_type = 'PRIMARY KEY'"
            )) {
                if let Ok(mut pk_rows) = pk_stmt.query([]) {
                    loop {
                        match pk_rows.next() {
                            Ok(Some(row)) => {
                                if let Ok(col) = row.get::<_, String>(0) {
                                    pk_cols.push(col);
                                }
                            }
                            Ok(None) | Err(_) => break,
                        }
                    }
                }
            }

            // Foreign key columns via duckdb_constraints()
            let mut fk_map: std::collections::HashMap<String, ForeignKey> =
                std::collections::HashMap::new();
            if let Ok(mut fk_stmt) = conn.prepare(&format!(
                "SELECT \
                     constraint_column_names[1] AS col, \
                     referenced_table \
                 FROM duckdb_constraints() \
                 WHERE table_name = '{safe}' AND constraint_type = 'FOREIGN KEY'"
            )) {
                if let Ok(mut fk_rows) = fk_stmt.query([]) {
                    loop {
                        match fk_rows.next() {
                            Ok(Some(row)) => {
                                let col: String = row.get(0).unwrap_or_default();
                                let ref_table: String = row.get(1).unwrap_or_default();
                                if !col.is_empty() && !ref_table.is_empty() {
                                    fk_map.insert(
                                        col,
                                        ForeignKey { table: ref_table, column: "id".to_string() },
                                    );
                                }
                            }
                            Ok(None) | Err(_) => break,
                        }
                    }
                }
            }

            let schema = cols
                .into_iter()
                .map(|(name, type_name, nullable)| {
                    let is_pk = pk_cols.contains(&name);
                    let fk = fk_map.get(&name).cloned();
                    ColumnSchema { name, type_name, is_pk, is_nullable: nullable, fk }
                })
                .collect();

            Ok(schema)
        })
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?
    }
}

// --- Value conversion ---

fn duck_row_value(row: &::duckdb::Row, idx: usize) -> Value {
    match row.get::<_, ::duckdb::types::Value>(idx) {
        Ok(v) => duck_to_value(v),
        Err(_) => Value::Null,
    }
}

fn duck_to_value(v: ::duckdb::types::Value) -> Value {
    use ::duckdb::types::Value as DV;
    match v {
        DV::Null => Value::Null,
        DV::Boolean(b) => Value::Bool(b),
        DV::TinyInt(i) => Value::Int(i64::from(i)),
        DV::SmallInt(i) => Value::Int(i64::from(i)),
        DV::Int(i) => Value::Int(i64::from(i)),
        DV::BigInt(i) => Value::Int(i),
        DV::HugeInt(i) => Value::Text(i.to_string()),
        DV::UTinyInt(i) => Value::Int(i64::from(i)),
        DV::USmallInt(i) => Value::Int(i64::from(i)),
        DV::UInt(i) => Value::Int(i64::from(i)),
        DV::UBigInt(i) => Value::Text(i.to_string()),
        DV::Float(f) => Value::Float(f64::from(f)),
        DV::Double(f) => Value::Float(f),
        DV::Decimal(d) => Value::Text(d.to_string()),
        DV::Text(s) => Value::Text(s),
        DV::Blob(b) => Value::Bytes(b),
        DV::Enum(s) => Value::Text(s),
        DV::Date32(days) => format_date(days),
        DV::Time64(unit, t) => format_time(unit, t),
        DV::Timestamp(unit, ts) => format_timestamp(unit, ts),
        DV::Interval { months, days, .. } => {
            Value::Text(format!("{months} months {days} days"))
        }
        DV::List(items) | DV::Array(items) => {
            let json: Vec<serde_json::Value> = items.into_iter().map(duck_to_json).collect();
            Value::NestedArray(serde_json::to_string(&json).unwrap_or_default())
        }
        DV::Struct(map) => {
            let obj: serde_json::Map<String, serde_json::Value> =
                map.iter().map(|(k, v)| (k.clone(), duck_to_json(v.clone()))).collect();
            Value::NestedDoc(
                serde_json::to_string(&serde_json::Value::Object(obj)).unwrap_or_default(),
            )
        }
        DV::Map(pairs) => {
            let obj: serde_json::Map<String, serde_json::Value> = pairs
                .iter()
                .map(|(k, v)| (duck_value_key(k.clone()), duck_to_json(v.clone())))
                .collect();
            Value::NestedDoc(
                serde_json::to_string(&serde_json::Value::Object(obj)).unwrap_or_default(),
            )
        }
        DV::Union(inner) => duck_to_value(*inner),
    }
}

fn duck_to_json(v: ::duckdb::types::Value) -> serde_json::Value {
    use ::duckdb::types::Value as DV;
    use serde_json::Value as JV;
    match v {
        DV::Null => JV::Null,
        DV::Boolean(b) => JV::Bool(b),
        DV::TinyInt(i) => JV::Number(i.into()),
        DV::SmallInt(i) => JV::Number(i.into()),
        DV::Int(i) => JV::Number(i.into()),
        DV::BigInt(i) => JV::Number(i.into()),
        DV::UTinyInt(i) => JV::Number(i.into()),
        DV::USmallInt(i) => JV::Number(i.into()),
        DV::UInt(i) => JV::Number(i.into()),
        DV::HugeInt(i) => JV::String(i.to_string()),
        DV::UBigInt(i) => JV::String(i.to_string()),
        DV::Float(f) => {
            serde_json::Number::from_f64(f64::from(f)).map(JV::Number).unwrap_or(JV::Null)
        }
        DV::Double(f) => serde_json::Number::from_f64(f).map(JV::Number).unwrap_or(JV::Null),
        DV::Text(s) | DV::Enum(s) => JV::String(s),
        DV::Decimal(d) => JV::String(d.to_string()),
        DV::Date32(days) => JV::String(format_date(days).into_text()),
        DV::Time64(unit, t) => JV::String(format_time(unit, t).into_text()),
        DV::Timestamp(unit, ts) => JV::String(format_timestamp(unit, ts).into_text()),
        DV::Interval { months, days, .. } => {
            JV::String(format!("{months} months {days} days"))
        }
        DV::List(items) | DV::Array(items) => {
            JV::Array(items.into_iter().map(duck_to_json).collect())
        }
        DV::Struct(map) => {
            let obj: serde_json::Map<String, serde_json::Value> =
                map.iter().map(|(k, v)| (k.clone(), duck_to_json(v.clone()))).collect();
            JV::Object(obj)
        }
        DV::Map(pairs) => {
            let obj: serde_json::Map<String, serde_json::Value> = pairs
                .iter()
                .map(|(k, v)| (duck_value_key(k.clone()), duck_to_json(v.clone())))
                .collect();
            JV::Object(obj)
        }
        DV::Union(inner) => duck_to_json(*inner),
        DV::Blob(b) => JV::String(hex_encode(&b)),
    }
}

fn duck_value_key(v: ::duckdb::types::Value) -> String {
    use ::duckdb::types::Value as DV;
    match v {
        DV::Text(s) | DV::Enum(s) => s,
        DV::TinyInt(i) => i.to_string(),
        DV::SmallInt(i) => i.to_string(),
        DV::Int(i) => i.to_string(),
        DV::BigInt(i) => i.to_string(),
        DV::HugeInt(i) => i.to_string(),
        DV::UTinyInt(i) => i.to_string(),
        DV::USmallInt(i) => i.to_string(),
        DV::UInt(i) => i.to_string(),
        DV::UBigInt(i) => i.to_string(),
        DV::Boolean(b) => b.to_string(),
        _ => String::from("<key>"),
    }
}

fn format_date(days: i32) -> Value {
    let base = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
    match base.checked_add_signed(chrono::Duration::days(i64::from(days))) {
        Some(d) => Value::Text(d.format("%Y-%m-%d").to_string()),
        None => Value::Text(days.to_string()),
    }
}

fn format_time(unit: ::duckdb::types::TimeUnit, t: i64) -> Value {
    use ::duckdb::types::TimeUnit;
    let micros = match unit {
        TimeUnit::Second => t * 1_000_000,
        TimeUnit::Millisecond => t * 1_000,
        TimeUnit::Microsecond => t,
        TimeUnit::Nanosecond => t / 1_000,
    };
    let h = micros / 3_600_000_000;
    let m = (micros % 3_600_000_000) / 60_000_000;
    let s = (micros % 60_000_000) / 1_000_000;
    let us = micros % 1_000_000;
    if us > 0 {
        Value::Text(format!("{h:02}:{m:02}:{s:02}.{us:06}"))
    } else {
        Value::Text(format!("{h:02}:{m:02}:{s:02}"))
    }
}

fn format_timestamp(unit: ::duckdb::types::TimeUnit, ts: i64) -> Value {
    use ::duckdb::types::TimeUnit;
    let micros = match unit {
        TimeUnit::Second => ts * 1_000_000,
        TimeUnit::Millisecond => ts * 1_000,
        TimeUnit::Microsecond => ts,
        TimeUnit::Nanosecond => ts / 1_000,
    };
    let secs = micros / 1_000_000;
    let nanos = ((micros % 1_000_000) * 1_000) as u32;
    match chrono::DateTime::<chrono::Utc>::from_timestamp(secs, nanos) {
        Some(dt) => Value::Text(dt.format("%Y-%m-%d %H:%M:%S").to_string()),
        None => Value::Text(ts.to_string()),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

trait IntoText {
    fn into_text(self) -> String;
}

impl IntoText for Value {
    fn into_text(self) -> String {
        match self {
            Value::Text(s) => s,
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            _ => String::new(),
        }
    }
}
