use async_trait::async_trait;
use std::collections::HashMap;
use crate::db::error::DbError;
use crate::db::traits::SqlClient;
use crate::db::types::{Column, ColumnSchema, DbQueryResult, ForeignKey, Row, TableKind, TableObject, Value};

pub struct TursoClient {
    db:   Option<libsql::Database>,
    conn: Option<libsql::Connection>,
}

impl TursoClient {
    pub fn new() -> Self {
        Self { db: None, conn: None }
    }

    fn conn(&self) -> Result<&libsql::Connection, DbError> {
        self.conn.as_ref().ok_or(DbError::NotConnected)
    }
}

#[async_trait]
impl SqlClient for TursoClient {
    async fn connect(&mut self, url: &str) -> Result<(), DbError> {
        let (base_url, token) = parse_url(url)?;
        let db = libsql::Builder::new_remote(base_url, token)
            .build()
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
        let conn = db.connect()
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
        self.db   = Some(db);
        self.conn = Some(conn);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DbError> {
        self.conn = None;
        self.db   = None;
        Ok(())
    }

    async fn execute(&self, query: &str) -> Result<u64, DbError> {
        self.conn()?
            .execute(query, ())
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))
    }

    async fn fetch_all(&self, query: &str) -> Result<DbQueryResult, DbError> {
        let mut rows = self.conn()?
            .query(query, ())
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let col_count = rows.column_count(); // i32

        let columns: Vec<Column> = (0..col_count)
            .map(|i| Column {
                name:      rows.column_name(i).unwrap_or("").to_string(),
                type_name: String::new(),
            })
            .collect();

        let mut result_rows: Vec<Row> = vec![];
        while let Some(row) = rows.next().await.map_err(|e| DbError::QueryFailed(e.to_string()))? {
            let values = (0..col_count)
                .map(|i| to_value(row.get_value(i).unwrap_or(libsql::Value::Null)))
                .collect();
            result_rows.push(Row { values });
        }

        let count = result_rows.len() as u64;
        Ok(DbQueryResult { columns, rows: result_rows, rows_affected: count })
    }

    async fn get_tables(&self) -> Result<Vec<String>, DbError> {
        let result = self.fetch_all(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
        ).await?;
        Ok(result.rows.iter()
            .filter_map(|r| {
                if let Some(Value::Text(s)) = r.values.first() { Some(s.clone()) } else { None }
            })
            .collect())
    }

    async fn get_table_objects(&self) -> Result<Vec<TableObject>, DbError> {
        let result = self.fetch_all(
            "SELECT name, type FROM sqlite_master \
             WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' ORDER BY name"
        ).await?;
        Ok(result.rows.iter().filter_map(|r| {
            let name = if let Some(Value::Text(s)) = r.values.first() { s.clone() } else { return None; };
            let type_str = r.values.get(1).and_then(|v| if let Value::Text(s) = v { Some(s.as_str()) } else { None }).unwrap_or("table");
            let kind = if type_str == "view" { TableKind::View } else { TableKind::Table };
            Some(TableObject { name, kind })
        }).collect())
    }

    async fn get_schema(&self, table: &str) -> Result<Vec<ColumnSchema>, DbError> {
        let safe = table.replace('"', "");

        let info    = self.fetch_all(&format!("PRAGMA table_info(\"{}\")", safe)).await?;
        let fk_list = self.fetch_all(&format!("PRAGMA foreign_key_list(\"{}\")", safe)).await?;

        // PRAGMA foreign_key_list: id(0) seq(1) table(2) from(3) to(4) …
        let mut fk_map: HashMap<String, ForeignKey> = HashMap::new();
        for row in &fk_list.rows {
            if let (Some(Value::Text(from)), Some(Value::Text(tbl)), Some(Value::Text(to))) =
                (row.values.get(3), row.values.get(2), row.values.get(4))
            {
                fk_map.insert(from.clone(), ForeignKey { table: tbl.clone(), column: to.clone() });
            }
        }

        // PRAGMA table_info: cid(0) name(1) type(2) notnull(3) dflt_value(4) pk(5)
        let schema = info.rows.iter()
            .filter_map(|row| {
                let name = match row.values.get(1)? {
                    Value::Text(s) => s.clone(),
                    _ => return None,
                };
                let type_name = match row.values.get(2)? {
                    Value::Text(s) => s.clone(),
                    _ => String::new(),
                };
                let notnull   = matches!(row.values.get(3), Some(Value::Int(1)));
                let pk        = matches!(row.values.get(5), Some(Value::Int(n)) if *n > 0);
                let fk        = fk_map.get(&name).cloned();
                Some(ColumnSchema { name, type_name, is_pk: pk, is_nullable: !notnull, fk })
            })
            .collect();

        Ok(schema)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn to_value(v: libsql::Value) -> Value {
    match v {
        libsql::Value::Null        => Value::Null,
        libsql::Value::Integer(n)  => Value::Int(n),
        libsql::Value::Real(f)     => Value::Float(f),
        libsql::Value::Text(s)     => Value::Text(s),
        libsql::Value::Blob(b)     => Value::Bytes(b),
    }
}

/// Parse `libsql://host?authToken=TOKEN` into `(base_url, token)`.
fn parse_url(url: &str) -> Result<(String, String), DbError> {
    if let Some(pos) = url.find("?authToken=") {
        let base  = url[..pos].to_string();
        let token = url[pos + "?authToken=".len()..].to_string();
        if base.is_empty() || token.is_empty() {
            return Err(DbError::ConnectionFailed(
                "Invalid Turso URL: base URL or authToken is empty".into(),
            ));
        }
        Ok((base, token))
    } else {
        Err(DbError::ConnectionFailed(
            "Invalid Turso URL. Expected format: libsql://host?authToken=TOKEN".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_url, TursoClient};
    use crate::db::{error::DbError, traits::SqlClient, types::{TableKind, Value}};
    use std::sync::atomic::{AtomicU32, Ordering};

    // ── helpers ──────────────────────────────────────────────────────────────────

    fn turso_url() -> Option<String> {
        std::env::var("TURSO_URL").ok()
    }

    fn unique_table() -> String {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        format!("_rowdy_ts_test_{}", COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    async fn connected(url: &str) -> TursoClient {
        let mut c = TursoClient::new();
        c.connect(url).await.expect("connect failed");
        println!("  [turso] connected");
        c
    }

    async fn drop_table(c: &TursoClient, tbl: &str) {
        c.execute(&format!("DROP TABLE IF EXISTS \"{tbl}\"")).await.ok();
    }

    // ── parse_url (pure) ─────────────────────────────────────────────────────────

    #[test]
    fn test_parse_url_valid() {
        let (base, token) = parse_url("libsql://mydb.turso.io?authToken=abc123").unwrap();
        assert_eq!(base, "libsql://mydb.turso.io");
        assert_eq!(token, "abc123");
        println!("  ✓ valid URL splits correctly");
    }

    #[test]
    fn test_parse_url_missing_token() {
        let e = parse_url("libsql://mydb.turso.io?authToken=").unwrap_err();
        assert!(matches!(e, DbError::ConnectionFailed(_)));
        println!("  ✓ empty token → ConnectionFailed");
    }

    #[test]
    fn test_parse_url_no_auth_token() {
        let e = parse_url("libsql://mydb.turso.io").unwrap_err();
        assert!(matches!(e, DbError::ConnectionFailed(_)));
        println!("  ✓ missing authToken → ConnectionFailed");
    }

    // ── connection ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_connect() {
        println!("\n[turso] test_connect");
        let Some(url) = turso_url() else {
            println!("  TURSO_URL not set — skipped");
            return;
        };
        let mut c = TursoClient::new();
        assert!(c.connect(&url).await.is_ok());
        println!("  ✓ connection OK");
    }

    #[tokio::test]
    async fn test_not_connected_returns_error() {
        println!("\n[turso] test_not_connected_returns_error");
        let c = TursoClient::new();
        assert!(matches!(c.fetch_all("SELECT 1").await.unwrap_err(), DbError::NotConnected));
        assert!(matches!(c.execute("SELECT 1").await.unwrap_err(), DbError::NotConnected));
        assert!(matches!(c.get_tables().await.unwrap_err(), DbError::NotConnected));
        println!("  ✓ fetch_all/execute/get_tables before connect → NotConnected");
    }

    #[tokio::test]
    async fn test_disconnect() {
        println!("\n[turso] test_disconnect");
        let Some(url) = turso_url() else {
            println!("  TURSO_URL not set — skipped");
            return;
        };
        let mut c = connected(&url).await;
        assert!(c.disconnect().await.is_ok());
        assert!(matches!(c.fetch_all("SELECT 1").await.unwrap_err(), DbError::NotConnected));
        println!("  ✓ disconnect OK, subsequent fetch_all → NotConnected");
    }

    // ── execute ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_execute_insert_rows_affected() {
        println!("\n[turso] test_execute_insert_rows_affected");
        let Some(url) = turso_url() else {
            println!("  TURSO_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        c.execute(&format!("CREATE TABLE IF NOT EXISTS \"{tbl}\" (id INTEGER, val TEXT)")).await.unwrap();
        let n = c.execute(&format!("INSERT INTO \"{tbl}\" VALUES (1,'a'),(2,'b'),(3,'c')")).await.expect("INSERT failed");
        println!("  INSERT rows affected = {n}");
        assert_eq!(n, 3);
        drop_table(&c, &tbl).await;
        println!("  ✓ INSERT 3 rows → 3 affected");
    }

    #[tokio::test]
    async fn test_execute_update_rows_affected() {
        println!("\n[turso] test_execute_update_rows_affected");
        let Some(url) = turso_url() else {
            println!("  TURSO_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        c.execute(&format!("CREATE TABLE IF NOT EXISTS \"{tbl}\" (id INTEGER, val TEXT)")).await.unwrap();
        c.execute(&format!("INSERT INTO \"{tbl}\" VALUES (1,'x'),(2,'x'),(3,'y')")).await.unwrap();
        let n = c.execute(&format!("UPDATE \"{tbl}\" SET val = 'z' WHERE val = 'x'")).await.expect("UPDATE failed");
        println!("  UPDATE rows affected = {n}");
        assert_eq!(n, 2);
        drop_table(&c, &tbl).await;
        println!("  ✓ UPDATE 2 matching rows → 2 affected");
    }

    #[tokio::test]
    async fn test_execute_invalid_sql() {
        println!("\n[turso] test_execute_invalid_sql");
        let Some(url) = turso_url() else {
            println!("  TURSO_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let e = c.execute("NOT VALID SQL!!!").await.unwrap_err();
        assert!(matches!(e, DbError::QueryFailed(_)));
        println!("  ✓ invalid SQL → QueryFailed");
    }

    // ── fetch_all ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_fetch_all_empty_result() {
        println!("\n[turso] test_fetch_all_empty_result");
        let Some(url) = turso_url() else {
            println!("  TURSO_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        c.execute(&format!("CREATE TABLE IF NOT EXISTS \"{tbl}\" (id INTEGER, name TEXT)")).await.unwrap();
        let r = c.fetch_all(&format!("SELECT * FROM \"{tbl}\"")).await.expect("fetch_all failed");
        println!("  cols={} rows={}", r.columns.len(), r.rows.len());
        assert_eq!(r.columns.len(), 2);
        assert_eq!(r.rows.len(), 0);
        drop_table(&c, &tbl).await;
        println!("  ✓ empty table → 2 columns, 0 rows");
    }

    #[tokio::test]
    async fn test_fetch_all_column_and_row_count() {
        println!("\n[turso] test_fetch_all_column_and_row_count");
        let Some(url) = turso_url() else {
            println!("  TURSO_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        c.execute(&format!(
            "CREATE TABLE IF NOT EXISTS \"{tbl}\" (id INTEGER, title TEXT, price REAL, available INTEGER)",
        )).await.unwrap();
        c.execute(&format!(
            "INSERT INTO \"{tbl}\" VALUES (1,'1984',9.99,1),(2,'The Plague',12.50,1),(3,'The Trial',NULL,0)",
        )).await.unwrap();
        let r = c.fetch_all(&format!("SELECT * FROM \"{tbl}\"")).await.expect("fetch_all failed");
        println!("  cols={} rows={}", r.columns.len(), r.rows.len());
        assert_eq!(r.columns.len(), 4);
        assert_eq!(r.rows.len(), 3);
        drop_table(&c, &tbl).await;
        println!("  ✓ 4 columns, 3 rows");
    }

    #[tokio::test]
    async fn test_fetch_all_scalar_types() {
        println!("\n[turso] test_fetch_all_scalar_types");
        let Some(url) = turso_url() else {
            println!("  TURSO_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let r = c
            .fetch_all("SELECT 42, 3.14, 'hello', NULL")
            .await
            .expect("fetch_all failed");
        assert_eq!(r.rows.len(), 1);
        let row = &r.rows[0];
        println!("  values: {:?}", row.values);
        assert!(matches!(row.values[0], Value::Int(42)));
        assert!(matches!(row.values[1], Value::Float(f) if (f - 3.14).abs() < 1e-9));
        assert!(matches!(row.values[2], Value::Text(ref s) if s == "hello"));
        assert!(matches!(row.values[3], Value::Null));
        println!("  ✓ Int/Float/Text/Null types OK");
    }

    // ── get_table_objects ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_table_objects_kinds() {
        println!("\n[turso] test_get_table_objects_kinds");
        let Some(url) = turso_url() else {
            println!("  TURSO_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        let view = format!("{tbl}_view");
        c.execute(&format!("CREATE TABLE IF NOT EXISTS \"{tbl}\" (id INTEGER, val TEXT)")).await.unwrap();
        c.execute(&format!("INSERT INTO \"{tbl}\" VALUES (1,'x')")).await.unwrap();
        c.execute(&format!("CREATE VIEW IF NOT EXISTS \"{view}\" AS SELECT * FROM \"{tbl}\"")).await.unwrap();

        let objects = c.get_table_objects().await.expect("get_table_objects failed");
        println!("  found {} objects total", objects.len());

        let t = objects.iter().find(|o| o.name == tbl).expect("table not found");
        let v = objects.iter().find(|o| o.name == view).expect("view not found");
        assert_eq!(t.kind, TableKind::Table);
        assert_eq!(v.kind, TableKind::View);

        c.execute(&format!("DROP VIEW IF EXISTS \"{view}\"")).await.ok();
        drop_table(&c, &tbl).await;
        println!("  ✓ TABLE and VIEW kinds correct");
    }

    // ── get_schema ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_schema_pk_and_fk() {
        println!("\n[turso] test_get_schema_pk_and_fk");
        let Some(url) = turso_url() else {
            println!("  TURSO_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let parent = unique_table();
        let child  = unique_table();

        c.execute(&format!("CREATE TABLE IF NOT EXISTS \"{parent}\" (id INTEGER PRIMARY KEY, name TEXT)")).await.unwrap();
        c.execute(&format!(
            "CREATE TABLE IF NOT EXISTS \"{child}\" (\
                id INTEGER PRIMARY KEY, \
                label TEXT, \
                parent_id INTEGER REFERENCES \"{parent}\"(id)\
            )",
        )).await.unwrap();

        let schema = c.get_schema(&child).await.expect("get_schema failed");
        println!("  schema: {:?}", schema.iter().map(|c| (&c.name, c.is_pk, c.fk.is_some())).collect::<Vec<_>>());

        let id_col = schema.iter().find(|c| c.name == "id").expect("id not found");
        let fk_col = schema.iter().find(|c| c.name == "parent_id").expect("parent_id not found");
        assert!(id_col.is_pk, "id should be PK");
        assert!(fk_col.fk.is_some(), "parent_id should have FK");
        assert_eq!(fk_col.fk.as_ref().unwrap().table, parent);

        drop_table(&c, &child).await;
        drop_table(&c, &parent).await;
        println!("  ✓ id is PK, parent_id FK → {parent}");
    }

    #[tokio::test]
    async fn test_get_schema_type_names() {
        println!("\n[turso] test_get_schema_type_names");
        let Some(url) = turso_url() else {
            println!("  TURSO_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        c.execute(&format!(
            "CREATE TABLE IF NOT EXISTS \"{tbl}\" (id INTEGER PRIMARY KEY, label TEXT, price REAL, flag INTEGER)",
        )).await.unwrap();

        let schema = c.get_schema(&tbl).await.expect("get_schema failed");
        println!("  schema: {:?}", schema.iter().map(|c| (&c.name, &c.type_name)).collect::<Vec<_>>());

        let price = schema.iter().find(|c| c.name == "price").expect("price not found");
        let label = schema.iter().find(|c| c.name == "label").expect("label not found");
        assert_eq!(price.type_name.to_uppercase(), "REAL");
        assert_eq!(label.type_name.to_uppercase(), "TEXT");

        drop_table(&c, &tbl).await;
        println!("  ✓ type names REAL and TEXT");
    }
}
