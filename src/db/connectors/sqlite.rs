use async_trait::async_trait;
use sqlx::{
    sqlite::{SqlitePool, SqliteRow},
    Column as SqlxColumn, Row as SqlxRow, TypeInfo, ValueRef,
};
use crate::db::error::DbError;
use crate::db::traits::SqlClient;
use crate::db::types::{Column, ColumnSchema, DbQueryResult, ForeignKey, Row, TableKind, TableObject, Value};

pub struct SqliteConnector {
    pool: Option<SqlitePool>,
}

impl SqliteConnector {
    pub fn new() -> Self {
        Self { pool: None }
    }

    fn pool(&self) -> Result<&SqlitePool, DbError> {
        self.pool.as_ref().ok_or(DbError::NotConnected)
    }
}

#[async_trait]
impl SqlClient for SqliteConnector {
    async fn connect(&mut self, url: &str) -> Result<(), DbError> {
        let pool = SqlitePool::connect(url)
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
        let rows: Vec<SqliteRow> = sqlx::query(query)
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
                values: (0..r.len()).map(|i| sqlite_value(r, i)).collect(),
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
        let rows: Vec<SqliteRow> =
            sqlx::query("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .fetch_all(self.pool()?)
                .await
                .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| r.try_get::<String, _>(0).unwrap_or_default())
            .collect())
    }

    async fn get_table_objects(&self) -> Result<Vec<TableObject>, DbError> {
        let rows: Vec<SqliteRow> = sqlx::query(
            "SELECT name, type FROM sqlite_master WHERE type IN ('table', 'view') ORDER BY name",
        )
        .fetch_all(self.pool()?)
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(rows.iter().map(|r| {
            let name = r.try_get::<String, _>(0).unwrap_or_default();
            let type_str = r.try_get::<String, _>(1).unwrap_or_default();
            let kind = if type_str == "view" { TableKind::View } else { TableKind::Table };
            TableObject { name, kind }
        }).collect())
    }

    async fn get_schema(&self, table: &str) -> Result<Vec<ColumnSchema>, DbError> {
        use std::collections::HashMap;
        let pool = self.pool()?;
        let safe = table.replace('"', "");

        let info_rows: Vec<SqliteRow> = sqlx::query(&format!("PRAGMA table_info(\"{}\")", safe))
            .fetch_all(pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let fk_rows: Vec<SqliteRow> = sqlx::query(&format!("PRAGMA foreign_key_list(\"{}\")", safe))
            .fetch_all(pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut fk_map: HashMap<String, ForeignKey> = HashMap::new();
        for row in &fk_rows {
            let from: String = row.try_get("from").unwrap_or_default();
            let to_table: String = row.try_get("table").unwrap_or_default();
            let to_col: String = row.try_get("to").unwrap_or_default();
            fk_map.insert(from, ForeignKey { table: to_table, column: to_col });
        }

        let mut schema = vec![];
        for row in &info_rows {
            let name: String = row.try_get("name").unwrap_or_default();
            let type_name: String = row.try_get("type").unwrap_or_default();
            let notnull: i64 = row.try_get("notnull").unwrap_or(0);
            let pk: i64 = row.try_get("pk").unwrap_or(0);
            let fk = fk_map.get(&name).cloned();
            schema.push(ColumnSchema { name, type_name, is_pk: pk > 0, is_nullable: notnull == 0, fk });
        }
        Ok(schema)
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteConnector;
    use crate::db::{
        error::DbError,
        traits::SqlClient,
        types::{TableKind, Value},
    };

    // ── helpers ─────────────────────────────────────────────────────────────────

    /// Opens a fresh in-memory SQLite database.
    async fn connected() -> SqliteConnector {
        let mut c = SqliteConnector::new();
        c.connect("sqlite::memory:")
            .await
            .expect("connect to :memory: failed");
        println!("  [sqlite] connected to :memory:");
        c
    }

    /// Creates a minimal schema + 3 authors, 3 books, 1 view.
    async fn seeded() -> SqliteConnector {
        let c = connected().await;
        c.execute(
            "CREATE TABLE authors (
                id   INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL
            )",
        ).await.unwrap();
        c.execute(
            "CREATE TABLE books (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                title     TEXT    NOT NULL,
                author_id INTEGER REFERENCES authors(id),
                price     REAL,
                available INTEGER NOT NULL DEFAULT 1
            )",
        ).await.unwrap();
        c.execute(
            "CREATE VIEW v_available AS SELECT * FROM books WHERE available = 1",
        ).await.unwrap();
        c.execute(
            "INSERT INTO authors (name) VALUES ('George Orwell'), ('Albert Camus'), ('Franz Kafka')",
        ).await.unwrap();
        c.execute(
            "INSERT INTO books (title, author_id, price, available) VALUES
                ('1984',       1,    9.99,  1),
                ('The Plague', 2,   12.50,  1),
                ('The Trial',  3,    NULL,  0)",
        ).await.unwrap();
        println!("  [sqlite] seeded: authors(3) books(3) view(v_available)");
        c
    }

    // ── connection ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_connect_memory() {
        println!("\n[sqlite] test_connect_memory");
        let mut c = SqliteConnector::new();
        let result = c.connect("sqlite::memory:").await;
        assert!(result.is_ok(), "connect should succeed: {:?}", result);
        println!("  ✓ connection OK");
    }

    #[tokio::test]
    async fn test_not_connected_returns_error() {
        println!("\n[sqlite] test_not_connected_returns_error");
        let c = SqliteConnector::new();

        let e1 = c.fetch_all("SELECT 1").await.unwrap_err();
        assert!(matches!(e1, DbError::NotConnected), "expected NotConnected, got {:?}", e1);
        println!("  ✓ fetch_all before connect → NotConnected");

        let e2 = c.execute("CREATE TABLE t (x INT)").await.unwrap_err();
        assert!(matches!(e2, DbError::NotConnected), "expected NotConnected, got {:?}", e2);
        println!("  ✓ execute before connect → NotConnected");
    }

    #[tokio::test]
    async fn test_disconnect() {
        println!("\n[sqlite] test_disconnect");
        let mut c = connected().await;
        assert!(c.disconnect().await.is_ok(), "disconnect should not error");
        println!("  ✓ disconnect OK");
    }

    // ── schema ───────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_table_objects_kinds() {
        println!("\n[sqlite] test_get_table_objects_kinds");
        let c = seeded().await;
        let objects = c.get_table_objects().await.expect("get_table_objects failed");
        println!("  objects: {:?}", objects.iter().map(|o| (&o.name, &o.kind)).collect::<Vec<_>>());

        let authors = objects.iter().find(|o| o.name == "authors").expect("authors missing");
        assert_eq!(authors.kind, TableKind::Table);

        let books = objects.iter().find(|o| o.name == "books").expect("books missing");
        assert_eq!(books.kind, TableKind::Table);

        let view = objects.iter().find(|o| o.name == "v_available").expect("v_available missing");
        assert_eq!(view.kind, TableKind::View, "v_available should be a View");

        println!("  ✓ tables → Table, view → View");
    }

    #[tokio::test]
    async fn test_get_schema_pk() {
        println!("\n[sqlite] test_get_schema_pk");
        let c = seeded().await;
        let schema = c.get_schema("authors").await.expect("get_schema failed");
        println!("  schema: {:?}", schema.iter().map(|s| (&s.name, s.is_pk)).collect::<Vec<_>>());

        let id = schema.iter().find(|s| s.name == "id").expect("id missing");
        assert!(id.is_pk, "id should be PK");

        let name = schema.iter().find(|s| s.name == "name").expect("name missing");
        assert!(!name.is_pk, "name should not be PK");

        println!("  ✓ id → is_pk=true, name → is_pk=false");
    }

    #[tokio::test]
    async fn test_get_schema_fk() {
        println!("\n[sqlite] test_get_schema_fk");
        let c = seeded().await;
        let schema = c.get_schema("books").await.expect("get_schema failed");
        let author_id = schema.iter().find(|s| s.name == "author_id").expect("author_id missing");
        println!("  author_id FK: {:?}", author_id.fk);

        let fk = author_id.fk.as_ref().expect("author_id should have a FK");
        assert_eq!(fk.table, "authors", "FK should point to authors");
        assert_eq!(fk.column, "id", "FK should point to id column");

        println!("  ✓ author_id → FK(authors.id)");
    }

    #[tokio::test]
    async fn test_get_schema_no_fk_on_plain_column() {
        println!("\n[sqlite] test_get_schema_no_fk_on_plain_column");
        let c = seeded().await;
        let schema = c.get_schema("books").await.expect("get_schema failed");
        let title = schema.iter().find(|s| s.name == "title").expect("title missing");
        assert!(title.fk.is_none(), "title should have no FK");
        println!("  ✓ title → FK=None");
    }

    // ── fetch_all ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_fetch_all_column_and_row_count() {
        println!("\n[sqlite] test_fetch_all_column_and_row_count");
        let c = seeded().await;
        let result = c.fetch_all("SELECT * FROM authors ORDER BY id").await.expect("fetch_all failed");
        println!("  columns: {:?}", result.columns.iter().map(|c| &c.name).collect::<Vec<_>>());
        println!("  rows: {}", result.rows.len());

        assert_eq!(result.columns.len(), 2, "authors has 2 columns");
        assert_eq!(result.rows.len(), 3, "3 authors were inserted");
        assert_eq!(result.columns[0].name, "id");
        assert_eq!(result.columns[1].name, "name");

        println!("  ✓ 2 columns, 3 rows");
    }

    #[tokio::test]
    async fn test_fetch_all_type_mapping() {
        println!("\n[sqlite] test_fetch_all_type_mapping");
        let c = seeded().await;
        let result = c
            .fetch_all("SELECT id, title, price, available FROM books ORDER BY id")
            .await
            .expect("fetch_all failed");

        let row0 = &result.rows[0]; // 1984 — id=1, price=9.99, available=1
        let row2 = &result.rows[2]; // The Trial — price=NULL, available=0
        println!("  row[0] (1984):      {:?}", row0.values);
        println!("  row[2] (The Trial): {:?}", row2.values);

        assert!(matches!(row0.values[0], Value::Int(1)),   "id should be Int(1)");
        assert!(matches!(row0.values[1], Value::Text(_)),  "title should be Text");
        assert!(matches!(row0.values[2], Value::Float(_)), "price 9.99 should be Float");
        assert!(matches!(row0.values[3], Value::Int(1)),   "available=1 should be Int(1)");
        assert!(matches!(row2.values[2], Value::Null),     "NULL price should be Null");

        println!("  ✓ Int, Text, Float, Null all decoded correctly");
    }

    #[tokio::test]
    async fn test_fetch_all_empty_result() {
        println!("\n[sqlite] test_fetch_all_empty_result");
        let c = seeded().await;
        let result = c
            .fetch_all("SELECT * FROM authors WHERE id = 9999")
            .await
            .expect("fetch_all failed");
        println!("  columns: {}, rows: {}", result.columns.len(), result.rows.len());
        assert_eq!(result.rows.len(), 0, "no rows for non-existent id");
        println!("  ✓ empty result → 0 rows, 0 columns");
    }

    // ── execute ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_execute_insert_rows_affected() {
        println!("\n[sqlite] test_execute_insert_rows_affected");
        let c = seeded().await;
        let n = c
            .execute("INSERT INTO authors (name) VALUES ('New Author')")
            .await
            .expect("execute INSERT failed");
        println!("  rows_affected: {n}");
        assert_eq!(n, 1);
        println!("  ✓ INSERT → rows_affected=1");
    }

    #[tokio::test]
    async fn test_execute_update_rows_affected() {
        println!("\n[sqlite] test_execute_update_rows_affected");
        let c = seeded().await;
        let n = c
            .execute("UPDATE books SET price = 7.99 WHERE available = 1")
            .await
            .expect("execute UPDATE failed");
        println!("  rows_affected: {n}");
        assert_eq!(n, 2, "2 available books should be updated");
        println!("  ✓ UPDATE → rows_affected=2");
    }

    #[tokio::test]
    async fn test_execute_delete_rows_affected() {
        println!("\n[sqlite] test_execute_delete_rows_affected");
        let c = seeded().await;
        let n = c
            .execute("DELETE FROM books WHERE available = 0")
            .await
            .expect("execute DELETE failed");
        println!("  rows_affected: {n}");
        assert_eq!(n, 1, "1 unavailable book should be deleted");
        println!("  ✓ DELETE → rows_affected=1");
    }

    #[tokio::test]
    async fn test_execute_invalid_sql() {
        println!("\n[sqlite] test_execute_invalid_sql");
        let c = connected().await;
        let err = c.execute("THIS IS NOT VALID SQL").await.unwrap_err();
        assert!(matches!(err, DbError::QueryFailed(_)), "expected QueryFailed, got {:?}", err);
        println!("  ✓ invalid SQL → QueryFailed");
    }
}

fn sqlite_value(row: &SqliteRow, index: usize) -> Value {
    let raw = row.try_get_raw(index).unwrap();
    if raw.is_null() {
        return Value::Null;
    }
    // SQLite has dynamic typing: the declared column type is advisory, not enforced.
    // Try each storage class in priority order instead of matching on type names.
    if let Ok(v) = row.try_get::<i64, _>(index)     { return Value::Int(v);   }
    if let Ok(v) = row.try_get::<f64, _>(index)     { return Value::Float(v); }
    if let Ok(v) = row.try_get::<String, _>(index)  { return Value::Text(v);  }
    if let Ok(v) = row.try_get::<Vec<u8>, _>(index) { return Value::Bytes(v); }
    let tn = raw.type_info().name().to_string();
    Value::Text(format!("<?{tn}>"))
}
