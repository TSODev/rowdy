# Rowdy — Notes de développement

## Présentation

**Rowdy** (`rowdy-db` sur crates.io) est un client de base de données TUI (Terminal User Interface) écrit en Rust. Objectif : gérer et interroger des bases de données sans quitter le terminal, avec une navigation clavier à la Vim.

## Stack

| Rôle | Crate |
|---|---|
| TUI | `ratatui` 0.26 + `crossterm` 0.27 |
| Async runtime | `tokio` (full) |
| Base de données | `sqlx` 0.7 — SQLite + PostgreSQL |
| Traits async | `async-trait` |

## Architecture

Rowdy utilise une architecture découplée séparant le thread de rendu UI des opérations I/O base de données, pour éviter tout gel de l'interface.

```
             ┌─────────────────────────────────┐
             │       TUI Main Event Loop       │◄────────────────┐
             └──────┬────────────────────▲─────┘                 │
                    │                    │                       │
     User Inputs    │                    │ (Tokio mpsc Channel)  │ (Tokio mpsc Channel)
    (Key presses)   │                    │                       │
                    ▼                    │                       │
        ┌───────────────────────┐   ┌────┴──────────────────┐  ┌─┴─────────────────────┐
        │  Crossterm Backend    │   │  PostgreSQL Connector │  │   SQLite Connector    │
        └───────────────────────┘   └───────────────────────┘  └───────────────────────┘
```

Chaque connecteur implémente le trait commun `DatabaseClient` :

```rust
#[async_trait]
pub trait DatabaseClient: Send + Sync {
    async fn connect(&mut self, url: &str) -> Result<(), DbError>;
    async fn execute(&self, query: &str) -> Result<u64, DbError>;
    async fn fetch_all(&self, query: &str) -> Result<DbQueryResult, DbError>;
    async fn get_tables(&self, query: &str) -> Result<Vec<String>, DbError>;
}
```

## Configuration

Rowdy recherche un fichier de configuration dans `~/.config/rowdy/config.toml` pour sauvegarder les profils de connexion :

```toml
[[connections]]
name = "Local Postgres"
type = "postgres"
url = "postgres://postgres:password@localhost:5432/my_db"

[[connections]]
name = "Production Analytics"
type = "sqlite"
url = "/path/to/local/analytics.db"
```

## Avancement

### Fait
- [x] Boilerplate initial du projet
- [x] Crate renommé `rowdy-db` (conflit de nom sur crates.io)
- [x] Mise à jour de l'URL du dépôt
- [x] Mise à jour de l'auteur dans `Cargo.toml`
- [x] `main.rs` : initialisation du terminal TUI et affichage d'un écran de démarrage

### En cours
- [ ] Architecture des modules (ui, db, app)

### Roadmap
- [ ] Couche d'abstraction base de données (`trait DatabaseClient`)
- [ ] Connexion SQLite
- [ ] Connexion PostgreSQL
- [ ] Écran de connexion (saisie DSN)
- [ ] Navigation Vim (`h j k l /`)
- [ ] Vue liste des tables
- [ ] Data Grid avec pagination mémoire
- [ ] Édition inline de cellules
- [ ] Éditeur SQL multi-lignes (`tui-textarea` + coloration syntaxique)
- [ ] Support MySQL/MariaDB
- [ ] Support Redis

## Commandes utiles

```bash
cargo run          # lancer le projet
cargo build        # compiler
cargo test         # lancer les tests
cargo clippy       # linter
```

## Conventions

- Navigation : bindings Vim (`h j k l`, `/` pour rechercher)
- Édition 2024, async/await partout via `tokio`
- Binaire standalone : pas de dépendances runtime système
