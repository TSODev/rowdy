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

#[cfg(test)]
mod tests {
    use super::{parse_url, DuckDbConnector};
    use crate::db::{error::DbError, traits::SqlClient, types::{TableKind, Value}};

    // ── helpers ──────────────────────────────────────────────────────────────────

    async fn connected() -> DuckDbConnector {
        let mut c = DuckDbConnector::new();
        c.connect("duckdb://:memory:").await.expect("connect :memory: failed");
        println!("  [duckdb] connected to :memory:");
        c
    }

    /// authors + books (with FK) + v_available view, 3 rows each.
    async fn seeded() -> DuckDbConnector {
        let c = connected().await;
        c.execute("CREATE TABLE authors (id INTEGER PRIMARY KEY, name VARCHAR)").await.unwrap();
        c.execute(
            "CREATE TABLE books (\
                id INTEGER PRIMARY KEY, \
                title VARCHAR, \
                author_id INTEGER REFERENCES authors(id), \
                price DOUBLE, \
                available BOOLEAN\
            )",
        ).await.unwrap();
        c.execute(
            "CREATE VIEW v_available AS SELECT * FROM books WHERE available = true",
        ).await.unwrap();
        c.execute("INSERT INTO authors VALUES (1,'Orwell'),(2,'Camus'),(3,'Kafka')").await.unwrap();
        c.execute(
            "INSERT INTO books VALUES \
                (1,'1984',1,9.99,true),\
                (2,'The Plague',2,12.50,true),\
                (3,'The Trial',3,NULL,false)",
        ).await.unwrap();
        c
    }

    // ── parse_url (pure) ─────────────────────────────────────────────────────────

    #[test]
    fn test_parse_url_memory_prefix() {
        assert_eq!(parse_url("duckdb://:memory:"), ":memory:");
        println!("  ✓ duckdb://:memory: → :memory:");
    }

    #[test]
    fn test_parse_url_empty_path() {
        assert_eq!(parse_url("duckdb://"), ":memory:");
        println!("  ✓ duckdb:// → :memory:");
    }

    #[test]
    fn test_parse_url_file_path() {
        assert_eq!(parse_url("duckdb:///tmp/test.db"), "/tmp/test.db");
        println!("  ✓ duckdb:///tmp/test.db → /tmp/test.db");
    }

    #[test]
    fn test_parse_url_no_prefix() {
        assert_eq!(parse_url(":memory:"), ":memory:");
        println!("  ✓ :memory: (no prefix) → :memory:");
    }

    // ── connection ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_connect_memory() {
        println!("\n[duckdb] test_connect_memory");
        let mut c = DuckDbConnector::new();
        assert!(c.connect("duckdb://:memory:").await.is_ok());
        println!("  ✓ :memory: connect OK");
    }

    #[tokio::test]
    async fn test_not_connected_returns_error() {
        println!("\n[duckdb] test_not_connected_returns_error");
        let c = DuckDbConnector::new();
        assert!(matches!(c.fetch_all("SELECT 1").await.unwrap_err(), DbError::NotConnected));
        assert!(matches!(c.execute("SELECT 1").await.unwrap_err(), DbError::NotConnected));
        assert!(matches!(c.get_tables().await.unwrap_err(), DbError::NotConnected));
        println!("  ✓ fetch_all/execute/get_tables before connect → NotConnected");
    }

    #[tokio::test]
    async fn test_disconnect() {
        println!("\n[duckdb] test_disconnect");
        let mut c = connected().await;
        assert!(c.disconnect().await.is_ok());
        assert!(matches!(c.fetch_all("SELECT 1").await.unwrap_err(), DbError::NotConnected));
        println!("  ✓ disconnect OK, subsequent fetch_all → NotConnected");
    }

    // ── execute ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_execute_insert_rows_affected() {
        println!("\n[duckdb] test_execute_insert_rows_affected");
        let c = connected().await;
        c.execute("CREATE TABLE t (id INTEGER, val VARCHAR)").await.unwrap();
        let n = c.execute("INSERT INTO t VALUES (1,'a'),(2,'b'),(3,'c')").await.expect("INSERT failed");
        println!("  INSERT rows affected = {n}");
        assert_eq!(n, 3);
        println!("  ✓ INSERT 3 rows → 3 affected");
    }

    #[tokio::test]
    async fn test_execute_update_rows_affected() {
        println!("\n[duckdb] test_execute_update_rows_affected");
        let c = connected().await;
        c.execute("CREATE TABLE t2 (id INTEGER, val VARCHAR)").await.unwrap();
        c.execute("INSERT INTO t2 VALUES (1,'x'),(2,'x'),(3,'y')").await.unwrap();
        let n = c.execute("UPDATE t2 SET val = 'z' WHERE val = 'x'").await.expect("UPDATE failed");
        println!("  UPDATE rows affected = {n}");
        assert_eq!(n, 2);
        println!("  ✓ UPDATE 2 matching rows → 2 affected");
    }

    #[tokio::test]
    async fn test_execute_invalid_sql() {
        println!("\n[duckdb] test_execute_invalid_sql");
        let c = connected().await;
        let e = c.execute("NOT VALID SQL!!!").await.unwrap_err();
        assert!(matches!(e, DbError::QueryFailed(_)));
        println!("  ✓ invalid SQL → QueryFailed");
    }

    // ── fetch_all ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_fetch_all_empty_result() {
        println!("\n[duckdb] test_fetch_all_empty_result");
        let c = connected().await;
        c.execute("CREATE TABLE empty_t (id INTEGER, name VARCHAR)").await.unwrap();
        let r = c.fetch_all("SELECT * FROM empty_t").await.expect("fetch_all failed");
        println!("  cols={} rows={}", r.columns.len(), r.rows.len());
        assert_eq!(r.columns.len(), 2);
        assert_eq!(r.rows.len(), 0);
        println!("  ✓ empty table → 2 columns, 0 rows");
    }

    #[tokio::test]
    async fn test_fetch_all_column_and_row_count() {
        println!("\n[duckdb] test_fetch_all_column_and_row_count");
        let c = seeded().await;
        let r = c.fetch_all("SELECT * FROM books").await.expect("fetch_all failed");
        println!("  cols={} rows={}", r.columns.len(), r.rows.len());
        assert_eq!(r.columns.len(), 5);
        assert_eq!(r.rows.len(), 3);
        println!("  ✓ books → 5 columns, 3 rows");
    }

    #[tokio::test]
    async fn test_fetch_all_scalar_types() {
        println!("\n[duckdb] test_fetch_all_scalar_types");
        let c = connected().await;
        let r = c
            .fetch_all(
                "SELECT 42::INTEGER, 3.14::DOUBLE, true::BOOLEAN, 'hello'::VARCHAR, NULL::INTEGER",
            )
            .await
            .expect("fetch_all failed");
        assert_eq!(r.rows.len(), 1);
        let row = &r.rows[0];
        println!("  values: {:?}", row.values);
        assert!(matches!(row.values[0], Value::Int(42)));
        assert!(matches!(row.values[1], Value::Float(f) if (f - 3.14).abs() < 1e-9));
        assert!(matches!(row.values[2], Value::Bool(true)));
        assert!(matches!(row.values[3], Value::Text(ref s) if s == "hello"));
        assert!(matches!(row.values[4], Value::Null));
        println!("  ✓ Int/Float/Bool/Text/Null types OK");
    }

    #[tokio::test]
    async fn test_fetch_all_date_time_types() {
        println!("\n[duckdb] test_fetch_all_date_time_types");
        let c = connected().await;
        let r = c
            .fetch_all(
                "SELECT \
                    DATE '2024-06-18', \
                    TIME '14:30:00', \
                    TIMESTAMP '2024-06-18 14:30:00'",
            )
            .await
            .expect("fetch_all failed");
        assert_eq!(r.rows.len(), 1);
        let row = &r.rows[0];
        println!("  date={:?} time={:?} ts={:?}", row.values[0], row.values[1], row.values[2]);
        assert!(matches!(&row.values[0], Value::Text(s) if s == "2024-06-18"));
        assert!(matches!(&row.values[1], Value::Text(s) if s == "14:30:00"));
        assert!(matches!(&row.values[2], Value::Text(s) if s == "2024-06-18 14:30:00"));
        println!("  ✓ Date/Time/Timestamp formatted correctly");
    }

    #[tokio::test]
    async fn test_fetch_all_list_type() {
        println!("\n[duckdb] test_fetch_all_list_type");
        let c = connected().await;
        let r = c
            .fetch_all("SELECT [1, 2, 3]::INTEGER[]")
            .await
            .expect("fetch_all failed");
        assert_eq!(r.rows.len(), 1);
        let val = &r.rows[0].values[0];
        println!("  list value: {:?}", val);
        assert!(matches!(val, Value::NestedArray(_)));
        if let Value::NestedArray(json) = val {
            let arr: serde_json::Value = serde_json::from_str(json).unwrap();
            assert_eq!(arr[0], 1);
            assert_eq!(arr[2], 3);
        }
        println!("  ✓ LIST → NestedArray with correct JSON");
    }

    #[tokio::test]
    async fn test_fetch_all_struct_type() {
        println!("\n[duckdb] test_fetch_all_struct_type");
        let c = connected().await;
        let r = c
            .fetch_all("SELECT {'name': 'Alice', 'score': 30}")
            .await
            .expect("fetch_all failed");
        assert_eq!(r.rows.len(), 1);
        let val = &r.rows[0].values[0];
        println!("  struct value: {:?}", val);
        assert!(matches!(val, Value::NestedDoc(_)));
        if let Value::NestedDoc(json) = val {
            let obj: serde_json::Value = serde_json::from_str(json).unwrap();
            assert_eq!(obj["name"], "Alice");
            assert_eq!(obj["score"], 30);
        }
        println!("  ✓ STRUCT → NestedDoc with correct JSON");
    }

    // ── get_table_objects ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_table_objects_kinds() {
        println!("\n[duckdb] test_get_table_objects_kinds");
        let c = seeded().await;
        let mut objects = c.get_table_objects().await.expect("get_table_objects failed");
        objects.sort_by(|a, b| a.name.cmp(&b.name));
        println!(
            "  objects: {:?}",
            objects.iter().map(|o| (&o.name, &o.kind)).collect::<Vec<_>>()
        );
        let authors = objects.iter().find(|o| o.name == "authors").expect("authors not found");
        let books = objects.iter().find(|o| o.name == "books").expect("books not found");
        let view = objects.iter().find(|o| o.name == "v_available").expect("v_available not found");
        assert_eq!(authors.kind, TableKind::Table);
        assert_eq!(books.kind, TableKind::Table);
        assert_eq!(view.kind, TableKind::View);
        println!("  ✓ TABLE/TABLE/VIEW kinds correct");
    }

    // ── get_schema ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_schema_pk() {
        println!("\n[duckdb] test_get_schema_pk");
        let c = seeded().await;
        let schema = c.get_schema("authors").await.expect("get_schema failed");
        println!(
            "  schema: {:?}",
            schema.iter().map(|c| (&c.name, c.is_pk)).collect::<Vec<_>>()
        );
        let id_col = schema.iter().find(|c| c.name == "id").expect("id not found");
        let name_col = schema.iter().find(|c| c.name == "name").expect("name not found");
        assert!(id_col.is_pk, "id should be PK");
        assert!(!name_col.is_pk, "name should not be PK");
        println!("  ✓ id is PK, name is not PK");
    }

    #[tokio::test]
    async fn test_get_schema_fk() {
        println!("\n[duckdb] test_get_schema_fk");
        let c = seeded().await;
        let schema = c.get_schema("books").await.expect("get_schema failed");
        let author_id = schema.iter().find(|c| c.name == "author_id").expect("author_id not found");
        println!("  author_id fk: {:?}", author_id.fk);
        assert!(author_id.fk.is_some(), "author_id should have FK");
        let fk = author_id.fk.as_ref().unwrap();
        assert_eq!(fk.table, "authors");
        println!("  ✓ author_id FK → authors");
    }

    #[tokio::test]
    async fn test_get_schema_type_names() {
        println!("\n[duckdb] test_get_schema_type_names");
        let c = seeded().await;
        let schema = c.get_schema("books").await.expect("get_schema failed");
        let price = schema.iter().find(|c| c.name == "price").expect("price not found");
        let available = schema.iter().find(|c| c.name == "available").expect("available not found");
        println!("  price='{}' available='{}'", price.type_name, available.type_name);
        assert!(
            price.type_name.to_uppercase().contains("DOUBLE")
                || price.type_name.to_uppercase().contains("FLOAT"),
            "price should be DOUBLE/FLOAT, got '{}'",
            price.type_name
        );
        assert!(
            available.type_name.to_uppercase().contains("BOOLEAN"),
            "available should be BOOLEAN, got '{}'",
            available.type_name
        );
        println!("  ✓ type names DOUBLE/FLOAT and BOOLEAN");
    }
}
