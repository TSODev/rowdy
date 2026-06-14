#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
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
    pub rows_affected: u64,
}
