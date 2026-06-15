# Changelog

All notable changes to Rowdy are documented here.  
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased]

## [0.7.0] — 2026-06-15

### Added

#### Modal de confirmation / erreur
- `Ctrl+S` dans EditRecord ouvre un modal de confirmation avant d'exécuter l'UPDATE — affiche un aperçu de la requête SQL
- `[Y]` confirme et exécute ; `[N]` / `Esc` annule et retourne à l'écran d'édition
- Les erreurs de sauvegarde (`EditFailed`) ouvrent désormais un modal d'erreur (fond rouge) plutôt qu'un message inline
- Le modal s'affiche en overlay centré sur l'écran courant ; toutes les touches sont interceptées pendant son affichage

#### Mode read-only production
- Ajouter `?readonly=true` à n'importe quelle URL de connexion pour activer le mode lecture seule
- **Badge `READ-ONLY`** rouge en barre de statut, visible depuis tous les écrans
- **Écriture bloquée** : Enter (EditRecord) désactivé en Data Grid et FK View ; INSERT/UPDATE/DELETE/DROP refusés dans l'éditeur SQL avec message d'erreur explicite
- **Navigation conservée** : filtres, pagination, export CSV/JSON, redimensionnement colonnes continuent de fonctionner
- Le flag est réinitialisé à la déconnexion
- Le paramètre `readonly` est strippé de l'URL avant connexion (non transmis au driver)

### Fixed

- **Liste des tables — en-tête "Connected:"** : ligne redondante supprimée (la barre de statut affiche déjà le type et l'URL de connexion) — la liste gagne une ligne de hauteur
- **Écran de connexion — message "Connecting…"** : l'URL affichée pendant la connexion passe maintenant par `redact_url()` — le mot de passe et les tokens (`authToken`, `password`, etc.) sont masqués `***` comme dans la barre de statut
- **Data Grid — redimensionnement de colonne** : les touches `[` / `]` remplacées par `-` / `=`, directement accessibles sur les claviers AZERTY et QWERTY sans modificateur
- **Mode read-only — double `?` dans l'URL** : `strip_readonly_param` normalise les `?` supplémentaires en `&` avant de parser les paramètres — `?readonly=true` est détecté même quand d'autres paramètres sont déjà présents (`?sslmode=require&channel_binding=require?readonly=true` fonctionne)

## [0.6.0] — 2026-06-15

### Added

#### Barre de statut (status bar)
- Ligne permanente en bas de l'écran, visible depuis tous les écrans
- **Badge mode** (cyan/gras) : `CONNECTION`, `TABLES`, `DATA GRID`, `FK VIEW`, `EDIT`, `SQL EDITOR`, `QUERY RESULT`
- **Indicateur de connexion** : `●` vert (connecté) ou `○` rouge (déconnecté)
- **Info DB** : type + URL (tronquée à 45 caractères)
- **Nombre de lignes** : affiché en Data Grid, FK View et SQL Result (`[N rows]`)
- **Messages flash** : messages de confirmation (vert) ou d'erreur (rouge) temporaires

#### Masquage des mots de passe et tokens (URL redaction)
- `redact_url()` masque les identifiants dans toute URL affichée (status bar et en-tête de la liste des tables)
- `user:password@host` → `user:***@host`
- Paramètres sensibles (`authToken`, `token`, `password`, `pwd`, `secret`, `key`, `auth`) → `param=***`

#### Historique des requêtes SQL
- Les requêtes exécutées sont sauvegardées dans `~/.config/rowdy/history.toml` (max 200 entrées, dédoublonnées)
- `Alt+↑` : rappeler la requête précédente (plus ancienne)
- `Alt+↓` : rappeler la requête suivante (plus récente) — revenir à vide
- Le curseur d'historique se réinitialise à chaque nouvelle exécution
- Raccourci affiché dans la barre d'aide de l'éditeur SQL

#### Export CSV / JSON
- `E` depuis DataGrid, FkGrid ou SqlResultGrid ouvre le prompt d'export
- `c` → CSV (RFC 4180 : guillemets si nécessaire, champs vides pour NULL)
- `j` → JSON (tableau d'objets, valeurs typées)
- `Esc` → cancel
- Fichier écrit dans `~/rowdy_<table>_<timestamp>.csv/json`
- Confirmation dans la status bar : `Saved: ~/rowdy_books_1718453421.csv`
- `E: export` ajouté à la barre d'aide de toutes les grilles

## [0.5.8] — 2026-06-15

### Fixed

#### Connecteur MySQL — normalisation `ssl-mode`
- La valeur du paramètre `ssl-mode` dans l'URL est normalisée en minuscules avant connexion : `REQUIRED`, `Required` et `required` sont tous acceptés (sqlx 0.7 est case-sensitive sur cette valeur)

#### SQL Editor — exécution multi-instructions
- Les scripts collés contenant plusieurs instructions séparées par `;` sont découpés et exécutés séquentiellement au lieu d'être envoyés en bloc (MySQL refuse les multi-statements par défaut)
- Les lignes de commentaires `--` sont supprimées avant découpage pour éviter les erreurs de charset sur les caractères Unicode (ex. caractères box-drawing `──` dans les commentaires)
- Les commentaires inline `sql -- note` sont également strippés (en vérifiant que `--` n'est pas à l'intérieur d'une chaîne `'...'`)
- En cas d'échec, le message d'erreur indique le numéro d'instruction (`Statement X/N failed`) et les 5 premières lignes du statement fautif

#### SQL Editor — affichage des erreurs
- Les messages d'erreur longs sont découpés en plusieurs lignes (word-wrap calculé sur la largeur réelle du panneau) — fini les messages tronqués

#### Écran de connexion — affichage des erreurs
- Même correction : `Wrap { trim: false }` appliqué sur le paragraphe de statut/erreur

## [0.5.7] — 2026-06-15

### Fixed

#### Compiler warnings — dead_code (15 → 0)
- `AppState::Quit` supprimé : le quit passe par `should_quit` ; le bras `_` devenu inatteignable dans le dispatch clavier retiré ; bras `AppState::Quit => {}` retiré de `layout.rs`
- `DbEvent::SchemaLoadFailed(String)` : payload `String` retiré (jamais lu ; site de construction mis à jour)
- `ConnectorType` enum et `from_str` supprimés de `db/connectors/mod.rs` (reliquat — la factory utilise déjà un `match` direct sur la chaîne)
- `#[allow(dead_code)]` sur les champs d'API future : `Column::type_name`, `DbQueryResult::rows_affected`, `ColumnSchema::is_nullable`
- `#[allow(dead_code)]` sur les méthodes de trait non encore appelées : `KvClient::{disconnect, get, set, del}`, `SqlClient::disconnect`
- `#![allow(dead_code)]` sur les stubs de roadmap : `events/app_event.rs`, `events/handler.rs`, `ui/components/modal.rs`, `ui/components/status_bar.rs`

## [0.5.6] — 2026-06-15

### Added

#### Connecteur libsql / Turso
- Nouveau connecteur `TursoClient` basé sur `libsql` 0.9.30 (feature `remote`)
- Implémente le trait `SqlClient` : `connect` / `execute` / `fetch_all` / `get_tables` / `get_schema`
- `get_schema` via `PRAGMA table_info` + `PRAGMA foreign_key_list` (compatible SQLite)
- URL format : `libsql://host?authToken=TOKEN` — le token est parsé et passé séparément au Builder (non inclus dans l'URL réseau)
- Type `libsql` ajouté au sélecteur de connexion (`Tab` pour cycler : postgres → sqlite → libsql → mysql → redis)
- Rétrocompatibilité : le type `turso` est également accepté comme alias dans la factory

## [0.5.5] — 2026-06-15

### Fixed

- **Data Grid — filtre BOOLEAN** : `build_where` détecte les colonnes `BOOL`/`BOOLEAN`/`TINYINT(1)` via le schéma et génère `= TRUE`/`= FALSE` au lieu de `LIKE '%val%'` (qui échouait sur PostgreSQL avec `operator does not exist: boolean ~~ unknown`)
- **Data Grid — filtre colonnes numériques** : génère `= val` (sans guillemets ni `LIKE`) pour les colonnes `INT`/`FLOAT`/`NUMERIC`/`DECIMAL` lorsque la valeur saisie est un nombre valide
- Le schéma (`Vec<ColumnSchema>`) est désormais propagé à `spawn_sql_page` et `spawn_sql_count` pour permettre la génération de clauses `WHERE` type-aware

## [0.5.4] — 2026-06-15

### Added

#### EditRecord — affichage du type et gestion des booléens
- **Colonne type** (16 caractères, bleu) affichée entre le badge `[PK]`/`[→FK]` et la valeur — le type SQL brut (`integer`, `character varying`, `boolean`…) est visible directement dans l'écran d'édition
- **Toggle booléen** : `Space` en mode navigation bascule `true` ↔ `false` sur tout champ de type `BOOL`/`BOOLEAN`/`TINYINT(1)` sans passer en mode édition
- **`sql_literal` BOOLEAN** : génère désormais `TRUE`/`FALSE` sans guillemets (au lieu de `'true'`/`'false'`)

#### SQL Editor → Data Grid (`F4`)
- **`F4`** depuis l'éditeur SQL (quel que soit le focus — éditeur ou panneau résultats) ouvre le résultat SELECT courant dans le Data Grid complet (`AppState::SqlResultGrid`)
- La grille est en **lecture seule** (`read_only = true`) : navigation `j/k/h/l`, resize `[/]`, panel preview, collapse — tout fonctionne ; filtres et édition (`Enter`) sont désactivés
- La barre d'info affiche `SQL Result` ; la barre d'aide indique `q: back to editor`
- `q`/`Esc` retourne à l'éditeur SQL avec la requête et le résultat intacts

## [0.5.3] — 2026-06-15

### Added

#### Optimisation de l'affichage des colonnes
- **`MAX_COL_WIDTH` 25 → 40** : colonnes plus larges par défaut pour réduire les troncatures
- **Redimensionnement manuel** : `[` réduit la colonne de 5 (min 4), `]` l'agrandit de 5 (max 80) — état conservé dans `col_widths: HashMap<usize, u16>` ; réinitialisé à chaque nouveau chargement de table
- **Panel de prévisualisation** (2 lignes, fond `DarkGray`) entre la table et la barre d'aide : affiche `▸ col_name : valeur complète` de la cellule courante, sans troncature
- Barre d'aide mise à jour avec `[/]: resize`

### Fixed

- **EditRecord depuis FkGrid** : le nom de table dans la requête SQL générée (`UPDATE`) était le label d'affichage `"books [id=1]"` au lieu du vrai nom SQL `"books"` — résolu en séparant `table_name` (nom SQL pur) et `display_name` (label contextuel affiché dans la barre d'info)

## [0.5.2] — 2026-06-14

### Added

#### FK expandable rows (`AppState::FkGrid`)
- **Enter sur une cellule FK** (`[table_liée]`) ouvre une sous-grille affichant l'enregistrement lié via `SELECT * FROM ref_table WHERE ref_col = val`
- **Navigation récursive** : Enter sur une FK dans la sous-grille ouvre un nouveau niveau (profondeur illimitée) — pile `fk_history` conservant l'état de chaque niveau
- **Esc / q** remonte d'un niveau ; au niveau racine, retour à la Data Grid parente
- **Enter sur une cellule non-FK** dans la sous-grille ouvre EditRecord (avec retour correct à la sous-grille)
- Schéma et badges FK chargés sur chaque niveau → la récursivité fonctionne aussi sur les FK de 2ᵉ/3ᵉ niveau

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

[Unreleased]: https://github.com/TSODev/rowdy/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/TSODev/rowdy/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/TSODev/rowdy/compare/v0.5.8...v0.6.0
[0.5.8]: https://github.com/TSODev/rowdy/compare/v0.5.7...v0.5.8
[0.5.7]: https://github.com/TSODev/rowdy/compare/v0.5.6...v0.5.7
[0.5.6]: https://github.com/TSODev/rowdy/compare/v0.5.5...v0.5.6
[0.5.5]: https://github.com/TSODev/rowdy/compare/v0.5.4...v0.5.5
[0.5.4]: https://github.com/TSODev/rowdy/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/TSODev/rowdy/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/TSODev/rowdy/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/TSODev/rowdy/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/TSODev/rowdy/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/TSODev/rowdy/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/TSODev/rowdy/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/TSODev/rowdy/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/TSODev/rowdy/releases/tag/v0.1.0
