use async_trait::async_trait;
use sqlx::{
    postgres::{PgPool, PgRow},
    Column as SqlxColumn, Row as SqlxRow, TypeInfo, ValueRef,
};
use crate::db::error::DbError;
use crate::db::traits::SqlClient;
use crate::db::types::{Column, ColumnSchema, DbQueryResult, ForeignKey, Row, Value};

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

    async fn get_schema(&self, table: &str) -> Result<Vec<ColumnSchema>, DbError> {
        let pool = self.pool()?;
        let safe = table.replace('\'', "");
        let query = format!(
            r#"
            SELECT
                c.column_name,
                c.data_type,
                c.is_nullable,
                COALESCE(pk.is_pk, false) AS is_pk,
                fk.foreign_table_name,
                fk.foreign_column_name
            FROM information_schema.columns c
            LEFT JOIN (
                SELECT kcu.column_name, true AS is_pk
                FROM information_schema.table_constraints tc
                JOIN information_schema.key_column_usage kcu
                    ON tc.constraint_name = kcu.constraint_name
                    AND tc.table_schema = kcu.table_schema
                    AND tc.table_name = kcu.table_name
                WHERE tc.table_schema = 'public' AND tc.table_name = '{safe}'
                  AND tc.constraint_type = 'PRIMARY KEY'
            ) pk ON pk.column_name = c.column_name
            LEFT JOIN (
                SELECT kcu.column_name,
                       ccu.table_name AS foreign_table_name,
                       ccu.column_name AS foreign_column_name
                FROM information_schema.table_constraints tc
                JOIN information_schema.key_column_usage kcu
                    ON tc.constraint_name = kcu.constraint_name
                    AND tc.table_schema = kcu.table_schema
                    AND tc.table_name = kcu.table_name
                JOIN information_schema.constraint_column_usage ccu
                    ON tc.constraint_name = ccu.constraint_name
                    AND tc.table_schema = ccu.table_schema
                WHERE tc.table_schema = 'public' AND tc.table_name = '{safe}'
                  AND tc.constraint_type = 'FOREIGN KEY'
            ) fk ON fk.column_name = c.column_name
            WHERE c.table_schema = 'public' AND c.table_name = '{safe}'
            ORDER BY c.ordinal_position
            "#
        );

        let rows: Vec<PgRow> = sqlx::query(&query)
            .fetch_all(pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut schema = vec![];
        for row in &rows {
            let name: String = row.try_get("column_name").unwrap_or_default();
            let type_name: String = row.try_get("data_type").unwrap_or_default();
            let is_nullable: String = row.try_get("is_nullable").unwrap_or_else(|_| "YES".into());
            let is_pk: bool = row.try_get("is_pk").unwrap_or(false);
            let fk_table: Option<String> = row.try_get::<Option<String>, _>("foreign_table_name").unwrap_or(None);
            let fk_col: Option<String> = row.try_get::<Option<String>, _>("foreign_column_name").unwrap_or(None);
            let fk = match (fk_table, fk_col) {
                (Some(t), Some(c)) if !t.is_empty() => Some(ForeignKey { table: t, column: c }),
                _ => None,
            };
            schema.push(ColumnSchema { name, type_name, is_pk, is_nullable: is_nullable == "YES", fk });
        }
        Ok(schema)
    }
}

fn pg_value(row: &PgRow, index: usize) -> Value {
    let raw = row.try_get_raw(index).unwrap();
    if raw.is_null() {
        return Value::Null;
    }
    match raw.type_info().name() {
        "BOOL"    => row.try_get::<bool, _>(index).map(Value::Bool).unwrap_or(Value::Null),
        // Each integer width requires its own Rust type in sqlx
        "INT2"    => row.try_get::<i16, _>(index).map(|v| Value::Int(v as i64)).unwrap_or(Value::Null),
        "INT4"    => row.try_get::<i32, _>(index).map(|v| Value::Int(v as i64)).unwrap_or(Value::Null),
        "INT8"    => row.try_get::<i64, _>(index).map(Value::Int).unwrap_or(Value::Null),
        "FLOAT4"  => row.try_get::<f32, _>(index).map(|v| Value::Float(v as f64)).unwrap_or(Value::Null),
        "FLOAT8"  => row.try_get::<f64, _>(index).map(Value::Float).unwrap_or(Value::Null),
        // NUMERIC: try String (text protocol) then parse to f64; keep as Text if not parseable
        "NUMERIC" => row.try_get::<String, _>(index).ok()
            .map(|s| s.parse::<f64>().map(Value::Float).unwrap_or_else(|_| Value::Text(s)))
            .unwrap_or(Value::Null),
        "BYTEA"   => row.try_get::<Vec<u8>, _>(index).map(Value::Bytes).unwrap_or(Value::Null),
        _ => row.try_get::<String, _>(index).map(Value::Text).unwrap_or(Value::Null),
    }
}
