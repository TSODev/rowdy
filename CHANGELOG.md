# Changelog

All notable changes to Rowdy are documented here.  
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased]

### In progress
- FK expandable rows (display related records below a row when a column is a foreign key)
- Inline cell editing
- Status bar component
- Confirmation / error modal
- Redis key-detail view in Data Grid

---

## [0.2.0] — 2026-06-14

### Added

#### SQL Editor (`AppState::SqlEditor`)
- `tui-textarea` 0.5 integration (ratatui 0.27 compatible)
- Multi-line SQL editor with placeholder text, cursor, undo/redo, full text editing
- Split-pane layout: editor (45%) + results (flexible) + help bar (3 lines)
- `EditorFocus` toggle: `Tab` switches between editor and results pane; `Esc` or `Tab` returns to editor
- **F5** or **Ctrl+Enter** executes the query asynchronously
- **Ctrl+Q** returns to the table list
- Auto-detect SELECT vs. DML: `SELECT/WITH/EXPLAIN/SHOW/DESCRIBE/PRAGMA` → `fetch_all`; everything else → `execute`
- Results: row-scrollable table with `j/k/g/G/PgUp/PgDn`, column scrolling with `h/l`
- DML result shows "N row(s) affected" in green
- Error shown in red inline (no modal needed)
- Running indicator `⏳` in the editor title while query executes
- `e` key in table list opens the SQL editor

#### Dependency
- `tui-textarea = "0.5"` added
- `ratatui` upgraded from `0.26` to `0.27`

#### Bug fixes
- Restored `f.size()` and `f.set_cursor()` compatibility with ratatui 0.27.0 (which still uses the 0.26 API)

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

#### UI — Data Grid screen (`AppState::DataGrid`)
- Paginated table view via `ratatui::widgets::Table` + `TableState` (auto-scroll)
- Column width calculated from content, capped at 25 chars
- Horizontal column scroll (`h/l`) with automatic `col_offset` adjustment
- Column collapse/expand with `Space` (collapsed → 3 chars wide, header shows `…`)
- Selected column highlighted with yellow underlined header
- `g/G` first/last row, `PgUp/PgDn` ±10 rows
- Data loaded asynchronously via `SELECT * FROM "table" LIMIT 1000`
- Graceful error display for KV stores (Redis) and load failures
- Values: `NULL`, bool, int, float, text (newlines → `↵`), bytes (`<N bytes>`)
- `q` / `Esc` → back to table list

#### UI — Table list screen (`AppState::TableList`)
- Header: active connection info (`[db_type] url`)
- Scrollable table list, `j/k` navigation
- Real-time filter with `/` (case-insensitive, `Esc` clears)
- Count display: `Tables (N)` or `Tables (match / total)` when filtered
- `Enter` → DataGrid (stub), `q` / `Esc` → disconnect and return to connection screen
- Tables loaded asynchronously via `spawn_load_tables()` after connection
- Redis: lists keys via `KEYS *`

---

[Unreleased]: https://github.com/TSODev/rowdy/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/TSODev/rowdy/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/TSODev/rowdy/releases/tag/v0.1.0
