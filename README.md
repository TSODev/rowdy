# 🤠 Rowdy

A fast, modern, and rowdy **Terminal User Interface (TUI)** database management tool written in Rust.

Rowdy is designed for developers, DBAs, and terminal enthusiasts who want to inspect, query, and manage their databases without ever leaving their terminal or touching a mouse. Built on top of `ratatui` and `sqlx`, it aims to be lightweight, cross-platform, and blazing fast.

---

## ✨ Features (Roadmap)

- **Unified Interface:** Support for multiple database engines (SQLite, PostgreSQL, MySQL/MariaDB, and Redis) using a clean abstraction layer.
- **Keyboard-Driven Navigation:** Full Vim-bindings (`h`, `j`, `k`, `l`, `/`) for lightning-fast navigation.
- **Data Grid view:** Seamlessly scroll through millions of rows with efficient memory buffering and inline cell editing.
- **Embedded SQL Editor:** Advanced multi-line query editor with syntax highlighting (powered by `tui-textarea`).
- **No Heavy Dependencies:** Compiles into a single, tiny, standalone binary.

---

## 🚀 Quick Start

### Installation from Crates.io

Once the project is fully compiled and published, you can install it directly via Cargo:

```bash
cargo install rowdy