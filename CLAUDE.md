# Rowdy — Notes de développement

## Présentation

**Rowdy** (`rowdy-db` sur crates.io) est un client de base de données TUI (Terminal User Interface) écrit en Rust. Objectif : gérer et interroger des bases de données sans quitter le terminal, avec une navigation clavier à la Vim.

## Stack

| Rôle | Crate |
|---|---|
| TUI | `ratatui` 0.27 + `crossterm` 0.27 (event-stream) |
| Éditeur multi-lignes | `tui-textarea` 0.5 |
| Async runtime | `tokio` (full) |
| Base de données | `sqlx` 0.7 — SQLite + PostgreSQL + MySQL |
| libsql / Turso | `libsql` 0.9 (remote) |
| Key-value | `redis` 0.24 (tokio-comp) |
| Traits async | `async-trait` |
| Configuration | `serde` + `toml` |
| Erreurs | `thiserror` |
| Streams async | `futures` |

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

Les connecteurs sont partagés via `Arc<dyn SqlClient>` / `Arc<dyn KvClient>` pour permettre l'accès concurrent depuis les tâches tokio sans déplacer la propriété.

### Traits

```rust
// src/db/traits/sql_client.rs
#[async_trait]
pub trait SqlClient: Send + Sync {
    async fn connect(&mut self, url: &str) -> Result<(), DbError>;
    async fn disconnect(&mut self) -> Result<(), DbError>;
    async fn execute(&self, query: &str) -> Result<u64, DbError>;
    async fn fetch_all(&self, query: &str) -> Result<DbQueryResult, DbError>;
    async fn get_tables(&self) -> Result<Vec<String>, DbError>;
}

// src/db/traits/kv_client.rs
#[async_trait]
pub trait KvClient: Send + Sync {
    async fn connect(&mut self, url: &str) -> Result<(), DbError>;
    async fn disconnect(&mut self) -> Result<(), DbError>;
    async fn get(&self, key: &str) -> Result<Option<String>, DbError>;
    async fn set(&self, key: &str, value: &str) -> Result<(), DbError>;
    async fn del(&self, key: &str) -> Result<bool, DbError>;
    async fn keys(&self, pattern: &str) -> Result<Vec<String>, DbError>;
}
```

### Flux de connexion async

```
User → Enter  →  ConnectionAction::Connect { url, db_type }
             →  App::spawn_connect()  →  tokio::spawn
                                              ↓ async
                                     connectors::connect_sql/kv()
                                              ↓
                                     DbEvent::SqlConnected(Arc<dyn SqlClient>)
                                              ↓ mpsc channel (≤ 50 ms)
                                     App::handle_db_event()
                                     → active_client = Some(...)
                                     → AppState::TableList
                                     → spawn_load_tables()
                                              ↓ async
                                     get_tables() / keys("*")
                                              ↓
                                     DbEvent::TablesLoaded(Vec<String>)
                                              ↓
                                     table_list_screen.set_tables(...)
```

### Flux d'exécution SQL (éditeur)

```
User → F5/Ctrl+Enter  →  SqlEditorAction::Execute(sql)
                      →  App::spawn_execute_query(sql)  →  tokio::spawn
                                                                ↓ async
                                              is_select_query() ?
                                              ↙ true            ↘ false
                                 fetch_all(sql)             execute(sql)
                                      ↓                          ↓
                             DbEvent::QueryRows(r)   DbEvent::QueryExecuted(n)
                                              ↓ mpsc channel
                                     sql_editor_screen.set_rows() / set_affected()
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

## Structure des modules

```
src/
├── main.rs                        # bootstrap tokio + TUI
├── app.rs                         # machine à états + event loop (50 ms tick)
├── config.rs                      # chargement ~/.config/rowdy/config.toml
├── events/
│   ├── app_event.rs               # AppEvent enum
│   └── handler.rs                 # dispatch clavier (stub)
├── db/
│   ├── error.rs                   # DbError (thiserror)
│   ├── types.rs                   # Column, Row, Value, DbQueryResult
│   ├── traits/
│   │   ├── sql_client.rs          # trait SqlClient
│   │   └── kv_client.rs           # trait KvClient
│   └── connectors/
│       ├── mod.rs                 # connect_sql() / connect_kv() factories
│       ├── postgres.rs            # ✅ implémenté
│       ├── sqlite.rs              # ✅ implémenté
│       ├── mysql.rs               # ✅ implémenté
│       ├── redis.rs               # ✅ implémenté (KvClient)
│       └── turso.rs               # ✅ implémenté (libsql remote)
└── ui/
    ├── layout.rs                  # dispatch draw() selon AppState
    ├── screens/
    │   ├── connection.rs          # ✅ implémenté
    │   ├── table_list.rs          # ✅ implémenté
    │   ├── data_grid.rs           # ✅ implémenté
    │   └── sql_editor.rs          # ✅ implémenté
    └── components/
        ├── status_bar.rs          # 🔲 stub
        └── modal.rs               # 🔲 stub
```

## Avancement

### Fait
- [x] Boilerplate initial du projet
- [x] Crate renommé `rowdy-db` (conflit de nom sur crates.io)
- [x] Licences MIT et Apache 2.0
- [x] Architecture des modules (ui, db, events, app, config)
- [x] Trait `SqlClient` (connect / disconnect / execute / fetch_all / get_tables)
- [x] Trait `KvClient` (connect / disconnect / get / set / del / keys)
- [x] Connecteur SQLite (sqlx)
- [x] Connecteur PostgreSQL (sqlx)
- [x] Connecteur MySQL / MariaDB (sqlx)
- [x] Connecteur Redis (redis-rs, tokio-comp)
- [x] Connecteur libsql / Turso (libsql 0.9, remote HTTP, URL `libsql://host?authToken=TOKEN`)
- [x] Factory `connect_sql()` / `connect_kv()`
- [x] Écran de connexion : liste de profils + saisie DSN + sélecteur de type
- [x] Event loop async (EventStream + mpsc channel, tick 50 ms)
- [x] Connexion async avec retour d'état via `DbEvent`
- [x] Vue liste des tables : navigation j/k, filtre `/`, chargement async
- [x] `~/.config/rowdy/config.toml` (profils de connexion)
- [x] Data Grid : défilement lignes/colonnes, collapse/expand colonnes
- [x] Data Grid : pagination infinite scroll (PAGE_SIZE=200, OFFSET progressif, COUNT parallèle)
- [x] Data Grid : filtres cumulatifs par colonne (`f/d/F`, `LIKE '%val%'`, BTreeMap, réinjection sûre)
- [x] Éditeur SQL multi-lignes (`tui-textarea`) : F5/Ctrl+Enter, focus editor/résultats, SELECT vs DML
- [x] Sauvegarde d'une nouvelle connexion dans `~/.config/rowdy/config.toml` (Ctrl+S → nom → Entrée)
- [x] Suppression d'un profil avec confirmation (D → y: fichier+liste / n: liste seulement)
- [x] Data Grid : curseur cellule (croisement ligne × colonne en bleu, reste de la ligne en jaune)
- [x] Introspection du schéma `get_schema()` — PK, FK, types (SQLite PRAGMA, PostgreSQL pg_catalog + MySQL information_schema)
- [x] Écran EditRecord : édition champ par champ, curseur complet (←/→/Backspace/Del/Home/End), badges [PK]/[→FK], aperçu SQL live, Ctrl+S sauvegarde
- [x] Décodage complet des types PostgreSQL : DATE/TIME/TIMESTAMP/TIMESTAMPTZ (chrono), UUID (uuid), JSON/JSONB (serde_json), tableaux `_TYPE` (Vec), INTERVAL/INET/CIDR/MACADDR/XML (String)
- [x] Décodage complet des types MySQL : DATE/TIME/DATETIME/TIMESTAMP (chrono), YEAR (u16), JSON (serde_json), ENUM/SET (String)
- [x] Marqueur `<?TYPE?>` universel sur les 3 connecteurs pour tout type non décodable (aide au debug)
- [x] Data Grid : badge magenta `[table_liée]` sur les cellules FK (introspection via `pg_catalog` pour PostgreSQL, `information_schema` pour MySQL, `PRAGMA foreign_key_list` pour SQLite)
- [x] FK expandable rows : Enter sur badge FK → sous-grille `FkGrid` avec navigation récursive (pile `fk_history`, Esc remonte d'un niveau, Enter sur non-FK → EditRecord)
- [x] Data Grid : `MAX_COL_WIDTH` 25 → 40, redimensionnement manuel `-`/`=` (pas de 5, min 4, max 80, `col_widths: HashMap<usize, u16>`), panel preview (2 lignes DarkGray) affichant la valeur complète de la cellule courante
- [x] Fix EditRecord depuis FkGrid : séparation `table_name` (SQL pur) / `display_name` (label `table [col=val]`) dans `DataGridScreen`
- [x] EditRecord : colonne type SQL (16 chars, bleu), toggle BOOLEAN avec `Space`, `sql_literal` génère `TRUE`/`FALSE` sans guillemets
- [x] SQL Editor → `F4` ouvre le résultat SELECT dans une grille lecture seule (`AppState::SqlResultGrid`, `read_only: bool` dans `DataGridScreen`) ; `q` retourne à l'éditeur
- [x] Suppression de tous les warnings `dead_code` du compilateur (15 → 0) : `AppState::Quit` retiré, `ConnectorType` retiré, `#[allow(dead_code)]` sur champs/méthodes d'API future, `#![allow(dead_code)]` sur stubs roadmap
- [x] Connecteur MySQL : normalisation `ssl-mode` case-insensitive (`REQUIRED`/`Required`/`required` tous acceptés) via `normalize_ssl_mode()` avant `MySqlPool::connect()`
- [x] SQL Editor : exécution multi-instructions — `split_sql_statements()` découpe sur `;`, supprime les lignes `--` et les commentaires inline, exécute chaque instruction séquentiellement avec rapport d'erreur `Statement X/N failed: … → preview…`
- [x] SQL Editor : messages d'erreur sur plusieurs lignes via `word_wrap()` calé sur la largeur réelle du panneau
- [x] Écran de connexion : messages d'erreur wrappés (`Wrap { trim: false }` sur le paragraphe de statut)
- [x] Barre de statut — badge mode (cyan), indicateur connexion (●/○), info DB (URL masquée), nb lignes, messages flash (permanente, 1 ligne en bas)
- [x] `redact_url()` — masque `user:password@` et paramètres sensibles (`authToken`, `token`, `password`, `pwd`, `secret`, `key`, `auth`) dans toute URL affichée dans l'UI (status bar, en-tête liste des tables, message "Connecting…")
- [x] Historique des requêtes SQL — `QueryHistory` persisté dans `~/.config/rowdy/history.toml` (max 200 entrées, dédoublonné), `Alt+↑/↓` dans l'éditeur SQL pour naviguer
- [x] Export CSV / JSON — `E` depuis DataGrid/FkGrid/SqlResultGrid, prompt `c`=CSV / `j`=JSON / `Esc`=cancel, fichier écrit dans `~/rowdy_<table>_<timestamp>.<ext>`
- [x] Mode read-only prod — `?readonly=true` (ou `&readonly=true`) dans l'URL, badge `READ-ONLY` rouge en status bar, bloque EditRecord + DML SQL editor, filtres/export conservés, reset à la déconnexion ; double `?` toléré (`strip_readonly_param` normalise en `&`)

### Roadmap

#### Différenciation (priorité haute)
- [ ] **Vue schema/ERD FK** — visualisation graphique des relations entre tables (l'introspection `get_schema` est déjà en place sur les 4 connecteurs SQL)
- [ ] **Connecteur MongoDB** — aucun concurrent TUI sérieux sur ce terrain ; trait `NoSqlClient` à définir, driver `mongodb` crate
- [x] **Mode read-only prod** — ✅ implémenté (voir section Fait)

#### Fonctionnel
- [ ] Modal de confirmation / erreur
- [ ] Vue clé-détail Redis dans le Data Grid
- [ ] Tests d'intégration sur les connecteurs
- [ ] _(priorité basse)_ Validation de format et helpers d'édition par type : format de date, JSON valide, UUID, etc.

## Commandes utiles

```bash
cargo run          # lancer le projet
cargo build        # compiler
cargo test         # lancer les tests
cargo clippy       # linter
```

## Conventions

- Navigation : bindings Vim (`j k`, `/` pour filtrer, `q` pour quitter/reculer)
- Async partout via `tokio` ; jamais de blocage dans le thread UI
- `Arc<dyn Trait>` pour partager un connecteur entre tâches sans mutex
- Binaire standalone, pas de dépendances runtime système
- `Frame<'_>` sans générique Backend (ratatui 0.27)
- `Table::new(rows, widths)` — ratatui 0.27 requiert les widths en 2e argument
- `DataGridScreen.table_name` = nom SQL brut ; `display_name: Option<String>` = label affiché (ex. `books [id=1]` pour FkGrid)
