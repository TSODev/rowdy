use crossterm::event::KeyEvent;
use crate::db::error::DbError;
use crate::db::types::DbQueryResult;

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    DbResult(DbQueryResult),
    DbError(DbError),
    Tick,
    Quit,
}
