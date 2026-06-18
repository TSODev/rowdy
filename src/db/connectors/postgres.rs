use async_trait::async_trait;
use sqlx::{
    postgres::{PgPool, PgRow},
    Column as SqlxColumn, Row as SqlxRow, TypeInfo, ValueRef,
};
use crate::db::error::DbError;
use crate::db::traits::SqlClient;
use crate::db::types::{Column, ColumnSchema, DbQueryResult, ForeignKey, Row, TableKind, TableObject, Value};

pub struct PostgresConnector {
    pool: Option<PgPool>,
}

impl PostgresConnector {
    pub fn new() -> Self {
        Self { pool: None }
    }

    fn pool(&self) -> Result<&PgPool, DbError> {
        self.pool.as_ref().ok_or(DbError::NotConnected)
    }
}

#[async_trait]
impl SqlClient for PostgresConnector {
    async fn connect(&mut self, url: &str) -> Result<(), DbError> {
        let pool = PgPool::connect(url)
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
        self.pool = Some(pool);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DbError> {
        if let Some(pool) = self.pool.take() {
            pool.close().await;
        }
        Ok(())
    }

    async fn execute(&self, query: &str) -> Result<u64, DbError> {
        let result = sqlx::query(query)
            .execute(self.pool()?)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(result.rows_affected())
    }

    async fn fetch_all(&self, query: &str) -> Result<DbQueryResult, DbError> {
        let rows: Vec<PgRow> = sqlx::query(query)
            .fetch_all(self.pool()?)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        if rows.is_empty() {
            return Ok(DbQueryResult {
                columns: vec![],
                rows: vec![],
                rows_affected: 0,
            });
        }

        let columns: Vec<Column> = rows[0]
            .columns()
            .iter()
            .map(|c| Column {
                name: c.name().to_string(),
                type_name: c.type_info().name().to_string(),
            })
            .collect();

        let mapped_rows: Vec<Row> = rows
            .iter()
            .map(|r| Row {
                values: (0..r.len()).map(|i| pg_value(r, i)).collect(),
            })
            .collect();

        let count = mapped_rows.len() as u64;
        Ok(DbQueryResult {
            columns,
            rows: mapped_rows,
            rows_affected: count,
        })
    }

    async fn get_tables(&self) -> Result<Vec<String>, DbError> {
        let rows: Vec<PgRow> = sqlx::query(
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema = 'public' AND table_type = 'BASE TABLE' \
             ORDER BY table_name",
        )
        .fetch_all(self.pool()?)
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| r.try_get::<String, _>(0).unwrap_or_default())
            .collect())
    }

    async fn get_table_objects(&self) -> Result<Vec<TableObject>, DbError> {
        let rows: Vec<PgRow> = sqlx::query(
            "SELECT table_name, table_type FROM information_schema.tables \
             WHERE table_schema = 'public' AND table_type IN ('BASE TABLE', 'VIEW') \
             ORDER BY table_name",
        )
        .fetch_all(self.pool()?)
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(rows.iter().map(|r| {
            let name = r.try_get::<String, _>(0).unwrap_or_default();
            let type_str = r.try_get::<String, _>(1).unwrap_or_default();
            let kind = if type_str == "VIEW" { TableKind::View } else { TableKind::Table };
            TableObject { name, kind }
        }).collect())
    }

    async fn get_schema(&self, table: &str) -> Result<Vec<ColumnSchema>, DbError> {
        let pool = self.pool()?;
        let safe = table.replace(['\'', '"'], "");
        // Correlated subqueries avoid JOIN fan-out and the array_position()
        // type-resolution edge-cases that silently broke the previous query.
        let query = format!(
            r#"SELECT
                a.attname::text AS column_name,
                pg_catalog.format_type(a.atttypid, a.atttypmod) AS data_type,
                (NOT a.attnotnull) AS is_nullable,
                EXISTS(
                    SELECT 1 FROM pg_constraint c
                    WHERE c.conrelid = a.attrelid
                      AND c.contype  = 'p'
                      AND a.attnum   = ANY(c.conkey)
                ) AS is_pk,
                (
                    SELECT rc.relname::text
                    FROM pg_constraint c
                    JOIN pg_class rc ON rc.oid = c.confrelid
                    WHERE c.conrelid = a.attrelid
                      AND c.contype  = 'f'
                      AND a.attnum   = ANY(c.conkey)
                    LIMIT 1
                ) AS foreign_table_name,
                (
                    SELECT ra.attname::text
                    FROM pg_constraint c
                    JOIN pg_attribute ra ON ra.attrelid = c.confrelid
                                        AND ra.attnum   = c.confkey[1]
                    WHERE c.conrelid = a.attrelid
                      AND c.contype  = 'f'
                      AND a.attnum   = ANY(c.conkey)
                    LIMIT 1
                ) AS foreign_column_name
            FROM pg_attribute a
            JOIN pg_class     cl ON cl.oid = a.attrelid
            JOIN pg_namespace n  ON n.oid  = cl.relnamespace
            WHERE n.nspname  = 'public'
              AND cl.relname = '{safe}'
              AND a.attnum   > 0
              AND NOT a.attisdropped
            ORDER BY a.attnum"#
        );

        let rows: Vec<PgRow> = sqlx::query(&query)
            .fetch_all(pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut schema = vec![];
        for row in &rows {
            let name: String = row.try_get("column_name").unwrap_or_default();
            let type_name: String = row.try_get("data_type").unwrap_or_default();
            let is_nullable: bool = row.try_get("is_nullable").unwrap_or(true);
            let is_pk: bool = row.try_get("is_pk").unwrap_or(false);
            let fk_table: Option<String> = row.try_get("foreign_table_name").unwrap_or(None);
            let fk_col: Option<String> = row.try_get("foreign_column_name").unwrap_or(None);
            let fk = match (fk_table, fk_col) {
                (Some(t), Some(c)) if !t.is_empty() => Some(ForeignKey { table: t, column: c }),
                _ => None,
            };
            schema.push(ColumnSchema { name, type_name, is_pk, is_nullable, fk });
        }
        Ok(schema)
    }
}

fn pg_value(row: &PgRow, index: usize) -> Value {
    use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
    let raw = row.try_get_raw(index).unwrap();
    if raw.is_null() {
        return Value::Null;
    }
    let tn = raw.type_info().name().to_string();
    let marker = || Value::Text(format!("<?{tn}>"));
    match tn.as_str() {
        "BOOL"           => row.try_get::<bool, _>(index).map(Value::Bool).unwrap_or_else(|_| marker()),
        // Each integer width requires its own Rust type in sqlx
        "INT2"           => row.try_get::<i16, _>(index).map(|v| Value::Int(v as i64)).unwrap_or_else(|_| marker()),
        "INT4"           => row.try_get::<i32, _>(index).map(|v| Value::Int(v as i64)).unwrap_or_else(|_| marker()),
        "INT8"           => row.try_get::<i64, _>(index).map(Value::Int).unwrap_or_else(|_| marker()),
        "FLOAT4"         => row.try_get::<f32, _>(index).map(|v| Value::Float(v as f64)).unwrap_or_else(|_| marker()),
        "FLOAT8"         => row.try_get::<f64, _>(index).map(Value::Float).unwrap_or_else(|_| marker()),
        "NUMERIC"        => row.try_get::<bigdecimal::BigDecimal, _>(index)
                               .map(|d| Value::Text(crate::db::types::format_decimal(d)))
                               .unwrap_or_else(|_| marker()),
        "BYTEA"          => row.try_get::<Vec<u8>, _>(index).map(Value::Bytes).unwrap_or_else(|_| marker()),
        // Dates and times
        "DATE"           => row.try_get::<NaiveDate, _>(index).map(|d| Value::Text(d.to_string())).unwrap_or_else(|_| marker()),
        "TIME"           => row.try_get::<NaiveTime, _>(index).map(|t| Value::Text(t.to_string())).unwrap_or_else(|_| marker()),
        "TIMESTAMP"      => row.try_get::<NaiveDateTime, _>(index).map(|dt| Value::Text(dt.to_string())).unwrap_or_else(|_| marker()),
        "TIMESTAMPTZ"    => row.try_get::<DateTime<Utc>, _>(index).map(|dt| Value::Text(dt.to_rfc3339())).unwrap_or_else(|_| marker()),
        // UUID
        "UUID"           => row.try_get::<uuid::Uuid, _>(index).map(|u| Value::Text(u.to_string())).unwrap_or_else(|_| marker()),
        // JSON / JSONB — decoded via serde_json then serialised back to a compact string
        "JSON" | "JSONB" => row.try_get::<serde_json::Value, _>(index).map(|v| Value::Text(v.to_string())).unwrap_or_else(|_| marker()),
        // Arrays: OID names start with '_', e.g. _TEXT, _INT4, _UUID
        // Try Vec<String> first (text arrays), then Vec<i64> (int arrays), else marker
        s if s.starts_with('_') => {
            if let Ok(v) = row.try_get::<Vec<String>, _>(index) {
                return Value::Text(format!("[{}]", v.join(", ")));
            }
            if let Ok(v) = row.try_get::<Vec<i64>, _>(index) {
                return Value::Text(format!("[{}]", v.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")));
            }
            if let Ok(v) = row.try_get::<Vec<bool>, _>(index) {
                return Value::Text(format!("[{}]", v.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(", ")));
            }
            marker()
        }
        // Text-like types (TEXT, VARCHAR, CHAR, BPCHAR, NAME, XML, INTERVAL, INET, CIDR, MACADDR…)
        _ => row.try_get::<String, _>(index).map(Value::Text).unwrap_or_else(|_| marker()),
    }
}

#[cfg(test)]
mod tests {
    use super::PostgresConnector;
    use crate::db::{
        error::DbError,
        traits::SqlClient,
        types::{TableKind, Value},
    };
    use std::sync::atomic::{AtomicU32, Ordering};

    // ── helpers ──────────────────────────────────────────────────────────────────

    fn pg_url() -> Option<String> {
        std::env::var("POSTGRES_URL").ok()
    }

    /// Masks password in URL for display: postgres://user:***@host/db
    fn display_url(url: &str) -> String {
        if let (Some(p), Some(at)) = (url.find("://"), url.rfind('@')) {
            return format!("{}***{}", &url[..p + 3], &url[at..]);
        }
        url.to_string()
    }

    /// Each test gets a unique table name to avoid cross-test conflicts.
    fn unique_table() -> String {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        format!("_rowdy_pg_test_{}", COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    async fn connected(url: &str) -> PostgresConnector {
        let mut c = PostgresConnector::new();
        c.connect(url).await.expect("connect failed");
        println!("  [postgres] connected to {}", display_url(url));
        c
    }

    /// Creates a self-referencing test table: id(PK), label, amount, flag, ref_id(FK→id).
    async fn create_test_table(c: &PostgresConnector, tbl: &str) {
        c.execute(&format!("DROP TABLE IF EXISTS {tbl} CASCADE")).await.ok();
        c.execute(&format!(
            "CREATE TABLE {tbl} (
                id     SERIAL  PRIMARY KEY,
                label  TEXT    NOT NULL,
                amount NUMERIC(10,2),
                flag   BOOLEAN DEFAULT FALSE,
                ref_id INTEGER REFERENCES {tbl}(id)
            )"
        )).await.expect("CREATE TABLE failed");
        println!("  [postgres] created {tbl}");
    }

    async fn drop_test_table(c: &PostgresConnector, tbl: &str) {
        c.execute(&format!("DROP TABLE IF EXISTS {tbl} CASCADE")).await.ok();
        println!("  [postgres] dropped {tbl}");
    }

    // ── connection ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_connect() {
        println!("\n[postgres] test_connect");
        let Some(url) = pg_url() else {
            println!("  POSTGRES_URL not set — skipped");
            return;
        };
        let mut c = PostgresConnector::new();
        let result = c.connect(&url).await;
        assert!(result.is_ok(), "connect failed: {:?}", result);
        println!("  ✓ connection OK");
    }

    #[tokio::test]
    async fn test_not_connected_returns_error() {
        println!("\n[postgres] test_not_connected_returns_error");
        let c = PostgresConnector::new();
        let e = c.fetch_all("SELECT 1").await.unwrap_err();
        assert!(matches!(e, DbError::NotConnected));
        println!("  ✓ fetch_all before connect → NotConnected");
        let e2 = c.execute("SELECT 1").await.unwrap_err();
        assert!(matches!(e2, DbError::NotConnected));
        println!("  ✓ execute before connect → NotConnected");
    }

    #[tokio::test]
    async fn test_disconnect() {
        println!("\n[postgres] test_disconnect");
        let Some(url) = pg_url() else {
            println!("  POSTGRES_URL not set — skipped");
            return;
        };
        let mut c = connected(&url).await;
        assert!(c.disconnect().await.is_ok(), "disconnect should not error");
        println!("  ✓ disconnect OK");
    }

    // ── type mapping (inline casts — no tables required) ─────────────────────────

    #[tokio::test]
    async fn test_fetch_all_scalar_types() {
        println!("\n[postgres] test_fetch_all_scalar_types");
        let Some(url) = pg_url() else {
            println!("  POSTGRES_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let result = c.fetch_all(
            "SELECT
                42::INT4         AS int_col,
                3.14::FLOAT8     AS float_col,
                TRUE::BOOL       AS bool_col,
                'hello'::TEXT    AS text_col,
                '99.99'::NUMERIC AS numeric_col,
                NULL::TEXT       AS null_col",
        ).await.expect("fetch_all failed");

        println!("  columns: {:?}", result.columns.iter().map(|c| &c.name).collect::<Vec<_>>());
        println!("  row[0]: {:?}", result.rows[0].values);

        assert_eq!(result.rows.len(), 1);
        let row = &result.rows[0];
        assert!(matches!(row.values[0], Value::Int(42)),    "INT4 → Int(42)");
        assert!(matches!(row.values[1], Value::Float(_)),   "FLOAT8 → Float");
        assert!(matches!(row.values[2], Value::Bool(true)), "BOOL → Bool(true)");
        assert!(matches!(row.values[3], Value::Text(_)),    "TEXT → Text");
        assert!(matches!(row.values[4], Value::Text(_)),    "NUMERIC → Text (formatted decimal)");
        assert!(matches!(row.values[5], Value::Null),       "NULL → Null");
        println!("  ✓ INT4, FLOAT8, BOOL, TEXT, NUMERIC, NULL decoded correctly");
    }

    #[tokio::test]
    async fn test_fetch_all_date_time_types() {
        println!("\n[postgres] test_fetch_all_date_time_types");
        let Some(url) = pg_url() else {
            println!("  POSTGRES_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let result = c.fetch_all(
            "SELECT
                '2024-01-15'::DATE              AS date_col,
                '14:30:00'::TIME                AS time_col,
                '2024-01-15 14:30:00'::TIMESTAMP AS ts_col,
                NOW()::TIMESTAMPTZ              AS tstz_col",
        ).await.expect("fetch_all date types failed");

        println!("  row[0]: {:?}", result.rows[0].values);
        let row = &result.rows[0];
        assert!(matches!(row.values[0], Value::Text(_)), "DATE → Text");
        assert!(matches!(row.values[1], Value::Text(_)), "TIME → Text");
        assert!(matches!(row.values[2], Value::Text(_)), "TIMESTAMP → Text");
        assert!(matches!(row.values[3], Value::Text(_)), "TIMESTAMPTZ → Text");
        println!("  ✓ DATE, TIME, TIMESTAMP, TIMESTAMPTZ decoded as Text");
    }

    #[tokio::test]
    async fn test_fetch_all_int_widths() {
        println!("\n[postgres] test_fetch_all_int_widths");
        let Some(url) = pg_url() else {
            println!("  POSTGRES_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let result = c.fetch_all(
            "SELECT 1::INT2 AS i2, 2::INT4 AS i4, 3::INT8 AS i8",
        ).await.expect("fetch_all int widths failed");

        let row = &result.rows[0];
        println!("  row[0]: {:?}", row.values);
        assert!(matches!(row.values[0], Value::Int(1)), "INT2 → Int");
        assert!(matches!(row.values[1], Value::Int(2)), "INT4 → Int");
        assert!(matches!(row.values[2], Value::Int(3)), "INT8 → Int");
        println!("  ✓ INT2, INT4, INT8 all decoded as Int");
    }

    // ── schema introspection ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_table_objects_kinds() {
        println!("\n[postgres] test_get_table_objects_kinds");
        let Some(url) = pg_url() else {
            println!("  POSTGRES_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        let view = format!("{tbl}_view");

        create_test_table(&c, &tbl).await;
        c.execute(&format!(
            "CREATE OR REPLACE VIEW {view} AS SELECT id, label FROM {tbl}"
        )).await.expect("CREATE VIEW failed");

        let objects = c.get_table_objects().await.expect("get_table_objects failed");
        println!("  {} objects in public schema", objects.len());

        let t = objects.iter().find(|o| o.name == tbl);
        assert!(t.is_some(), "{tbl} not found");
        assert_eq!(t.unwrap().kind, TableKind::Table, "{tbl} should be Table");

        let v = objects.iter().find(|o| o.name == view);
        assert!(v.is_some(), "{view} not found");
        assert_eq!(v.unwrap().kind, TableKind::View, "{view} should be View");

        c.execute(&format!("DROP VIEW IF EXISTS {view}")).await.ok();
        drop_test_table(&c, &tbl).await;
        println!("  ✓ TABLE → Table, VIEW → View");
    }

    #[tokio::test]
    async fn test_get_schema_pk_and_fk() {
        println!("\n[postgres] test_get_schema_pk_and_fk");
        let Some(url) = pg_url() else {
            println!("  POSTGRES_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        create_test_table(&c, &tbl).await;

        let schema = c.get_schema(&tbl).await.expect("get_schema failed");
        println!("  schema: {:?}", schema.iter().map(|s| (&s.name, s.is_pk, s.fk.is_some())).collect::<Vec<_>>());

        let id = schema.iter().find(|s| s.name == "id").expect("id missing");
        assert!(id.is_pk, "id should be PK");
        assert!(id.fk.is_none(), "id should have no FK");

        let label = schema.iter().find(|s| s.name == "label").expect("label missing");
        assert!(!label.is_pk, "label should not be PK");
        assert!(label.fk.is_none(), "label should have no FK");

        let ref_id = schema.iter().find(|s| s.name == "ref_id").expect("ref_id missing");
        let fk = ref_id.fk.as_ref().expect("ref_id should have a FK");
        assert_eq!(fk.table, tbl, "FK table should be {tbl}");
        assert_eq!(fk.column, "id");

        drop_test_table(&c, &tbl).await;
        println!("  ✓ id → PK, label → no FK, ref_id → FK({tbl}.id)");
    }

    #[tokio::test]
    async fn test_get_schema_type_names() {
        println!("\n[postgres] test_get_schema_type_names");
        let Some(url) = pg_url() else {
            println!("  POSTGRES_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        create_test_table(&c, &tbl).await;

        let schema = c.get_schema(&tbl).await.expect("get_schema failed");
        let types: Vec<(&str, &str)> = schema.iter()
            .map(|s| (s.name.as_str(), s.type_name.as_str()))
            .collect();
        println!("  types: {:?}", types);

        let amount = schema.iter().find(|s| s.name == "amount").expect("amount missing");
        assert!(amount.type_name.contains("numeric"), "amount should be numeric type, got '{}'", amount.type_name);

        let flag = schema.iter().find(|s| s.name == "flag").expect("flag missing");
        assert!(flag.type_name.contains("boolean"), "flag should be boolean type, got '{}'", flag.type_name);

        drop_test_table(&c, &tbl).await;
        println!("  ✓ NUMERIC and BOOLEAN type names returned correctly");
    }

    // ── fetch_all ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_fetch_all_column_and_row_count() {
        println!("\n[postgres] test_fetch_all_column_and_row_count");
        let Some(url) = pg_url() else {
            println!("  POSTGRES_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        create_test_table(&c, &tbl).await;

        c.execute(&format!(
            "INSERT INTO {tbl} (label, amount) VALUES ('alpha', 10.00), ('beta', 20.50), ('gamma', NULL)"
        )).await.unwrap();

        let result = c.fetch_all(&format!("SELECT * FROM {tbl} ORDER BY id")).await.expect("fetch_all failed");
        println!("  columns: {:?}", result.columns.iter().map(|c| &c.name).collect::<Vec<_>>());
        println!("  rows: {}", result.rows.len());

        assert_eq!(result.columns.len(), 5, "test table has 5 columns");
        assert_eq!(result.rows.len(), 3);
        assert_eq!(result.columns[0].name, "id");
        assert_eq!(result.columns[1].name, "label");

        drop_test_table(&c, &tbl).await;
        println!("  ✓ 5 columns, 3 rows");
    }

    #[tokio::test]
    async fn test_fetch_all_empty_result() {
        println!("\n[postgres] test_fetch_all_empty_result");
        let Some(url) = pg_url() else {
            println!("  POSTGRES_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let result = c.fetch_all("SELECT 1 WHERE FALSE").await.expect("fetch_all failed");
        assert_eq!(result.rows.len(), 0);
        println!("  ✓ SELECT … WHERE FALSE → 0 rows");
    }

    // ── execute ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_execute_insert_and_delete() {
        println!("\n[postgres] test_execute_insert_and_delete");
        let Some(url) = pg_url() else {
            println!("  POSTGRES_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        create_test_table(&c, &tbl).await;

        let n_ins = c.execute(&format!(
            "INSERT INTO {tbl} (label) VALUES ('row_a'), ('row_b')"
        )).await.expect("INSERT failed");
        println!("  INSERT rows_affected: {n_ins}");
        assert_eq!(n_ins, 2);

        let n_del = c.execute(&format!(
            "DELETE FROM {tbl} WHERE label IN ('row_a', 'row_b')"
        )).await.expect("DELETE failed");
        println!("  DELETE rows_affected: {n_del}");
        assert_eq!(n_del, 2);

        drop_test_table(&c, &tbl).await;
        println!("  ✓ INSERT → 2, DELETE → 2");
    }

    #[tokio::test]
    async fn test_execute_update_rows_affected() {
        println!("\n[postgres] test_execute_update_rows_affected");
        let Some(url) = pg_url() else {
            println!("  POSTGRES_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        create_test_table(&c, &tbl).await;

        c.execute(&format!(
            "INSERT INTO {tbl} (label, flag) VALUES ('x', FALSE), ('y', FALSE), ('z', TRUE)"
        )).await.unwrap();

        let n = c.execute(&format!("UPDATE {tbl} SET flag = TRUE WHERE flag = FALSE")).await.expect("UPDATE failed");
        println!("  UPDATE rows_affected: {n}");
        assert_eq!(n, 2, "2 rows had flag=FALSE");

        drop_test_table(&c, &tbl).await;
        println!("  ✓ UPDATE → rows_affected=2");
    }

    #[tokio::test]
    async fn test_execute_invalid_sql() {
        println!("\n[postgres] test_execute_invalid_sql");
        let Some(url) = pg_url() else {
            println!("  POSTGRES_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let err = c.execute("THIS IS NOT VALID SQL").await.unwrap_err();
        assert!(matches!(err, DbError::QueryFailed(_)));
        println!("  ✓ invalid SQL → QueryFailed");
    }
}
