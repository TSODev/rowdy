# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Rowdy (`rowdy-db` on crates.io) is a TUI database client written in Rust with `ratatui` + `tokio`. It connects to PostgreSQL, SQLite, MySQL, libsql/Turso, Redis, and optionally MongoDB and DuckDB, and lets you browse tables, edit rows, run SQL, and export data — all from the terminal.

## Commands

```bash
cargo build                                  # debug build, built-in connectors only
cargo build --release                        # release build
cargo build --features mongodb               # + MongoDB
cargo build --features duckdb                # + DuckDB (bundled C++, first build is slow)
cargo build --features mongodb,duckdb        # both optional connectors

cargo run                                    # run in dev mode

cargo test                                   # run all tests
cargo test duckdb -- --nocapture             # DuckDB-only tests, requires --features duckdb
cargo test test_fetch_all_scalar_types       # single test by name
cargo clippy -- -D warnings                  # lint (CI-equivalent)
```

### Integration tests need live services

SQLite, MongoDB (`:memory:`/embedded-style) and DuckDB (`:memory:`) tests run with zero setup. PostgreSQL, MySQL, Redis, and Turso tests read a connection URL from an env var and **skip silently (early return) if it's unset** — they are not mocked:

| Engine | Env var |
|--------|---------|
| PostgreSQL | `POSTGRES_URL` |
| MySQL | `MYSQL_URL` |
| Redis | `REDIS_URL` |
| Turso | `TURSO_URL` |

Seed scripts for spinning up local test data live in `seed/` (`postgres.sql`, `mysql.sql`, `redis.sh`, `mongodb.sh`, `duckdb.sql`, plus `snippets.toml`).

CI (`.github/workflows/ci.yml`) only runs the infra-free subset (SQLite + MongoDB + DuckDB, ~50 tests) — Postgres/MySQL/Redis/Turso tests never run there since no `POSTGRES_URL` etc. is set.

## Architecture

### Connector abstraction

Every database engine implements one of three traits in `src/db/traits/`:
- `SqlClient` — PostgreSQL, SQLite, MySQL, Turso, DuckDB (`connect`, `execute`, `fetch_all`, `get_table_objects`, `get_schema`)
- `KvClient` — Redis
- `NoSqlClient` — MongoDB

Concrete implementations live in `src/db/connectors/*.rs`, one file per engine. `src/db/connectors/mod.rs::connect_sql/connect_kv/connect_nosql` is the single dispatch point that maps a `db_type` string (from the URL scheme or profile) to a boxed trait object. To add a new SQL-family engine: implement `SqlClient`, register it in `connect_sql`, and add its display type to `DB_TYPES` in `src/ui/screens/connection.rs`.

MongoDB and DuckDB are gated behind `#[cfg(feature = "mongodb")]` / `#[cfg(feature = "duckdb")]` throughout — connector module, `connect_*` match arms, and `Cargo.toml` `[features]`.

**Watch out**: `libsql` (Turso) and `sqlx`'s `sqlite` feature both bundle their own statically-linked copy of SQLite's C source. If `libsql`'s `core` feature is ever re-enabled (e.g. by removing `default-features = false` from its `Cargo.toml` entry), the two `sqlite3.c` object files collide at link time with `duplicate symbol` errors — this exact bug broke `cargo install rowdy-db` for a while (fixed in 0.9.4). Only the `remote` feature of `libsql` is needed; keep `default-features = false`.

### App state machine (`src/app.rs`)

`App` holds `Vec<Tab>` — one `Tab` per connection (`Ctrl+T` new tab, `Ctrl+W` close, `[`/`]` cycle). Each `Tab` owns its own `AppState` enum (`Connection → TableList → DataGrid/SqlEditor/ErdGraph → EditRecord`, plus `FkGrid`/`SqlResultGrid`), one screen struct per state (`src/ui/screens/*.rs`), and an `active_client: Option<ActiveClient>` (`Sql`/`Kv`/`NoSql`, each an `Arc<dyn Trait>` so background tasks can share it).

Input flow: `App::run` reads a key → `Tab::handle_key` matches on `self.state` → delegates to the current screen's own `handle_key`, which returns a small `*Action` enum (`ConnectionAction`, `DataGridAction`, `SqlEditorAction`, …) → `Tab::handle_key` interprets that action, mutating state and/or calling a `spawn_*` method.

All I/O is async and non-blocking relative to the UI: `spawn_*` methods (e.g. `spawn_connect`, `spawn_load_data`, `spawn_execute_query`) `tokio::spawn` a task that talks to the DB client and sends a `DbEvent` back over an mpsc channel (`Tab::db_tx`/`db_rx`). `App::run`'s main loop drains `db_rx` for **every** tab each frame (not just the active one), so background tabs keep working while you're on another tab. `Tab::handle_db_event` (further down in `app.rs`, not shown above) maps each `DbEvent` variant to a screen update.

Destructive actions (record save, MongoDB insert/replace/delete) go through `PendingAction` + a confirmation `Modal` rather than executing immediately — `handle_modal_key` fires the actual `spawn_*` on `y`/`Enter`.

### Query building & FK navigation

`src/db/query_builder.rs` builds parameterized-looking SQL strings (filters, sort, pagination, FK lookups) generically across the SQL engines from a `Vec<ColumnSchema>`. FK cells are detected via `ColumnSchema.fk` (populated by each connector's `get_schema` from `pg_catalog` / `information_schema` / SQLite `PRAGMA`); pressing `Enter` on one opens a recursive sub-grid (`fk_grid_screen` + `fk_history` stack in `Tab`) rather than navigating away from the current grid.

### Credentials

`src/config.rs` resolves `__keyring__` placeholders in `~/.config/rowdy/config.toml` against the OS keychain (via the `keyring` crate, feature `secure-storage`, default-on) at connect time — plaintext secrets are never written to disk when a profile is saved with `Ctrl+S`. `redact_url`/`strip_readonly_param` in the same file handle masking secrets for display and parsing the `?readonly=true` query param that flips `Tab::prod_readonly`.

### Nested document editing (MongoDB)

`EditRecordScreen` supports drilling into nested `[obj]`/`[arr]` fields recursively; `Tab` keeps a parallel `edit_record_stack: Vec<(EditRecordScreen, usize)>` so `Esc` at a nested level reconstructs the child's JSON (`reconstruct_nested_json`/`reconstruct_nested_array`) back into the parent's field before popping.
