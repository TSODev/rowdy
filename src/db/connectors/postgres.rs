use async_trait::async_trait;
use sqlx::{
    postgres::{PgPool, PgRow},
    Column as SqlxColumn, Row as SqlxRow, TypeInfo, ValueRef,
};
use crate::db::error::DbError;
use crate::db::traits::SqlClient;
use crate::db::types::{Column, DbQueryResult, Row, Value};

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
}

fn pg_value(row: &PgRow, index: usize) -> Value {
    let raw = row.try_get_raw(index).unwrap();
    if raw.is_null() {
        return Value::Null;
    }
    let type_info = raw.type_info();
    match type_info.name() {
        "BOOL"                              => row.try_get::<bool, _>(index).map(Value::Bool).unwrap_or(Value::Null),
        "INT2" | "INT4" | "INT8"           => row.try_get::<i64, _>(index).map(Value::Int).unwrap_or(Value::Null),
        "FLOAT4" | "FLOAT8" | "NUMERIC"    => row.try_get::<f64, _>(index).map(Value::Float).unwrap_or(Value::Null),
        "BYTEA"                             => row.try_get::<Vec<u8>, _>(index).map(Value::Bytes).unwrap_or(Value::Null),
        _ => row.try_get::<String, _>(index).map(Value::Text).unwrap_or(Value::Null),
    }
}
