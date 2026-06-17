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

### Connectors

| Engine | Type | Feature flag | URL format |
|--------|------|-------------|------------|
| PostgreSQL | SQL | _(built-in)_ | `postgres://user:pass@host:5432/db` |
| SQLite | SQL | _(built-in)_ | `sqlite:///path/to/file.db` |
| MySQL / MariaDB | SQL | _(built-in)_ | `mysql://user:pass@host:3306/db` |
| libsql / Turso | SQL | _(built-in)_ | `libsql://host?authToken=TOKEN` |
| Redis | Key-value | _(built-in)_ | `redis://host:6379` |
| MongoDB | Document | `--features mongodb` | `mongodb://user:pass@host:27017/db` |
| DuckDB | OLAP | `--features duckdb` | `duckdb:///path/to/file.db` |

### Connections & profiles

- **Saved profiles** — `~/.config/rowdy/config.toml` ; add with `Ctrl+S`, edit with `e`, delete with `D`
- **Encrypted credentials** — passwords and tokens stored in the OS keychain (macOS Keychain, libsecret, Windows Credential Manager) ; `__keyring__` placeholder in config file, resolved transparently at connect time
- **Pre/post-connect hooks** — optional shell scripts per profile for SSH tunnels, VPN, proxies
- **Read-only mode** — append `?readonly=true` to any URL ; red `READ-ONLY` badge ; all writes blocked, filters and export still work
- **URL redaction** — passwords and tokens masked in the UI everywhere (`user:***@host`, `authToken=***`)

### Data Grid

- **Infinite scroll pagination** — 200 rows/page, next page loads on `j` at last row, parallel `COUNT(*)`
- **Cumulative column filters** — `f` to filter, `d` to remove, `F` to clear all ; type-aware (`= TRUE/FALSE` for booleans, `= n` for numerics)
- **Sort by column** — `s` cycles ASC / DESC / reset ; `▲`/`▼` indicator in header
- **FK navigation** — magenta badge on FK cells ; `Enter` opens a recursive sub-grid ; breadcrumb in info bar
- **Nested field navigation** (MongoDB) — green `[obj]` / `[arr:N]` badges ; `Enter` drills into sub-grids recursively
- **Column resize** — `-`/`=` in steps of 5 (min 4, max 80) ; `Space` collapse/expand ; cell cursor highlight
- **Preview panel** — full value of selected cell displayed below the grid, no truncation
- **Load all** — `A` fetches all rows in a single query (replaces paged data)
- **TABLE / VIEW distinction** — `[T]` / `[V]` badges ; VIEW opens automatically read-only
- **Redis key-detail view** — `Enter` on a key shows string / hash / list / set / zset content with TTL

### SQL Editor

- **Multi-line editor** — `tui-textarea` ; `F5` / `Ctrl+Enter` to execute ; `Ctrl+Q` to exit
- **Multi-statement execution** — splits on `;`, executes sequentially, per-statement error report
- **SQL autocomplete** — `Tab` triggers a floating popup with tables, columns and 80 SQL keywords ; case-insensitive matching
- **Query history** — `Alt+↑/↓` to browse ; persisted in `~/.config/rowdy/history.toml` (200 entries, deduped)
- **F4** — opens the current SELECT result in a full read-only Data Grid

### Record editing

- **Edit Record** — field-by-field editor with full cursor, `[PK]` / `[→FK]` badges, live SQL preview, bool toggle with `Space`
- **Format validation** — DATE / TIME / TIMESTAMP / UUID / JSON / INET / CIDR validated on exit ; red highlight + format hint ; `Ctrl+S` blocked while errors remain
- **Confirmation modal** — shows the UPDATE statement before executing ; error modal on failure
- **MongoDB CRUD** — `Enter` opens Edit Record for `replace_one` ; `a` inserts ; `D` deletes ; drill into nested `[obj]`/`[arr]` fields (`Enter` = sub-editor, `i` = raw JSON edit)

### Schema & ERD

- **Schema introspection** — PK, FK, column types on all SQL connectors (PostgreSQL `pg_catalog`, MySQL `information_schema`, SQLite/Turso `PRAGMA`)
- **Schema panel** — right-hand panel in the table list showing columns with PK/FK badges and outgoing/incoming FK relations, loaded at connect time
- **ERD graph view** — `r` from the table list ; star layout with the selected table at center ; bent ASCII arrows routed from the exact FK column ; `j/k` to navigate boxes

### Export

- **CSV** — `E` → `c` ; RFC 4180, quoted fields, empty for NULL ; saved to `~/rowdy_<table>_<timestamp>.csv`
- **JSON simple** — `E` → `j` ; array of typed objects
- **JSON + FK resolution** — `E` → `J` ; embeds referenced rows as `<col>__ref` objects, recursive up to 3 levels, cycle detection

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

## 🦆 DuckDB notes

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
