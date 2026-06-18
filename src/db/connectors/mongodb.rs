#![cfg(feature = "mongodb")]

use async_trait::async_trait;
use futures::stream::TryStreamExt;
use mongodb::{
    bson::{self, Bson, Document},
    options::{ClientOptions, FindOptions},
    Client,
};

use crate::db::error::DbError;
use crate::db::traits::NoSqlClient;
use crate::db::types::{Column, DbQueryResult, Row, TableKind, TableObject, Value};

pub struct MongoDbConnector {
    client: Option<Client>,
    db_name: Option<String>,
}

impl MongoDbConnector {
    pub fn new() -> Self {
        Self { client: None, db_name: None }
    }

    fn get_db(&self) -> Result<mongodb::Database, DbError> {
        let client = self.client.as_ref().ok_or(DbError::NotConnected)?;
        let name = self.db_name.as_deref().ok_or(DbError::NotConnected)?;
        Ok(client.database(name))
    }
}

#[async_trait]
impl NoSqlClient for MongoDbConnector {
    async fn connect(&mut self, url: &str) -> Result<(), DbError> {
        let opts = ClientOptions::parse(url)
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;

        // Extract database from URI path or reject
        let db_name = opts.default_database.clone().ok_or_else(|| {
            DbError::ConnectionFailed(
                "No database specified in URL — use mongodb://host:27017/dbname".to_string(),
            )
        })?;

        let client =
            Client::with_options(opts).map_err(|e| DbError::ConnectionFailed(e.to_string()))?;

        // Verify connectivity with a ping
        client
            .database(&db_name)
            .run_command(bson::doc! { "ping": 1 })
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;

        self.client = Some(client);
        self.db_name = Some(db_name);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DbError> {
        self.client = None;
        self.db_name = None;
        Ok(())
    }

    async fn list_collections(&self) -> Result<Vec<TableObject>, DbError> {
        let db = self.get_db()?;
        let names = db
            .list_collection_names()
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        let mut objects: Vec<TableObject> = names
            .into_iter()
            .map(|name| TableObject { name, kind: TableKind::Table })
            .collect();
        objects.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(objects)
    }

    async fn find(
        &self,
        collection: &str,
        filter: &str,
        limit: u64,
        offset: u64,
    ) -> Result<DbQueryResult, DbError> {
        let db = self.get_db()?;
        let coll = db.collection::<Document>(collection);

        let filter_doc = parse_filter(filter)?;

        let opts = FindOptions::builder()
            .limit(limit as i64)
            .skip(offset)
            .build();

        let cursor = coll
            .find(filter_doc)
            .with_options(opts)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let docs: Vec<Document> = cursor
            .try_collect()
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(docs_to_result(docs))
    }

    async fn aggregate(
        &self,
        collection: &str,
        pipeline: &str,
    ) -> Result<DbQueryResult, DbError> {
        let db = self.get_db()?;
        let coll = db.collection::<Document>(collection);

        let pipeline_docs = parse_pipeline(pipeline)?;

        let cursor = coll
            .aggregate(pipeline_docs)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let docs: Vec<Document> = cursor
            .try_collect()
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(docs_to_result(docs))
    }

    async fn count(&self, collection: &str, filter: &str) -> Result<u64, DbError> {
        let db = self.get_db()?;
        let coll = db.collection::<Document>(collection);
        let filter_doc = parse_filter(filter)?;
        coll.count_documents(filter_doc)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))
    }

    async fn insert_one(&self, collection: &str, doc_json: &str) -> Result<String, DbError> {
        let db = self.get_db()?;
        let coll = db.collection::<Document>(collection);
        let doc = parse_filter(doc_json)?;
        let result = coll
            .insert_one(doc)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(result.inserted_id.to_string())
    }

    async fn replace_one(&self, collection: &str, id: &str, doc_json: &str) -> Result<u64, DbError> {
        let db = self.get_db()?;
        let coll = db.collection::<Document>(collection);
        let filter = bson::doc! { "_id": id_to_bson(id) };
        let mut replacement = parse_filter(doc_json)?;
        replacement.remove("_id");
        let result = coll
            .replace_one(filter, replacement)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(result.matched_count)
    }

    async fn delete_one(&self, collection: &str, id: &str) -> Result<u64, DbError> {
        let db = self.get_db()?;
        let coll = db.collection::<Document>(collection);
        let filter = bson::doc! { "_id": id_to_bson(id) };
        let result = coll
            .delete_one(filter)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(result.deleted_count)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Reconstruct the correct BSON _id type from its string representation.
/// A 24-char hex string is treated as ObjectId; anything else as a plain string.
fn id_to_bson(id: &str) -> Bson {
    if let Ok(oid) = bson::oid::ObjectId::parse_str(id) {
        Bson::ObjectId(oid)
    } else {
        Bson::String(id.to_string())
    }
}

fn parse_filter(filter: &str) -> Result<Document, DbError> {
    let s = filter.trim();
    if s.is_empty() || s == "{}" {
        return Ok(bson::doc! {});
    }
    let json: serde_json::Value = serde_json::from_str(s)
        .map_err(|e| DbError::QueryFailed(format!("Invalid filter JSON: {e}")))?;
    let bson_val =
        bson::to_bson(&json).map_err(|e| DbError::QueryFailed(format!("BSON error: {e}")))?;
    bson_val
        .as_document()
        .cloned()
        .ok_or_else(|| DbError::QueryFailed("Filter must be a JSON object {{ }}".to_string()))
}

fn parse_pipeline(pipeline: &str) -> Result<Vec<Document>, DbError> {
    let s = pipeline.trim();
    let json: Vec<serde_json::Value> = serde_json::from_str(s)
        .map_err(|e| DbError::QueryFailed(format!("Invalid pipeline JSON: {e}")))?;
    json.iter()
        .map(|v| {
            let bson_val = bson::to_bson(v)
                .map_err(|e| DbError::QueryFailed(format!("BSON error: {e}")))?;
            bson_val
                .as_document()
                .cloned()
                .ok_or_else(|| DbError::QueryFailed("Pipeline stage must be a JSON object".to_string()))
        })
        .collect()
}

/// Convert a batch of BSON documents into a DbQueryResult.
/// Columns are built from the union of all top-level keys, `_id` first.
fn docs_to_result(docs: Vec<Document>) -> DbQueryResult {
    let mut col_names: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for doc in &docs {
        for key in doc.keys() {
            if seen.insert(key.clone()) {
                // Keep _id at front
                if key == "_id" {
                    col_names.insert(0, key.clone());
                } else {
                    col_names.push(key.clone());
                }
            }
        }
    }

    let columns: Vec<Column> = col_names
        .iter()
        .map(|name| Column { name: name.clone(), type_name: "bson".to_string() })
        .collect();

    let rows: Vec<Row> = docs
        .iter()
        .map(|doc| {
            let values = col_names
                .iter()
                .map(|col| doc.get(col).map(bson_to_value).unwrap_or(Value::Null))
                .collect();
            Row { values }
        })
        .collect();

    DbQueryResult { columns, rows, rows_affected: 0 }
}

fn bson_to_value(bson: &Bson) -> Value {
    match bson {
        Bson::Null | Bson::Undefined => Value::Null,
        Bson::Boolean(b) => Value::Bool(*b),
        Bson::Int32(i) => Value::Int(*i as i64),
        Bson::Int64(i) => Value::Int(*i),
        Bson::Double(f) => Value::Float(*f),
        Bson::String(s) => Value::Text(s.clone()),
        Bson::ObjectId(oid) => Value::Text(oid.to_string()),
        Bson::DateTime(dt) => Value::Text(dt.to_string()),
        Bson::Timestamp(ts) => Value::Text(format!("Timestamp({})", ts.time)),
        Bson::Binary(bin) => Value::Text(format!("<binary:{}>", bin.bytes.len())),
        Bson::Decimal128(d) => Value::Text(d.to_string()),
        Bson::Symbol(s) => Value::Text(s.clone()),
        Bson::JavaScriptCode(s) => Value::Text(s.clone()),
        Bson::Document(doc) => {
            let json = serde_json::to_string(doc).unwrap_or_else(|_| "{}".to_string());
            Value::NestedDoc(json)
        }
        Bson::Array(arr) => {
            let json = serde_json::to_string(arr).unwrap_or_else(|_| "[]".to_string());
            Value::NestedArray(json)
        }
        other => Value::Text(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{id_to_bson, parse_filter, parse_pipeline, MongoDbConnector};
    use crate::db::{error::DbError, traits::NoSqlClient, types::Value};
    use mongodb::bson::{self, Bson};
    use std::sync::atomic::{AtomicU32, Ordering};

    // ── helpers ──────────────────────────────────────────────────────────────────

    fn mongo_url() -> Option<String> {
        std::env::var("MONGODB_URL").ok()
    }

    fn unique_coll() -> String {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        format!("_rowdy_mg_test_{}", COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    async fn connected(url: &str) -> MongoDbConnector {
        let mut c = MongoDbConnector::new();
        c.connect(url).await.expect("connect failed");
        println!("  [mongodb] connected");
        c
    }

    async fn drop_coll(c: &MongoDbConnector, coll: &str) {
        if let Ok(db) = c.get_db() {
            db.collection::<bson::Document>(coll).drop().await.ok();
        }
    }

    // ── parse_filter (pure) ──────────────────────────────────────────────────────

    #[test]
    fn test_parse_filter_empty_string() {
        let doc = parse_filter("").unwrap();
        assert!(doc.is_empty());
        println!("  ✓ empty string → empty doc");
    }

    #[test]
    fn test_parse_filter_empty_braces() {
        let doc = parse_filter("{}").unwrap();
        assert!(doc.is_empty());
        println!("  ✓ {{}} → empty doc");
    }

    #[test]
    fn test_parse_filter_with_fields() {
        let doc = parse_filter(r#"{"name": "Alice", "age": 30}"#).unwrap();
        assert_eq!(doc.get_str("name").unwrap(), "Alice");
        // serde_json integers deserialize as i64 → Bson::Int64
        assert_eq!(doc.get_i64("age").unwrap(), 30);
        println!("  ✓ JSON object → doc with correct fields");
    }

    #[test]
    fn test_parse_filter_invalid_json() {
        let e = parse_filter("not json!!!").unwrap_err();
        assert!(matches!(e, DbError::QueryFailed(_)));
        println!("  ✓ invalid JSON → QueryFailed");
    }

    // ── parse_pipeline (pure) ────────────────────────────────────────────────────

    #[test]
    fn test_parse_pipeline_valid() {
        let stages = parse_pipeline(
            r#"[{"$match": {"active": true}}, {"$limit": 10}]"#,
        ).unwrap();
        assert_eq!(stages.len(), 2);
        assert!(stages[0].contains_key("$match"));
        assert!(stages[1].contains_key("$limit"));
        println!("  ✓ pipeline JSON → 2 stages");
    }

    #[test]
    fn test_parse_pipeline_invalid_json() {
        let e = parse_pipeline("not an array").unwrap_err();
        assert!(matches!(e, DbError::QueryFailed(_)));
        println!("  ✓ invalid pipeline JSON → QueryFailed");
    }

    // ── id_to_bson (pure) ────────────────────────────────────────────────────────

    #[test]
    fn test_id_to_bson_objectid() {
        let bson = id_to_bson("507f1f77bcf86cd799439011");
        assert!(matches!(bson, Bson::ObjectId(_)));
        println!("  ✓ 24-char hex → ObjectId");
    }

    #[test]
    fn test_id_to_bson_string() {
        let bson = id_to_bson("my-custom-id");
        assert!(matches!(bson, Bson::String(ref s) if s == "my-custom-id"));
        println!("  ✓ non-hex id → String");
    }

    // ── connection ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_connect() {
        println!("\n[mongodb] test_connect");
        let Some(url) = mongo_url() else {
            println!("  MONGODB_URL not set — skipped");
            return;
        };
        let mut c = MongoDbConnector::new();
        if let Err(e) = c.connect(&url).await {
            println!("  connect error: {e}");
            panic!("connect failed: {e}");
        }
        println!("  ✓ connection OK");
    }

    #[tokio::test]
    async fn test_connect_missing_db_name() {
        println!("\n[mongodb] test_connect_missing_db_name");
        let Some(url) = mongo_url() else {
            println!("  MONGODB_URL not set — skipped");
            return;
        };
        // Strip the database path from the URL
        let base = url.split('/').take(3).collect::<Vec<_>>().join("/");
        let mut c = MongoDbConnector::new();
        let e = c.connect(&base).await.unwrap_err();
        assert!(matches!(e, DbError::ConnectionFailed(_)));
        println!("  ✓ URL without DB name → ConnectionFailed");
    }

    #[tokio::test]
    async fn test_not_connected_returns_error() {
        println!("\n[mongodb] test_not_connected_returns_error");
        let c = MongoDbConnector::new();
        assert!(matches!(c.list_collections().await.unwrap_err(), DbError::NotConnected));
        assert!(matches!(c.find("x", "{}", 10, 0).await.unwrap_err(), DbError::NotConnected));
        assert!(matches!(c.count("x", "{}").await.unwrap_err(), DbError::NotConnected));
        assert!(matches!(c.insert_one("x", "{}").await.unwrap_err(), DbError::NotConnected));
        println!("  ✓ all NoSqlClient methods before connect → NotConnected");
    }

    #[tokio::test]
    async fn test_disconnect() {
        println!("\n[mongodb] test_disconnect");
        let Some(url) = mongo_url() else {
            println!("  MONGODB_URL not set — skipped");
            return;
        };
        let mut c = connected(&url).await;
        assert!(c.disconnect().await.is_ok());
        assert!(matches!(c.list_collections().await.unwrap_err(), DbError::NotConnected));
        println!("  ✓ disconnect OK, subsequent list_collections → NotConnected");
    }

    // ── insert_one / find / count ─────────────────────────────────────────────────

    #[tokio::test]
    async fn test_insert_one_returns_id() {
        println!("\n[mongodb] test_insert_one_returns_id");
        let Some(url) = mongo_url() else {
            println!("  MONGODB_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let coll = unique_coll();

        let id = c.insert_one(&coll, r#"{"name": "Alice", "age": 30}"#).await.expect("insert_one failed");
        println!("  inserted _id = {id}");
        assert!(!id.is_empty());

        drop_coll(&c, &coll).await;
        println!("  ✓ insert_one returns non-empty id");
    }

    #[tokio::test]
    async fn test_find_empty_collection() {
        println!("\n[mongodb] test_find_empty_collection");
        let Some(url) = mongo_url() else {
            println!("  MONGODB_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let coll = unique_coll();

        // collection is implicitly created empty on first insert; find on nonexistent → 0 rows
        let r = c.find(&coll, "{}", 10, 0).await.expect("find failed");
        println!("  rows={}", r.rows.len());
        assert_eq!(r.rows.len(), 0);
        println!("  ✓ find on empty collection → 0 rows");
    }

    #[tokio::test]
    async fn test_find_column_and_row_count() {
        println!("\n[mongodb] test_find_column_and_row_count");
        let Some(url) = mongo_url() else {
            println!("  MONGODB_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let coll = unique_coll();

        c.insert_one(&coll, r#"{"title": "1984", "author": "Orwell", "year": 1949}"#).await.unwrap();
        c.insert_one(&coll, r#"{"title": "The Plague", "author": "Camus", "year": 1947}"#).await.unwrap();
        c.insert_one(&coll, r#"{"title": "The Trial", "author": "Kafka", "year": 1925}"#).await.unwrap();

        let r = c.find(&coll, "{}", 10, 0).await.expect("find failed");
        println!("  cols={} rows={}", r.columns.len(), r.rows.len());
        // _id + title + author + year = 4 columns
        assert_eq!(r.columns.len(), 4);
        assert_eq!(r.rows.len(), 3);

        drop_coll(&c, &coll).await;
        println!("  ✓ 4 columns (_id included), 3 rows");
    }

    #[tokio::test]
    async fn test_find_with_filter() {
        println!("\n[mongodb] test_find_with_filter");
        let Some(url) = mongo_url() else {
            println!("  MONGODB_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let coll = unique_coll();

        c.insert_one(&coll, r#"{"name": "Alice", "active": true}"#).await.unwrap();
        c.insert_one(&coll, r#"{"name": "Bob", "active": false}"#).await.unwrap();
        c.insert_one(&coll, r#"{"name": "Carol", "active": true}"#).await.unwrap();

        let r = c.find(&coll, r#"{"active": true}"#, 10, 0).await.expect("find failed");
        println!("  filter active=true → {} rows", r.rows.len());
        assert_eq!(r.rows.len(), 2);

        drop_coll(&c, &coll).await;
        println!("  ✓ filter {{active: true}} → 2 rows");
    }

    #[tokio::test]
    async fn test_find_limit_and_offset() {
        println!("\n[mongodb] test_find_limit_and_offset");
        let Some(url) = mongo_url() else {
            println!("  MONGODB_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let coll = unique_coll();

        for i in 0..5 {
            c.insert_one(&coll, &format!(r#"{{"n": {i}}}"#)).await.unwrap();
        }

        let page1 = c.find(&coll, "{}", 2, 0).await.expect("find page1 failed");
        let page2 = c.find(&coll, "{}", 2, 2).await.expect("find page2 failed");
        let page3 = c.find(&coll, "{}", 2, 4).await.expect("find page3 failed");
        println!("  page1={} page2={} page3={}", page1.rows.len(), page2.rows.len(), page3.rows.len());
        assert_eq!(page1.rows.len(), 2);
        assert_eq!(page2.rows.len(), 2);
        assert_eq!(page3.rows.len(), 1);

        drop_coll(&c, &coll).await;
        println!("  ✓ limit/offset pagination correct");
    }

    #[tokio::test]
    async fn test_count() {
        println!("\n[mongodb] test_count");
        let Some(url) = mongo_url() else {
            println!("  MONGODB_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let coll = unique_coll();

        c.insert_one(&coll, r#"{"x": 1}"#).await.unwrap();
        c.insert_one(&coll, r#"{"x": 2}"#).await.unwrap();

        let total = c.count(&coll, "{}").await.expect("count failed");
        println!("  count = {total}");
        assert_eq!(total, 2);

        drop_coll(&c, &coll).await;
        println!("  ✓ count → 2");
    }

    // ── replace_one / delete_one ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_replace_one() {
        println!("\n[mongodb] test_replace_one");
        let Some(url) = mongo_url() else {
            println!("  MONGODB_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let coll = unique_coll();

        let id = c.insert_one(&coll, r#"{"val": "before"}"#).await.unwrap();
        // id_to_bson works with ObjectId hex — strip ObjectId("...") wrapper if present
        let raw_id = id.trim_start_matches("ObjectId(\"").trim_end_matches("\")");
        let matched = c.replace_one(&coll, raw_id, r#"{"val": "after"}"#).await.expect("replace_one failed");
        println!("  replace_one matched = {matched}");
        assert_eq!(matched, 1);

        let r = c.find(&coll, "{}", 10, 0).await.unwrap();
        let val = r.rows[0].values.iter().find(|v| matches!(v, Value::Text(s) if s == "after"));
        assert!(val.is_some(), "updated value not found");

        drop_coll(&c, &coll).await;
        println!("  ✓ replace_one → matched 1, value updated");
    }

    #[tokio::test]
    async fn test_delete_one() {
        println!("\n[mongodb] test_delete_one");
        let Some(url) = mongo_url() else {
            println!("  MONGODB_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let coll = unique_coll();

        let id = c.insert_one(&coll, r#"{"val": "to-delete"}"#).await.unwrap();
        c.insert_one(&coll, r#"{"val": "keep"}"#).await.unwrap();

        let raw_id = id.trim_start_matches("ObjectId(\"").trim_end_matches("\")");
        let deleted = c.delete_one(&coll, raw_id).await.expect("delete_one failed");
        println!("  delete_one deleted = {deleted}");
        assert_eq!(deleted, 1);
        assert_eq!(c.count(&coll, "{}").await.unwrap(), 1);

        drop_coll(&c, &coll).await;
        println!("  ✓ delete_one → deleted 1, 1 remaining");
    }

    // ── aggregate ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_aggregate() {
        println!("\n[mongodb] test_aggregate");
        let Some(url) = mongo_url() else {
            println!("  MONGODB_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let coll = unique_coll();

        c.insert_one(&coll, r#"{"cat": "A", "val": 10}"#).await.unwrap();
        c.insert_one(&coll, r#"{"cat": "B", "val": 20}"#).await.unwrap();
        c.insert_one(&coll, r#"{"cat": "A", "val": 30}"#).await.unwrap();

        let r = c.aggregate(
            &coll,
            r#"[{"$match": {"cat": "A"}}, {"$count": "total"}]"#,
        ).await.expect("aggregate failed");
        println!("  aggregate result: cols={} rows={}", r.columns.len(), r.rows.len());
        assert_eq!(r.rows.len(), 1);
        // The $count stage returns a document with field "total" = 2
        let total_val = &r.rows[0].values[0];
        println!("  total = {:?}", total_val);
        assert!(matches!(total_val, Value::Int(2)));

        drop_coll(&c, &coll).await;
        println!("  ✓ aggregate $match+$count → 2");
    }

    // ── list_collections ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_collections() {
        println!("\n[mongodb] test_list_collections");
        let Some(url) = mongo_url() else {
            println!("  MONGODB_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let coll1 = unique_coll();
        let coll2 = unique_coll();

        c.insert_one(&coll1, r#"{"x": 1}"#).await.unwrap();
        c.insert_one(&coll2, r#"{"x": 2}"#).await.unwrap();

        let objects = c.list_collections().await.expect("list_collections failed");
        println!("  found {} collections", objects.len());
        let names: Vec<&str> = objects.iter().map(|o| o.name.as_str()).collect();
        assert!(names.contains(&coll1.as_str()), "{coll1} not in list");
        assert!(names.contains(&coll2.as_str()), "{coll2} not in list");

        drop_coll(&c, &coll1).await;
        drop_coll(&c, &coll2).await;
        println!("  ✓ both test collections visible in list");
    }
}
