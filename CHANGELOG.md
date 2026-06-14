# Changelog

All notable changes to Rowdy are documented here.  
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased]

## [0.5.2] — 2026-06-14

### Fixed

- **FK badge display** : colonnes FK affichées en magenta `[table_liée]` à côté de la valeur
  - Requête `get_schema` PostgreSQL réécrite avec `pg_catalog` (sous-requêtes corrélées) — `information_schema.constraint_column_usage` retournait les colonnes référencées et non les colonnes sources
  - Condition `avail >= 2` supprimée : le badge s'affiche toujours, la valeur est tronquée à l'espace restant (cas courant : entier court + badge qui remplit presque la colonne)

## [0.5.1] — 2026-06-14

### Fixed / Added

#### Type decoding — PostgreSQL
- **DATE / TIME / TIMESTAMP / TIMESTAMPTZ** : now decoded via `chrono` (previously returned `NULL`)
- **UUID** : decoded via `uuid::Uuid` → hyphenated string
- **JSON / JSONB** : decoded via `serde_json::Value` → compact JSON string
- **Arrays** (`_TEXT`, `_INT4`, `_BOOL`, …) : decoded as `Vec<String|i64|bool>` → `[a, b, c]`
- **INTERVAL, INET, CIDR, MACADDR, XML** and other text-compatible types : decoded as `String` via the catch-all arm

#### Type decoding — MySQL
- **DATE / TIME / DATETIME / TIMESTAMP** : now decoded via `chrono`
- **YEAR** : decoded as `u16` → integer
- **JSON** : decoded via `serde_json::Value` → compact JSON string
- **ENUM / SET** : decoded as `String` (catch-all arm, already worked)

#### Fallback marker
- All three connectors now show `<?TYPE?>` instead of `NULL` when a value cannot be decoded, making gaps immediately visible in the grid

### In progress
- FK expandable rows (sub-grid with linked records when pressing Enter on an FK cell)
- Status bar component
- Confirmation / error modal
- Redis key-detail view in Data Grid
- Export CSV / JSON

---

## [0.5.0] — 2026-06-14

### Added

#### Schema introspection
- New `SqlClient::get_schema(table)` trait method returning `Vec<ColumnSchema>` (name, type, is_pk, is_nullable, FK target)
- **SQLite**: implemented via `PRAGMA table_info` + `PRAGMA foreign_key_list`
- **PostgreSQL**: implemented via `information_schema` join (PK + FK detection)
- **MySQL**: implemented via `information_schema.COLUMNS` + `KEY_COLUMN_USAGE`
- Schema loads in parallel with data when opening any table; stored in `DataGridScreen`

#### Cell cursor in Data Grid
- Selected cell highlighted with **blue background** at the row×column intersection
- Rest of the selected row highlighted in yellow
- `Enter` on a cell opens the Edit Record screen for that row

#### Edit Record screen (`AppState::EditRecord`)
- One field per line; `j/k` navigate between fields
- `Enter` or `i` activates a field for inline editing
- Full cursor support: `←/→` moves within the value, `Backspace`/`Del` deletes, `Home`/`End` jumps
- **PK fields** are read-only (grayed out, `[PK]` badge in cyan)
- **FK fields** display a `[→table]` badge in magenta (future: opens sub-grid on Enter)
- Modified fields highlighted in **green**
- Live **SQL preview** pane shows the `UPDATE "table" SET … WHERE "pk" = …` statement as you type
- `Ctrl+S` executes the UPDATE, reloads the Data Grid, and returns automatically
- `Esc` / `q` returns to Data Grid without saving

---

## [0.4.0] — 2026-06-14

### Added

#### Connection screen — profile management
- **Save a new connection** : `Ctrl+S` in editing mode opens a "Save as (name)" field; `Enter` writes the profile to `~/.config/rowdy/config.toml` (updates in place if the URL already exists); the list refreshes and the new profile is selected
- **Delete a profile** : `D` in normal mode enters a `ConfirmDelete` mode — the profile is highlighted in red and the help bar shows: `Delete "name"? y: delete from file   n: remove from list only   Esc: cancel`
  - `y` → removes from the list **and** from the config file
  - `n` / `Esc` → removes only from the in-memory list (file unchanged)

---

## [0.3.0] — 2026-06-14

### Added

#### Data Grid — column filters (cumulative)
- `f` opens a filter input for the selected column (`LIKE '%value%'`, case-insensitive on MySQL/SQLite)
- Multiple column filters combined with AND — cumulative across columns
- `d` removes the filter on the selected column and reloads
- `F` clears all filters and reloads
- Single-quote escaping (`'` → `''`) prevents SQL injection
- Filtered column headers highlighted in **cyan**; active filters shown in info bar as `[col≈value]`
- Filter state preserved across page loads; cleared on table re-open

#### Data Grid — pagination (infinite scroll)
- `PAGE_SIZE = 200` rows per fetch (`SELECT … LIMIT 200 OFFSET N`)
- Auto-loads next page when `j` is pressed on the last loaded row (seamless infinite scroll)
- `COUNT(*)` query runs in parallel for total row count
- Info bar shows `loaded/total rows` (or `N+ rows` while count is pending)
- Loading indicator `⏳` during async fetches
- `has_more` / `loading` flags prevent duplicate concurrent requests

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

[Unreleased]: https://github.com/TSODev/rowdy/compare/v0.5.1...HEAD
[0.5.1]: https://github.com/TSODev/rowdy/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/TSODev/rowdy/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/TSODev/rowdy/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/TSODev/rowdy/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/TSODev/rowdy/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/TSODev/rowdy/releases/tag/v0.1.0
