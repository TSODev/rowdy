# 🤠 Rowdy

[![Crates.io](https://img.shields.io/crates/v/rowdy-db)](https://crates.io/crates/rowdy-db)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#-license)
![Rust](https://img.shields.io/badge/rust-2024_edition-orange?logo=rust)
![ratatui](https://img.shields.io/badge/ratatui-0.27-blueviolet)
![sqlx](https://img.shields.io/badge/sqlx-0.7-blue)
![tokio](https://img.shields.io/badge/tokio-1-green)
![tui-textarea](https://img.shields.io/badge/tui--textarea-0.5-purple)

A fast, modern, and rowdy **Terminal User Interface (TUI)** database management tool written in Rust.

Rowdy is designed for developers, DBAs, and terminal enthusiasts who want to inspect, query, and manage their databases without ever leaving their terminal or touching a mouse. Built on `ratatui` and `sqlx`, it compiles into a single standalone binary with no runtime dependencies.

![Rowdy demo](assets/demo.gif)

---

## ✨ Features

| Feature | Status |
|---------|--------|
| SQLite, PostgreSQL, MySQL/MariaDB connectors | ✅ |
| libsql / Turso connector (remote SQLite via libsql protocol) | ✅ |
| Redis connector (key listing) | ✅ |
| Saved connection profiles (`~/.config/rowdy/config.toml`) | ✅ |
| Save new connection with `Ctrl+S` — persists to config file | ✅ |
| Delete profile with confirmation (`D` → `y` / `n`) | ✅ |
| Encrypted credential storage — passwords and tokens stored in the OS keychain (macOS Keychain, libsecret, Windows Credential Manager) | ✅ |
| Vim-style keyboard navigation (`h j k l`, `/` to filter) | ✅ |
| Table list with live filter | ✅ |
| Data Grid — column scroll, collapse/expand, manual resize (`-`/`=`) | ✅ |
| Data Grid — cell preview panel (full value, no truncation) | ✅ |
| Data Grid — infinite scroll pagination (200 rows/page + COUNT) | ✅ |
| Data Grid — cumulative column filters (`f` / `d` / `F`) | ✅ |
| Data Grid — type-aware filters (bool → `= TRUE/FALSE`, numeric → `= n`) | ✅ |
| Data Grid — cell cursor (row × column highlight) | ✅ |
| Data Grid — FK badges + expandable sub-grid (recursive navigation) | ✅ |
| SQL Editor — multi-line, F5 to execute, F4 opens result in full grid | ✅ |
| SQL Editor — query history with `Alt+↑/↓`, persisted to `~/.config/rowdy/history.toml` | ✅ |
| Schema introspection — PK, FK, types (all 4 SQL engines) | ✅ |
| Inline record editing — field type display, bool toggle, live SQL preview | ✅ |
| Format validation in Edit Record — DATE/TIME/TIMESTAMP/UUID/JSON/INET validated on exit, red highlight + format hint | ✅ |
| Confirmation modal — `Ctrl+S` in Edit Record prompts before executing UPDATE | ✅ |
| Error modal — save failures displayed as a prominent overlay dialog | ✅ |
| Status bar — mode, connection indicator, DB info, row count, flash messages | ✅ |
| URL redaction — passwords and tokens masked in UI (`user:***@host`, `authToken=***`) | ✅ |
| Export CSV / JSON — `E` key in any data grid, file saved to `~/rowdy_<table>_<ts>.csv/json` | ✅ |
| Export JSON simple (`j`) or with recursive FK resolution (`J`) — nested `__ref` objects up to 3 levels deep, cycle detection | ✅ |
| Table list — TABLE / VIEW distinction with `[T]` / `[V]` badges; VIEW opens read-only with cyan badge | ✅ |
| Read-only safe mode — `?readonly=true` in URL, blocks all writes, `READ-ONLY` badge in status bar | ✅ |
| Pre-connect / post-disconnect hooks per profile — run shell scripts before connecting and after disconnecting (SSH tunnels, VPN…) | ✅ |
| Async I/O — UI never blocks during queries | ✅ |
| Redis key-detail view — `Enter` on a key shows its content (string/hash/list/set/zset) in a read-only grid with TTL | ✅ |
| Schema panel in table list — columns with PK/FK badges, outgoing and incoming FK relations, auto-loaded on connect | ✅ |
| ERD graph view (`r`) — star layout with box-drawing, bent arrows routed from exact FK column, navigate between boxes | ✅ |
| MongoDB connector (`--features mongodb`) — browse collections, MQL editor, nested field navigation | ✅ |
| MongoDB document editing — `replace_one` with confirmation, recursive nested object / array editor | ✅ |
| MongoDB insert (`a`) and delete (`D`) from Data Grid — confirmation modal, auto-reload | ✅ |
| MongoDB object fields — `Enter` drills in, `i` edits raw JSON inline | ✅ |
| DuckDB connector (`--features duckdb`) — embedded OLAP engine, Parquet/CSV/JSON queries, LIST and STRUCT types | ✅ |

---

## 🚀 Quick Start

### Build from source

```bash
git clone https://github.com/TSODev/rowdy.git
cd rowdy
cargo build --release
./target/release/rowdy-db
```

With MongoDB support:

```bash
cargo build --release --features mongodb
./target/release/rowdy-db
```

With DuckDB support (statically links DuckDB C++ — first build takes a few minutes):

```bash
cargo build --release --features duckdb
./target/release/rowdy-db
```

With all optional connectors:

```bash
cargo build --release --features mongodb,duckdb
./target/release/rowdy-db
```

### Install from crates.io

```bash
cargo install rowdy-db
```

With MongoDB support:

```bash
cargo install rowdy-db --features mongodb
```

With DuckDB support:

```bash
cargo install rowdy-db --features duckdb
```

---

## ⚙️ Configuration

Create `~/.config/rowdy/config.toml` to save connection profiles:

```toml
[[connections]]
name = "Local Postgres"
type = "postgres"
url = "postgres://user:password@localhost:5432/my_db"

[[connections]]
name = "Dev SQLite"
type = "sqlite"
url = "sqlite:///home/user/dev.db"

[[connections]]
name = "Turso Cloud"
type = "libsql"
url = "libsql://your-db-org.turso.io?authToken=eyJ..."

[[connections]]
name = "Cache Redis"
type = "redis"
url = "redis://127.0.0.1:6379"

[[connections]]
name = "MySQL Local"
type = "mysql"
url = "mysql://root:password@localhost:3306/my_db"

[[connections]]
name = "Analytics DuckDB"
type = "duckdb"
url = "duckdb:///home/user/analytics.db"
```

Profiles appear in the left panel of the connection screen at startup.

> **Credential security** — when you save a profile with `Ctrl+S`, Rowdy extracts the password or token from the URL and stores it in the OS keychain (macOS Keychain, libsecret on Linux, Windows Credential Manager). The URL written to `config.toml` uses a `__keyring__` placeholder instead of the actual secret:
>
> ```toml
> # What config.toml looks like after saving — no plaintext secrets
> [[connections]]
> name = "Local Postgres"
> type = "postgres"
> url = "postgres://user:__keyring__@localhost:5432/my_db"
> ```
>
> Credentials are resolved transparently at connection time. On headless Linux systems without libsecret, Rowdy falls back to storing the URL unchanged and displays a warning. The feature can be disabled at build time with `--no-default-features`.

You can also add optional **pre-connect** and **post-disconnect** shell scripts per profile (useful for SSH tunnels, VPN, etc.):

```toml
[[connections]]
name = "VPS Postgres (SSH tunnel)"
type = "postgres"
url = "postgres://user:password@localhost:5432/mydb"
pre_connect = "ssh -f -N -L 5432:localhost:5432 user@remote-host"
post_disconnect = "pkill -f 'ssh -L 5432:localhost:5432'"
```

The `pre_connect` script runs before the database connection is established; `post_disconnect` runs when you disconnect or quit the app.

To connect in **read-only mode** (blocks all writes — safe for production), append `?readonly=true` to any URL:

```toml
[[connections]]
name = "Production (read-only)"
type = "postgres"
url = "postgres://user:pass@prod-host/mydb?sslmode=require&readonly=true"
```

A red `READ-ONLY` badge appears in the status bar. `Enter` (edit record) and all DML statements in the SQL editor are disabled. Filters, pagination, and export still work normally.

---

## ⌨️ Keyboard shortcuts

### Connection screen

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate profiles |
| `Enter` | Connect to selected profile |
| `n` | Enter a new connection URL |
| `e` | Edit selected profile (pre-fills all fields for modification) |
| `Tab` | Cycle focus between fields: DB Type → URL → Pre-connect → Post-disconnect |
| `←` / `→` | Cycle database type when DB Type field is active |
| `Ctrl+S` | Save current connection (URL + scripts) as a named profile |
| `D` | Delete selected profile (with confirmation) |
| `q` | Quit |

### Table list

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate tables |
| `Enter` | Open table in Data Grid |
| `e` | Open SQL Editor |
| `r` | Open ERD graph view centered on selected table |
| `/` | Filter tables |
| `q` | Disconnect |

### Data Grid

| Key | Action |
|-----|--------|
| `j` / `k` | Next / previous row |
| `h` / `l` | Previous / next column |
| `g` / `G` | First / last row |
| `PgDn` / `PgUp` | ±10 rows |
| `Space` | Collapse / expand selected column |
| `-` / `=` | Shrink / grow selected column width (step 5) |
| `f` | Open filter input for selected column |
| `d` | Remove filter on selected column |
| `F` | Clear all filters |
| `Enter` | FK cell → open linked sub-grid ; other cell → Edit Record |
| `E` | Export prompt (then `c`=CSV, `j`=JSON, `J`=JSON+FK, `Esc`=cancel) |
| `q` | Back to table list |

### Edit Record

| Key | Action |
|-----|--------|
| `j` / `k` | Next / previous field |
| `Enter` | Edit selected field — or drill into nested `[obj]` / `[arr]` (MongoDB) |
| `i` | Edit selected field inline — on `[obj]` fields (MongoDB): edit raw JSON string instead of drilling in |
| `Space` | Toggle boolean field (`true` ↔ `false`) |
| `←` / `→` | Move cursor within field |
| `Home` / `End` | Jump to start / end of field |
| `Backspace` / `Del` | Delete character |
| `Ctrl+S` | Save changes — confirmation modal before UPDATE (SQL) or `replace_one` (MongoDB) |
| `Esc` / `q` | Back to Data Grid without saving — or confirm nested edit and go up one level (MongoDB) |

**MongoDB nested editor** — when drilling into `[obj]` or `[arr]` fields, the title shows the breadcrumb (`collection › field › subfield`). Press `Esc` at any nested level to confirm that level's edits and return to the parent. `Ctrl+S` is only available at the root level.

For `[obj]` fields specifically: `Enter` drills into the sub-editor; `i` edits the raw JSON string directly (useful when inserting a new document or when you prefer to type the JSON manually).

**Array editor** — additional keys when editing an array field:

| Key | Action |
|-----|--------|
| `a` | Add new item at end (enters edit mode immediately) |
| `D` | Delete selected item and renumber remaining items |

### ERD graph view (`r`)

| Key | Action |
|-----|--------|
| `j` / `k` or `Tab` | Cycle between visible table boxes |
| `Enter` | Re-center view on selected box (navigate the graph) |
| `q` / `Esc` | Back to table list |

The ERD view displays a **star layout**: the selected table in the center (yellow box), tables with incoming FK on the left (cyan), and tables referenced by outgoing FK on the right (cyan). Arrows are routed from the exact FK column line. No additional queries are made — the schema is reused from the panel loaded on connect.

### SQL Editor

| Key | Action |
|-----|--------|
| `F5` / `Ctrl+Enter` | Execute query |
| `F4` | Open SELECT result in full Data Grid (read-only) |
| `Alt+↑` | Recall previous query from history |
| `Alt+↓` | Recall next query from history (empty = clear) |
| `Tab` | Switch focus to results pane |
| `Tab` / `Esc` | Switch focus back to editor |
| `Ctrl+Q` | Back to table list |

`SELECT`, `WITH`, `EXPLAIN`, `SHOW`, `PRAGMA` → returns rows.  
`INSERT`, `UPDATE`, `DELETE`, `CREATE`, … → shows rows affected.

**`Ctrl-C` quits from anywhere.**

---

## 🗄️ Supported databases

| Engine | Type | Driver | Feature flag | URL format |
|--------|------|--------|--------------|------------|
| PostgreSQL | SQL | `sqlx` (native TLS) | _(built-in)_ | `postgres://user:pass@host:5432/db` |
| SQLite | SQL | `sqlx` | _(built-in)_ | `sqlite:///path/to/file.db` |
| libsql / Turso | SQL | `libsql` (remote HTTP) | _(built-in)_ | `libsql://host?authToken=TOKEN` |
| MySQL / MariaDB | SQL | `sqlx` | _(built-in)_ | `mysql://user:pass@host:3306/db` |
| Redis | Key-value | `redis-rs` (async multiplexed) | _(built-in)_ | `redis://host:6379` |
| MongoDB | Document | `mongodb` 3 | `--features mongodb` | `mongodb://user:pass@host:27017/dbname` |
| DuckDB | OLAP / SQL | `duckdb` 1 (bundled C++) | `--features duckdb` | `duckdb:///path/to/file.db` or `duckdb://:memory:` |

Optional connectors require building with the corresponding feature flag:

```bash
# MongoDB only
cargo build --release --features mongodb

# DuckDB only (first build is slow — statically compiles DuckDB C++)
cargo build --release --features duckdb

# Both
cargo build --release --features mongodb,duckdb
```

**DuckDB notes:**
- Supports local `.db` files and in-memory databases (`duckdb://:memory:`)
- Native support for Parquet, CSV, and JSON files via SQL (`SELECT * FROM 'data.parquet'`)
- Complex types (`VARCHAR[]`, `STRUCT`) are displayed as expandable nested views in the Data Grid
- Due to a DuckDB v1.x engine limitation, UPDATE on complex-type columns (`VARCHAR[]`, `STRUCT`) may fail with a FK constraint error when the table has child rows — use the SQL Editor as a workaround in that case

---

## 📖 Full documentation

See [USAGE.md](USAGE.md) for the complete user guide including connection URL formats, all keyboard shortcuts, and feature details.

---

## 🔨 Development

```bash
cargo run      # run in dev mode
cargo build    # debug build
cargo test     # run tests
cargo clippy   # lint
```

---

## 📜 License

Licensed under either of [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE) at your option.
