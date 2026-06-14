use async_trait::async_trait;
use sqlx::{
    sqlite::{SqlitePool, SqliteRow},
    Column as SqlxColumn, Row as SqlxRow, TypeInfo, ValueRef,
};
use crate::db::error::DbError;
use crate::db::traits::SqlClient;
use crate::db::types::{Column, ColumnSchema, DbQueryResult, ForeignKey, Row, Value};

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
