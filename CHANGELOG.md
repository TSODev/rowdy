# Changelog

All notable changes to Rowdy are documented here.  
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased]

### In progress
- Data Grid screen (paginated display of `DbQueryResult`)
- Inline cell editing
- Multi-line SQL editor (`tui-textarea` + syntax highlighting)
- Status bar component
- Confirmation / error modal

---

## [0.1.0] — 2026-06-14

### Added

#### Infrastructure
- Initial project boilerplate with `ratatui` + `crossterm` + `tokio` + `sqlx`
- Crate published as `rowdy-db` on crates.io (name conflict with `rowdy`)
- Dual license: MIT OR Apache-2.0
- `~/.config/rowdy/config.toml` support for saved connection profiles

#### Database layer
- `SqlClient` async trait — `connect / disconnect / execute / fetch_all / get_tables`
- `KvClient` async trait — `connect / disconnect / get / set / del / keys`
- `DbQueryResult` / `Row` / `Column` / `Value` types
- `DbError` typed errors via `thiserror`
- **SQLite connector** — `sqlx::SqlitePool`, type mapping (INTEGER / REAL / BLOB / TEXT)
- **PostgreSQL connector** — `sqlx::PgPool`, type mapping (BOOL / INT* / FLOAT* / NUMERIC / BYTEA / TEXT)
- **MySQL / MariaDB connector** — `sqlx::MySqlPool`, type mapping (TINYINT(1) / INT* / FLOAT / DECIMAL / BLOB* / TEXT)
- **Redis connector** — `redis::aio::MultiplexedConnection` wrapped in `Arc<tokio::sync::Mutex<…>>`
- Factory functions `connectors::connect_sql()` and `connectors::connect_kv()`

#### Application
- Full module skeleton: `app`, `config`, `db`, `events`, `ui`
- Async event loop: `crossterm::EventStream` + `tokio::mpsc` channel, 50 ms tick
- `Arc<dyn SqlClient>` / `Arc<dyn KvClient>` for zero-copy sharing across tokio tasks
- `DbEvent` channel: `SqlConnected` / `KvConnected` / `ConnectionFailed` / `TablesLoaded` / `TablesLoadFailed`
- `Ctrl-C` always quits; `q` quits or navigates back depending on context

#### UI — Connection screen (`AppState::Connection`)
- Left panel: saved profiles from config, `j/k` navigation, `Enter` to connect
- Right panel: DB type selector (`Tab` cycles postgres / sqlite / mysql / redis), URL input, cursor positioned
- Two modes: `Normal` (profile list) and `Editing` (manual DSN entry)
- Async connection with "Connecting…" feedback; errors displayed inline

#### UI — Table list screen (`AppState::TableList`)
- Header: active connection info (`[db_type] url`)
- Scrollable table list, `j/k` navigation
- Real-time filter with `/` (case-insensitive, `Esc` clears)
- Count display: `Tables (N)` or `Tables (match / total)` when filtered
- `Enter` → DataGrid (stub), `q` / `Esc` → disconnect and return to connection screen
- Tables loaded asynchronously via `spawn_load_tables()` after connection
- Redis: lists keys via `KEYS *`

---

[Unreleased]: https://github.com/TSODev/rowdy/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/TSODev/rowdy/releases/tag/v0.1.0
