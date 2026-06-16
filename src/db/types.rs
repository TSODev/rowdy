#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    #[allow(dead_code)]
    pub type_name: String,
}

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct Row {
    pub values: Vec<Value>,
}

#[derive(Debug, Clone)]
pub struct DbQueryResult {
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    #[allow(dead_code)]
    pub rows_affected: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TableKind {
    Table,
    View,
}

#[derive(Debug, Clone)]
pub struct TableObject {
    pub name: String,
    pub kind: TableKind,
}

#[derive(Debug, Clone)]
pub struct ForeignKey {
    pub table: String,
    pub column: String,
}

#[derive(Debug, Clone)]
pub struct ColumnSchema {
    pub name: String,
    pub type_name: String,
    pub is_pk: bool,
    #[allow(dead_code)]
    pub is_nullable: bool,
    pub fk: Option<ForeignKey>,
}

/// Typed value of a Redis key, fetched via TYPE + the appropriate read command.
#[derive(Debug, Clone)]
pub enum KvKeyDetail {
    String(String),
    Hash(Vec<(String, String)>),   // (field, value) sorted by field
    List(Vec<String>),              // ordered values
    Set(Vec<String>),               // unordered members
    ZSet(Vec<(String, f64)>),      // (member, score) ordered by score
}
