use async_trait::async_trait;
use sqlx::{
    mysql::{MySqlPool, MySqlRow},
    Column as SqlxColumn, Row as SqlxRow, TypeInfo, ValueRef,
};
use crate::db::error::DbError;
use crate::db::traits::SqlClient;
use crate::db::types::{Column, ColumnSchema, DbQueryResult, ForeignKey, Row, TableKind, TableObject, Value};

pub struct MySqlConnector {
    pool: Option<MySqlPool>,
}

impl MySqlConnector {
    pub fn new() -> Self {
        Self { pool: None }
    }

    fn pool(&self) -> Result<&MySqlPool, DbError> {
        self.pool.as_ref().ok_or(DbError::NotConnected)
    }
}

#[async_trait]
impl SqlClient for MySqlConnector {
    async fn connect(&mut self, url: &str) -> Result<(), DbError> {
        let normalized = normalize_ssl_mode(url);
        let pool = MySqlPool::connect(&normalized)
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
        let rows: Vec<MySqlRow> = sqlx::query(query)
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
                values: (0..r.len()).map(|i| mysql_value(r, i)).collect(),
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
        let rows: Vec<MySqlRow> = sqlx::query(
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE' \
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
        let rows: Vec<MySqlRow> = sqlx::query(
            "SELECT table_name, table_type FROM information_schema.tables \
             WHERE table_schema = DATABASE() AND table_type IN ('BASE TABLE', 'VIEW') \
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
        let safe = table.replace(['\'', '\\'], "");
        let query = format!(
            "SELECT c.COLUMN_NAME, c.DATA_TYPE, c.IS_NULLABLE, c.COLUMN_KEY, \
             kcu.REFERENCED_TABLE_NAME AS FK_TABLE, kcu.REFERENCED_COLUMN_NAME AS FK_COLUMN \
             FROM information_schema.COLUMNS c \
             LEFT JOIN information_schema.KEY_COLUMN_USAGE kcu \
                 ON kcu.TABLE_SCHEMA = c.TABLE_SCHEMA \
                 AND kcu.TABLE_NAME = c.TABLE_NAME \
                 AND kcu.COLUMN_NAME = c.COLUMN_NAME \
                 AND kcu.REFERENCED_TABLE_NAME IS NOT NULL \
             WHERE c.TABLE_SCHEMA = DATABASE() AND c.TABLE_NAME = '{safe}' \
             ORDER BY c.ORDINAL_POSITION"
        );

        let rows: Vec<MySqlRow> = sqlx::query(&query)
            .fetch_all(pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut schema = vec![];
        for row in &rows {
            let name: String = row.try_get("COLUMN_NAME").unwrap_or_default();
            let type_name: String = row.try_get("DATA_TYPE").unwrap_or_default();
            let is_nullable: String = row.try_get("IS_NULLABLE").unwrap_or_else(|_| "YES".into());
            let column_key: String = row.try_get("COLUMN_KEY").unwrap_or_default();
            let fk_table: Option<String> = row.try_get::<Option<String>, _>("FK_TABLE").unwrap_or(None);
            let fk_col: Option<String> = row.try_get::<Option<String>, _>("FK_COLUMN").unwrap_or(None);
            let fk = match (fk_table, fk_col) {
                (Some(t), Some(c)) if !t.is_empty() => Some(ForeignKey { table: t, column: c }),
                _ => None,
            };
            schema.push(ColumnSchema {
                name,
                type_name,
                is_pk: column_key == "PRI",
                is_nullable: is_nullable == "YES",
                fk,
            });
        }
        Ok(schema)
    }
}

#[cfg(test)]
mod tests {
    use super::{MySqlConnector, normalize_ssl_mode};
    use crate::db::{
        error::DbError,
        traits::SqlClient,
        types::{TableKind, Value},
    };
    use std::sync::atomic::{AtomicU32, Ordering};

    // ── helpers ──────────────────────────────────────────────────────────────────

    fn mysql_url() -> Option<String> {
        std::env::var("MYSQL_URL").ok()
    }

    fn display_url(url: &str) -> String {
        if let (Some(p), Some(at)) = (url.find("://"), url.rfind('@')) {
            return format!("{}***{}", &url[..p + 3], &url[at..]);
        }
        url.to_string()
    }

    fn unique_table() -> String {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        format!("_rowdy_my_test_{}", COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    async fn connected(url: &str) -> MySqlConnector {
        let mut c = MySqlConnector::new();
        c.connect(url).await.expect("connect failed");
        println!("  [mysql] connected to {}", display_url(url));
        c
    }

    /// Table with diverse types + self-referencing FK (InnoDB required for FK).
    async fn create_test_table(c: &MySqlConnector, tbl: &str) {
        c.execute(&format!("DROP TABLE IF EXISTS `{tbl}`")).await.ok();
        c.execute(&format!(
            "CREATE TABLE `{tbl}` (
                id           INT AUTO_INCREMENT PRIMARY KEY,
                label        VARCHAR(100) NOT NULL,
                amount       DECIMAL(10,2),
                flag         BOOLEAN DEFAULT FALSE,
                date_val     DATE,
                datetime_val DATETIME,
                ref_id       INT,
                FOREIGN KEY (ref_id) REFERENCES `{tbl}`(id)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"
        )).await.expect("CREATE TABLE failed");
        println!("  [mysql] created `{tbl}`");
    }

    async fn drop_test_table(c: &MySqlConnector, tbl: &str) {
        c.execute(&format!("DROP TABLE IF EXISTS `{tbl}`")).await.ok();
        println!("  [mysql] dropped `{tbl}`");
    }

    // ── normalize_ssl_mode (unit test — no DB needed) ────────────────────────────

    #[test]
    fn test_normalize_ssl_mode_uppercase() {
        println!("\n[mysql] test_normalize_ssl_mode_uppercase");
        let url = "mysql://user:pass@localhost/db?ssl-mode=REQUIRED";
        let result = normalize_ssl_mode(url);
        assert!(result.contains("ssl-mode=required"), "REQUIRED should be lowercased, got: {result}");
        println!("  ✓ ssl-mode=REQUIRED → ssl-mode=required");
    }

    #[test]
    fn test_normalize_ssl_mode_mixed_case() {
        println!("\n[mysql] test_normalize_ssl_mode_mixed_case");
        let url = "mysql://user:pass@localhost/db?ssl-mode=Required";
        let result = normalize_ssl_mode(url);
        assert!(result.contains("ssl-mode=required"), "Required should be lowercased, got: {result}");
        println!("  ✓ ssl-mode=Required → ssl-mode=required");
    }

    #[test]
    fn test_normalize_ssl_mode_already_lowercase() {
        println!("\n[mysql] test_normalize_ssl_mode_already_lowercase");
        let url = "mysql://user:pass@localhost/db?ssl-mode=required";
        let result = normalize_ssl_mode(url);
        // Should be Borrowed (no allocation) when already lowercase
        assert!(result.contains("ssl-mode=required"));
        assert!(matches!(result, std::borrow::Cow::Borrowed(_)), "no-op should return Borrowed");
        println!("  ✓ ssl-mode=required → Borrowed (no allocation)");
    }

    #[test]
    fn test_normalize_ssl_mode_no_ssl_param() {
        println!("\n[mysql] test_normalize_ssl_mode_no_ssl_param");
        let url = "mysql://user:pass@localhost/db";
        let result = normalize_ssl_mode(url);
        assert_eq!(result.as_ref(), url, "URL without ssl-mode should be unchanged");
        assert!(matches!(result, std::borrow::Cow::Borrowed(_)));
        println!("  ✓ no ssl-mode → Borrowed unchanged");
    }

    // ── connection ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_connect() {
        println!("\n[mysql] test_connect");
        let Some(url) = mysql_url() else {
            println!("  MYSQL_URL not set — skipped");
            return;
        };
        let mut c = MySqlConnector::new();
        let result = c.connect(&url).await;
        assert!(result.is_ok(), "connect failed: {:?}", result);
        println!("  ✓ connection OK");
    }

    #[tokio::test]
    async fn test_not_connected_returns_error() {
        println!("\n[mysql] test_not_connected_returns_error");
        let c = MySqlConnector::new();
        let e = c.fetch_all("SELECT 1").await.unwrap_err();
        assert!(matches!(e, DbError::NotConnected));
        println!("  ✓ fetch_all before connect → NotConnected");
        let e2 = c.execute("SELECT 1").await.unwrap_err();
        assert!(matches!(e2, DbError::NotConnected));
        println!("  ✓ execute before connect → NotConnected");
    }

    #[tokio::test]
    async fn test_disconnect() {
        println!("\n[mysql] test_disconnect");
        let Some(url) = mysql_url() else {
            println!("  MYSQL_URL not set — skipped");
            return;
        };
        let mut c = connected(&url).await;
        assert!(c.disconnect().await.is_ok(), "disconnect should not error");
        println!("  ✓ disconnect OK");
    }

    // ── type mapping ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_fetch_all_scalar_types() {
        println!("\n[mysql] test_fetch_all_scalar_types");
        let Some(url) = mysql_url() else {
            println!("  MYSQL_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        create_test_table(&c, &tbl).await;

        c.execute(&format!(
            "INSERT INTO `{tbl}` (label, amount, flag) VALUES ('test', 99.99, TRUE)"
        )).await.unwrap();

        let result = c.fetch_all(&format!(
            "SELECT id, label, amount, flag, date_val FROM `{tbl}` LIMIT 1"
        )).await.expect("fetch_all failed");

        println!("  row[0]: {:?}", result.rows[0].values);
        let row = &result.rows[0];
        assert!(matches!(row.values[0], Value::Int(_)),    "id (INT) → Int");
        assert!(matches!(row.values[1], Value::Text(_)),   "label (VARCHAR) → Text");
        assert!(matches!(row.values[2], Value::Text(_)),   "amount (DECIMAL) → Text (formatted)");
        assert!(matches!(row.values[3], Value::Bool(true)),"flag (BOOLEAN/TINYINT(1)) → Bool(true)");
        assert!(matches!(row.values[4], Value::Null),      "date_val NULL → Null");

        drop_test_table(&c, &tbl).await;
        println!("  ✓ INT, VARCHAR, DECIMAL, BOOLEAN, NULL decoded correctly");
    }

    #[tokio::test]
    async fn test_fetch_all_date_time_types() {
        println!("\n[mysql] test_fetch_all_date_time_types");
        let Some(url) = mysql_url() else {
            println!("  MYSQL_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        create_test_table(&c, &tbl).await;

        c.execute(&format!(
            "INSERT INTO `{tbl}` (label, date_val, datetime_val) \
             VALUES ('dt_test', '2024-01-15', '2024-01-15 14:30:00')"
        )).await.unwrap();

        let result = c.fetch_all(&format!(
            "SELECT date_val, datetime_val FROM `{tbl}` LIMIT 1"
        )).await.expect("fetch_all date types failed");

        println!("  row[0]: {:?}", result.rows[0].values);
        let row = &result.rows[0];
        assert!(matches!(row.values[0], Value::Text(_)), "DATE → Text");
        assert!(matches!(row.values[1], Value::Text(_)), "DATETIME → Text");

        drop_test_table(&c, &tbl).await;
        println!("  ✓ DATE, DATETIME decoded as Text");
    }

    // ── schema introspection ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_table_objects_kinds() {
        println!("\n[mysql] test_get_table_objects_kinds");
        let Some(url) = mysql_url() else {
            println!("  MYSQL_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        let view = format!("{tbl}_view");

        create_test_table(&c, &tbl).await;
        c.execute(&format!(
            "CREATE OR REPLACE VIEW `{view}` AS SELECT id, label FROM `{tbl}`"
        )).await.expect("CREATE VIEW failed");

        let objects = c.get_table_objects().await.expect("get_table_objects failed");
        println!("  {} objects in current schema", objects.len());

        let t = objects.iter().find(|o| o.name == tbl);
        assert!(t.is_some(), "`{tbl}` not found");
        assert_eq!(t.unwrap().kind, TableKind::Table);

        let v = objects.iter().find(|o| o.name == view);
        assert!(v.is_some(), "`{view}` not found");
        assert_eq!(v.unwrap().kind, TableKind::View);

        c.execute(&format!("DROP VIEW IF EXISTS `{view}`")).await.ok();
        drop_test_table(&c, &tbl).await;
        println!("  ✓ BASE TABLE → Table, VIEW → View");
    }

    #[tokio::test]
    async fn test_get_schema_pk_and_fk() {
        println!("\n[mysql] test_get_schema_pk_and_fk");
        let Some(url) = mysql_url() else {
            println!("  MYSQL_URL not set — skipped");
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
        println!("  ✓ id → PK, label → no FK, ref_id → FK(`{tbl}`.id)");
    }

    #[tokio::test]
    async fn test_get_schema_type_names() {
        println!("\n[mysql] test_get_schema_type_names");
        let Some(url) = mysql_url() else {
            println!("  MYSQL_URL not set — skipped");
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

        // information_schema.DATA_TYPE returns lowercase names
        let amount = schema.iter().find(|s| s.name == "amount").expect("amount missing");
        assert_eq!(amount.type_name, "decimal", "amount should be decimal, got '{}'", amount.type_name);

        // BOOLEAN is stored as tinyint in MySQL
        let flag = schema.iter().find(|s| s.name == "flag").expect("flag missing");
        assert_eq!(flag.type_name, "tinyint", "flag (BOOLEAN) should be tinyint, got '{}'", flag.type_name);

        drop_test_table(&c, &tbl).await;
        println!("  ✓ DECIMAL → 'decimal', BOOLEAN → 'tinyint'");
    }

    // ── fetch_all ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_fetch_all_column_and_row_count() {
        println!("\n[mysql] test_fetch_all_column_and_row_count");
        let Some(url) = mysql_url() else {
            println!("  MYSQL_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        create_test_table(&c, &tbl).await;

        c.execute(&format!(
            "INSERT INTO `{tbl}` (label, amount) VALUES ('alpha', 10.00), ('beta', 20.50), ('gamma', NULL)"
        )).await.unwrap();

        let result = c.fetch_all(&format!("SELECT * FROM `{tbl}` ORDER BY id")).await.expect("fetch_all failed");
        println!("  columns: {:?}", result.columns.iter().map(|c| &c.name).collect::<Vec<_>>());
        println!("  rows: {}", result.rows.len());

        assert_eq!(result.columns.len(), 7, "test table has 7 columns");
        assert_eq!(result.rows.len(), 3);
        assert_eq!(result.columns[0].name, "id");
        assert_eq!(result.columns[1].name, "label");

        drop_test_table(&c, &tbl).await;
        println!("  ✓ 7 columns, 3 rows");
    }

    #[tokio::test]
    async fn test_fetch_all_empty_result() {
        println!("\n[mysql] test_fetch_all_empty_result");
        let Some(url) = mysql_url() else {
            println!("  MYSQL_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        create_test_table(&c, &tbl).await;

        let result = c.fetch_all(&format!("SELECT * FROM `{tbl}` WHERE id = 9999")).await.expect("fetch_all failed");
        assert_eq!(result.rows.len(), 0);

        drop_test_table(&c, &tbl).await;
        println!("  ✓ empty result → 0 rows");
    }

    // ── execute ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_execute_insert_and_delete() {
        println!("\n[mysql] test_execute_insert_and_delete");
        let Some(url) = mysql_url() else {
            println!("  MYSQL_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        create_test_table(&c, &tbl).await;

        let n_ins = c.execute(&format!(
            "INSERT INTO `{tbl}` (label) VALUES ('row_a'), ('row_b')"
        )).await.expect("INSERT failed");
        println!("  INSERT rows_affected: {n_ins}");
        assert_eq!(n_ins, 2);

        let n_del = c.execute(&format!(
            "DELETE FROM `{tbl}` WHERE label IN ('row_a', 'row_b')"
        )).await.expect("DELETE failed");
        println!("  DELETE rows_affected: {n_del}");
        assert_eq!(n_del, 2);

        drop_test_table(&c, &tbl).await;
        println!("  ✓ INSERT → 2, DELETE → 2");
    }

    #[tokio::test]
    async fn test_execute_update_rows_affected() {
        println!("\n[mysql] test_execute_update_rows_affected");
        let Some(url) = mysql_url() else {
            println!("  MYSQL_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let tbl = unique_table();
        create_test_table(&c, &tbl).await;

        c.execute(&format!(
            "INSERT INTO `{tbl}` (label, flag) VALUES ('x', FALSE), ('y', FALSE), ('z', TRUE)"
        )).await.unwrap();

        let n = c.execute(&format!("UPDATE `{tbl}` SET flag = TRUE WHERE flag = FALSE")).await.expect("UPDATE failed");
        println!("  UPDATE rows_affected: {n}");
        assert_eq!(n, 2, "2 rows had flag=FALSE");

        drop_test_table(&c, &tbl).await;
        println!("  ✓ UPDATE → rows_affected=2");
    }

    #[tokio::test]
    async fn test_execute_invalid_sql() {
        println!("\n[mysql] test_execute_invalid_sql");
        let Some(url) = mysql_url() else {
            println!("  MYSQL_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let err = c.execute("THIS IS NOT VALID SQL").await.unwrap_err();
        assert!(matches!(err, DbError::QueryFailed(_)));
        println!("  ✓ invalid SQL → QueryFailed");
    }
}

fn normalize_ssl_mode(url: &str) -> std::borrow::Cow<'_, str> {
    let lower = url.to_ascii_lowercase();
    if let Some(pos) = lower.find("ssl-mode=") {
        let value_start = pos + "ssl-mode=".len();
        let value_end = lower[value_start..]
            .find(|c: char| !c.is_ascii_alphabetic())
            .map_or(url.len(), |i| value_start + i);
        if url[value_start..value_end].bytes().any(|b| b.is_ascii_uppercase()) {
            let mut result = url.to_string();
            result.replace_range(value_start..value_end, &url[value_start..value_end].to_ascii_lowercase());
            return std::borrow::Cow::Owned(result);
        }
    }
    std::borrow::Cow::Borrowed(url)
}

fn mysql_value(row: &MySqlRow, index: usize) -> Value {
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    let raw = row.try_get_raw(index).unwrap();
    if raw.is_null() {
        return Value::Null;
    }
    let tn = raw.type_info().name().to_string();
    let marker = || Value::Text(format!("<?{tn}>"));
    match tn.as_str() {
        "BOOLEAN" | "TINYINT(1)"              => row.try_get::<bool, _>(index).map(Value::Bool).unwrap_or_else(|_| marker()),
        // Signed integers — each width needs its own Rust type
        "TINYINT"                             => row.try_get::<i8, _>(index).map(|v| Value::Int(v as i64)).unwrap_or_else(|_| marker()),
        "SMALLINT"                            => row.try_get::<i16, _>(index).map(|v| Value::Int(v as i64)).unwrap_or_else(|_| marker()),
        "INT" | "MEDIUMINT"                   => row.try_get::<i32, _>(index).map(|v| Value::Int(v as i64)).unwrap_or_else(|_| marker()),
        "BIGINT"                              => row.try_get::<i64, _>(index).map(Value::Int).unwrap_or_else(|_| marker()),
        // Unsigned variants
        "TINYINT UNSIGNED"                    => row.try_get::<u8, _>(index).map(|v| Value::Int(v as i64)).unwrap_or_else(|_| marker()),
        "SMALLINT UNSIGNED"                   => row.try_get::<u16, _>(index).map(|v| Value::Int(v as i64)).unwrap_or_else(|_| marker()),
        "INT UNSIGNED" | "MEDIUMINT UNSIGNED" => row.try_get::<u32, _>(index).map(|v| Value::Int(v as i64)).unwrap_or_else(|_| marker()),
        "BIGINT UNSIGNED"                     => row.try_get::<u64, _>(index).map(|v| Value::Int(v as i64)).unwrap_or_else(|_| marker()),
        "FLOAT"                               => row.try_get::<f32, _>(index).map(|v| Value::Float(v as f64)).unwrap_or_else(|_| marker()),
        "DOUBLE"                              => row.try_get::<f64, _>(index).map(Value::Float).unwrap_or_else(|_| marker()),
        "DECIMAL"                             => row.try_get::<bigdecimal::BigDecimal, _>(index)
                                                    .map(|d| Value::Text(crate::db::types::format_decimal(d)))
                                                    .unwrap_or_else(|_| marker()),
        "BLOB" | "TINYBLOB" | "MEDIUMBLOB" | "LONGBLOB" | "BINARY" | "VARBINARY"
                                              => row.try_get::<Vec<u8>, _>(index).map(Value::Bytes).unwrap_or_else(|_| marker()),
        // Dates and times
        "DATE"                                => row.try_get::<NaiveDate, _>(index).map(|d| Value::Text(d.to_string())).unwrap_or_else(|_| marker()),
        "TIME"                                => row.try_get::<NaiveTime, _>(index).map(|t| Value::Text(t.to_string())).unwrap_or_else(|_| marker()),
        "DATETIME" | "TIMESTAMP"              => row.try_get::<NaiveDateTime, _>(index).map(|dt| Value::Text(dt.to_string())).unwrap_or_else(|_| marker()),
        // YEAR is a 1–4 digit year; MySQL sends it as u16
        "YEAR"                                => row.try_get::<u16, _>(index).map(|y| Value::Int(y as i64)).unwrap_or_else(|_| marker()),
        // JSON
        "JSON"                                => row.try_get::<serde_json::Value, _>(index).map(|v| Value::Text(v.to_string())).unwrap_or_else(|_| marker()),
        // ENUM and SET are text-compatible; fall through to the String arm below
        _ => row.try_get::<String, _>(index).map(Value::Text).unwrap_or_else(|_| marker()),
    }
}
