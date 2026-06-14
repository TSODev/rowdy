# 🤠 Rowdy

A fast, modern, and rowdy **Terminal User Interface (TUI)** database management tool written in Rust.

Rowdy is designed for developers, DBAs, and terminal enthusiasts who want to inspect, query, and manage their databases without ever leaving their terminal or touching a mouse. Built on `ratatui` and `sqlx`, it compiles into a single standalone binary with no runtime dependencies.

---

## ✨ Features

| Feature | Status |
|---------|--------|
| SQLite, PostgreSQL, MySQL/MariaDB connectors | ✅ |
| Redis connector (key listing) | ✅ |
| Saved connection profiles (`~/.config/rowdy/config.toml`) | ✅ |
| Vim-style keyboard navigation (`h j k l`, `/` to filter) | ✅ |
| Table list with live filter | ✅ |
| Data Grid — column scroll, collapse/expand, pagination | ✅ |
| SQL Editor — multi-line, F5 to execute, results table | ✅ |
| Async I/O — UI never blocks during queries | ✅ |
| Inline cell editing | 🔲 planned |
| Export CSV / JSON | 🔲 planned |
| Status bar & modal dialogs | 🔲 planned |

---

## 🚀 Quick Start

### Build from source

```bash
git clone https://github.com/TSODev/rowdy.git
cd rowdy
cargo build --release
./target/release/rowdy-db
```

### Install from crates.io _(coming soon)_

```bash
cargo install rowdy-db
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
name = "Cache Redis"
type = "redis"
url = "redis://127.0.0.1:6379"

[[connections]]
name = "MySQL Local"
type = "mysql"
url = "mysql://root:password@localhost:3306/my_db"
```

Profiles appear in the left panel of the connection screen at startup.

---

## ⌨️ Keyboard shortcuts

### Connection screen

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate profiles |
| `Enter` | Connect to selected profile |
| `n` | Enter a new connection URL |
| `Tab` | Cycle database type (postgres → sqlite → mysql → redis) |
| `q` | Quit |

### Table list

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate tables |
| `Enter` | Open table in Data Grid |
| `e` | Open SQL Editor |
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
| `q` | Back to table list |

### SQL Editor

| Key | Action |
|-----|--------|
| `F5` / `Ctrl+Enter` | Execute query |
| `Tab` | Switch focus to results pane |
| `Tab` / `Esc` | Switch focus back to editor |
| `Ctrl+Q` | Back to table list |

`SELECT`, `WITH`, `EXPLAIN`, `SHOW`, `PRAGMA` → returns rows.  
`INSERT`, `UPDATE`, `DELETE`, `CREATE`, … → shows rows affected.

**`Ctrl-C` quits from anywhere.**

---

## 🗄️ Supported databases

| Engine | Type | Driver |
|--------|------|--------|
| PostgreSQL | SQL | `sqlx` (native TLS) |
| SQLite | SQL | `sqlx` |
| MySQL / MariaDB | SQL | `sqlx` |
| Redis | Key-value | `redis-rs` (async multiplexed) |

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
