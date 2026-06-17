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
| Document store | `mongodb` 3 (optionnel, `--features mongodb`) |
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

Les connecteurs sont partagés via `Arc<dyn SqlClient>` / `Arc<dyn KvClient>` / `Arc<dyn NoSqlClient>` pour permettre l'accès concurrent depuis les tâches tokio sans déplacer la propriété.

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

// src/db/traits/nosql_client.rs
#[async_trait]
pub trait NoSqlClient: Send + Sync {
    async fn connect(&mut self, url: &str) -> Result<(), DbError>;
    async fn disconnect(&mut self) -> Result<(), DbError>;
    async fn list_collections(&self) -> Result<Vec<TableObject>, DbError>;
    async fn find(&self, collection: &str, filter: &str, limit: u64, offset: u64) -> Result<DbQueryResult, DbError>;
    async fn aggregate(&self, collection: &str, pipeline: &str) -> Result<DbQueryResult, DbError>;
    async fn count(&self, collection: &str, filter: &str) -> Result<u64, DbError>;
    // Write operations
    async fn insert_one(&self, collection: &str, doc_json: &str) -> Result<String, DbError>;
    async fn replace_one(&self, collection: &str, id: &str, doc_json: &str) -> Result<u64, DbError>;
    async fn delete_one(&self, collection: &str, id: &str) -> Result<u64, DbError>;
}
```

### Flux de connexion async

```
User → Enter  →  ConnectionAction::Connect { url, db_type }
             →  App::spawn_connect()  →  tokio::spawn
                                              ↓ async
                                     connectors::connect_sql/kv/nosql()
                                              ↓
                                     DbEvent::SqlConnected / KvConnected / NoSqlConnected
                                              ↓ mpsc channel (≤ 50 ms)
                                     App::handle_db_event()
                                     → active_client = Some(Sql|Kv|NoSql(...))
                                     → AppState::TableList
                                     → spawn_load_tables()
                                              ↓ async
                                     get_table_objects() / keys("*") / list_collections()
                                              ↓
                                     DbEvent::TableObjectsLoaded / TablesLoaded
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
├── app.rs                         # machine à états + event loop (50 ms tick) [~1514 lignes]
├── config.rs                      # chargement ~/.config/rowdy/config.toml + redact_url + strip_readonly_param
├── export.rs                      # export CSV / JSON (avec résolution FK récursive)
├── history.rs                     # QueryHistory persisté dans ~/.config/rowdy/history.toml
├── events/
│   ├── app_event.rs               # AppEvent enum
│   └── handler.rs                 # dispatch clavier (stub)
├── db/
│   ├── error.rs                   # DbError (thiserror)
│   ├── types.rs                   # Column, Row, Value, DbQueryResult, KvKeyDetail, TableObject
│   ├── query_builder.rs           # build_data_query, build_where, build_fk_query, build_count_query,
│   │                              #   parse_count, is_select_query, split_sql_statements
│   ├── converters.rs              # kv_detail_to_result, json_to_result, json_val_to_value,
│   │                              #   json_value_type_and_str, json_object/array_to_schema_values,
│   │                              #   mongo_type_name, value_to_string
│   ├── traits/
│   │   ├── sql_client.rs          # trait SqlClient
│   │   ├── kv_client.rs           # trait KvClient
│   │   └── nosql_client.rs        # trait NoSqlClient
│   └── connectors/
│       ├── mod.rs                 # connect_sql() / connect_kv() / connect_nosql() factories
│       ├── postgres.rs            # ✅ implémenté
│       ├── sqlite.rs              # ✅ implémenté
│       ├── mysql.rs               # ✅ implémenté
│       ├── redis.rs               # ✅ implémenté (KvClient)
│       ├── turso.rs               # ✅ implémenté (libsql remote)
│       └── mongodb.rs             # ✅ implémenté (NoSqlClient, feature-gated)
└── ui/
    ├── layout.rs                  # dispatch draw() selon AppState
    ├── screens/
    │   ├── connection.rs          # ✅ implémenté
    │   ├── table_list.rs          # ✅ implémenté
    │   ├── data_grid.rs           # ✅ implémenté
    │   ├── sql_editor.rs          # ✅ implémenté (autocomplétion + SQL_KEYWORDS)
    │   ├── edit_record.rs         # ✅ implémenté
    │   └── erd_graph.rs           # ✅ implémenté
    └── components/
        ├── status_bar.rs          # ✅ implémenté
        └── modal.rs               # ✅ implémenté
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
- [x] Liste des tables : en-tête "Connected:" supprimé (doublon avec la status bar)
- [x] Modal de confirmation / erreur — overlay centré, `Ctrl+S` dans EditRecord ouvre confirmation avant UPDATE, erreurs de sauvegarde en modal d'erreur rouge, `Y`=confirme / `N`/`Esc`=annule
- [x] Mode read-only prod — `?readonly=true` (ou `&readonly=true`) dans l'URL, badge `READ-ONLY` rouge en status bar, bloque EditRecord + DML SQL editor, filtres/export conservés, reset à la déconnexion ; double `?` toléré (`strip_readonly_param` normalise en `&`)
- [x] Édition de profil depuis l'écran de connexion — touche `e` en mode Normal charge le profil sélectionné (type, URL, pre/post scripts, nom) dans le panneau d'édition ; titre "Edit: name" ; `Config::save_profile` match par nom en priorité puis par URL pour mise à jour en place
- [x] Hooks `pre_connect` / `post_disconnect` par profil — champs optionnels dans `ConnectionProfile` (TOML) ; `pre_connect` exécuté via `sh -c` avant `connect()` (tunnel SSH, VPN…) ; `post_disconnect` exécuté au retour à l'écran de connexion ET au quit de l'application (awaité pour fermeture propre) ; saisie directement dans l'écran de connexion (Tab → champ Pre-connect / Post-disconnect), sauvegardée avec `Ctrl+S`
- [x] Connecteur MongoDB (`--features mongodb`) — trait `NoSqlClient` (`list_collections`, `find`, `aggregate`, `count`) ; `MongoDbConnector` via `mongodb` 3 (feature-gated, non inclus par défaut) ; `connect_nosql()` factory avec message d'erreur clair si feature absent ; `"mongodb"` ajouté au sélecteur de type dans l'écran de connexion ; URL `mongodb://host:27017/dbname` (nom de DB requis dans le path)
- [x] Intégration MongoDB dans App — `ActiveClient::NoSql` + `DbEvent::NoSqlConnected` ; `spawn_load_tables` → `list_collections()` ; `spawn_load_data` → `find("{}", PAGE_SIZE, 0)` + `count` parallèle, grille read-only ; `spawn_load_more` → `find` paginé ; MQL editor (F5) : `[` en tête = `aggregate`, sinon `find` avec le texte comme filtre ; titre "MQL Editor │ … │ collection: nom" et placeholder MQL
- [x] Navigation champs imbriqués MongoDB — `Value::NestedDoc(json)` et `Value::NestedArray(json)` dans l'enum `Value` ; badges verts `[obj]` / `[arr:N]` dans le DataGrid (priorité sur badge FK magenta) ; preview bar affiche le JSON réel ; `Enter` sur badge (même en read-only) ouvre une sous-grille `FkGrid` avec le contenu converti en `DbQueryResult` (`json_to_result` : objet → 1 ligne × N cols, tableau d'objets → N lignes, scalaires → index+value) ; navigation récursive avec breadcrumb `collection › address › orders` via `display_name` ; `fk_history` / Esc remonte d'un niveau
- [x] Édition de documents MongoDB — `insert_one` / `replace_one` / `delete_one` sur `NoSqlClient` + `MongoDbConnector` ; helper `id_to_bson()` reconstruit l'`ObjectId` BSON depuis la string hex ; `Enter` sur une ligne ouvre `EditRecordScreen` en mode `is_nosql` avec schéma synthétique inféré depuis les colonnes + valeurs ; `_id` badge `[PK]` non éditable ; `Ctrl+S` → modal de confirmation → `replace_one` ; rechargement automatique de la collection après save ; MongoDB n'est plus `read_only` par défaut (suit `prod_readonly`)
- [x] Navigation imbriquée récursive dans EditRecord — `Enter` sur champ `object` → drill-in avec breadcrumb `collection › field` ; pile `edit_record_stack: Vec<(EditRecordScreen, usize)>` dans `App` ; `Esc` reconstruit le JSON et remonte ; `Ctrl+S` bloqué depuis un niveau imbriqué
- [x] Éditeur d'arrays item par item — `Enter` sur champ `array` → liste numérotée `[0]`, `[1]`… ; `a` ajoute un item vide (entre en mode édition immédiatement) ; `D` supprime et renumérote ; items `[obj]`/`[arr]` font un drill-in récursif ; `Esc` reconstruit le JSON array et remonte ; preview "Array Preview" en temps réel ; barre d'aide dédiée
- [x] Lisibilité UI — tous les `fg(Color::DarkGray)` remplacés par `fg(Color::Gray)` dans les fichiers UI (39 occurrences) ; texte secondaire nettement plus lisible sur fond noir ; `bg(Color::DarkGray)` inchangés
- [x] Insert MongoDB depuis DataGrid — touche `a` ouvre `EditRecordScreen` avec schéma inféré depuis la 1ère ligne (types réels : object/array/int/float/bool/string), `is_insert = true` ; champs `object`/`array` initialisés à `{}`/`[]` ; `Ctrl+S` → modal → `insert_one` ; preview "New Document Preview" en temps réel ; rechargement automatique
- [x] Delete MongoDB depuis DataGrid — touche `D` ouvre modal `"Delete document with _id: …?"` ; confirmé → `delete_one` ; rechargement automatique ; `a`/`D` dans la help bar DataGrid (mode non read-only MongoDB uniquement)
- [x] Édition inline JSON sur champs `[obj]` — `Enter` = drill-in (comportement existant) ; `i` = édition du JSON brut directement dans le champ texte ; help bar adaptée dynamiquement selon le type du champ courant
- [x] Fix sous-éditeurs MongoDB (`is_nested`) — preview "Object Preview" via `reconstruct_nested_json()` au lieu de `build_mongo_replace()` (qui échouait "No _id field") ; `Ctrl+S` dans un sous-éditeur → `"Esc: confirm & go back to parent"`
- [x] **Chiffrement des credentials** — feature `secure-storage` (dans `default`, désactivable via `--no-default-features`) ; crate `keyring = "4"` délègue au trousseau natif (Keychain macOS via `SecItem` API, libsecret Linux, Windows Credential Manager) ; à l'enregistrement d'un profil (`Config::save_profile`), `store_credential()` extrait le secret de l'URL (mot de passe dans l'authority `user:pass@host` ou token dans les query params `authToken=…`) et le stocke dans le keyring sous la clé `rowdy/<profile_name>` ; `store_in_keyring` vérifie l'écriture par `get_password` immédiat — si la relecture échoue, l'URL originale est conservée (pas de `__keyring__` orphelin) ; l'URL dans `config.toml` contient le placeholder `__keyring__` uniquement si l'écriture est confirmée ; à la connexion, `resolve_credential()` résout le placeholder avant de passer l'URL au connecteur ; `delete_credential()` nettoie le keyring à la suppression du profil ; `ConnectionAction::Connect` porte un `profile_name: Option<String>` (`Some` depuis un profil sauvegardé, `None` depuis la saisie libre) ; fallback propre : si le keyring échoue, message d'erreur en status bar sans crash ; fonctionne avec tous les connecteurs SQL/KV/NoSQL — ⚠️ bug `keyring` v3 sur macOS Sequoia : `set_password` retournait `Ok` sans écrire → résolu par upgrade v4 + vérification read-back
- [x] Refactoring `app.rs` (étape 1) — extraction des fonctions utilitaires vers des modules dédiés : `db/query_builder.rs` (8 fonctions SQL pures : build_where, build_data_query, build_count_query, build_fk_query, build_fk_count_query, parse_count, is_select_query, split_sql_statements), `db/converters.rs` (8 fonctions JSON/Value/KV : kv_detail_to_result, json_to_result, json_val_to_value, json_value_type_and_str, json_object/array_to_schema_values, mongo_type_name, value_to_string), `config.rs` (redact_url, strip_readonly_param) ; `app.rs` réduit de 1897 → 1514 lignes ; spawn_sql_page/spawn_sql_count et nested_info_from restent dans app.rs (dépendent de DbEvent/DataGridScreen)

### Roadmap

#### Différenciation (priorité haute)
- [x] **Vue schema/ERD FK (niveau 1 — panneau relations)** — panneau droit intégré dans `TableListScreen` (pas d'état séparé) ; chargement auto en arrière-plan après `TableObjectsLoaded` → `DbEvent::AllSchemasLoaded(HashMap<String, Vec<ColumnSchema>>)` stocké dans `table_list_screen.all_schemas` ; panneau gauche = liste tables (28 chars), panneau droit = colonnes avec badges [PK]/[FK] + sections "Outgoing FK / Incoming FK" avec flèches ASCII `──►` ; désactivé si KV store
- [x] **Vue schema/ERD FK (niveau 2 — boîtes + flèches ASCII)** — touche `r` depuis TableList → `AppState::ErdGraph` + `ErdGraphScreen` (`src/ui/screens/erd_graph.rs`) ; layout étoile : table centrale (jaune) au centre, tables incoming FK à gauche (cyan), tables outgoing FK à droite (cyan) ; `CharCanvas` 2D (char + Style) avec flèches coudées `┐/└/┌/┘` routées depuis la colonne FK exacte dans la boîte centrale ; navigation `j/k` cycle entre boîtes, `Enter` recentre sur la boîte sélectionnée, `q` retour TableList ; réutilise `all_schemas` chargé par le niveau 1
- [x] **Connecteur MongoDB** — trait `NoSqlClient`, driver `mongodb` 3, feature-gated `--features mongodb` ; navigation champs imbriqués avec badges verts + sous-grilles récursives (même pile `fk_history`)
- [x] **Mode read-only prod** — ✅ implémenté (voir section Fait)

#### Fonctionnel
- [x] Modal de confirmation / erreur — ✅ implémenté (voir section Fait)
- [x] **Gestion TABLE vs VIEW (niveau 1)** — `TableObject { name, kind: TableKind }` dans `db/types.rs` ; `get_table_objects()` sur le trait `SqlClient`, implémenté sur les 4 connecteurs SQL (PostgreSQL/MySQL : `information_schema.tables` ; SQLite/Turso : `sqlite_master`) ; badge `[T]` gris / `[V]` cyan dans la liste ; ouverture d'une VIEW → `prod_readonly = true` + `is_view = true` sur `DataGridScreen` → badge ` VIEW ` cyan en status bar, édition bloquée
- [ ] **Gestion TABLE vs VIEW (niveau 2 — DDL)** — _(priorité basse, scope élargi)_ CREATE/DROP TABLE et CREATE/DROP/ALTER VIEW depuis le TUI ; nécessite une UI DDL dédiée + confirmations renforcées (opérations destructives irréversibles) ; à concevoir indépendamment du browse
- [x] **Vue clé-détail Redis** — `Enter` sur une clé ouvre son contenu dans un Data Grid read-only ; `KvKeyDetail` enum (String/Hash/List/Set/ZSet) dans `db/types.rs` ; `get_key_detail()` + `ttl()` sur `KvClient` et `RedisConnector` ; conversion vers `DbQueryResult` via `kv_detail_to_result()` ; TTL en barre d'info ; liste triée alphabétiquement, sans badge `[T]` ; script `seed/redis.sh` (79 clés, 5 types)
- [ ] Tests d'intégration sur les connecteurs
- [ ] **Coloration syntaxique SQL** — _(bloqué : tui-textarea 0.5 ne supporte pas le highlighting multi-couleur natif ; à revoir lors d'un upgrade de tui-textarea ou ratatui)_
- [x] **Tri par colonne dans DataGrid** — touche `s` sur colonne courante → ORDER BY ASC/DESC/reset (cycle) ; indicateur `▲`/`▼` en vert dans l'en-tête ; ORDER BY injecté dans `build_data_query` (paramètre `order_by: Option<(&str, bool)>`) et propagé à `spawn_reload_filters`, `spawn_load_more`, `spawn_load_all`, `EditSaved` ; `sortable: bool` activé uniquement sur le DataGrid principal SQL ; la colonne sélectionnée est conservée après rechargement (`set_result()` ne remet plus `selected_col` à 0 — les nouveaux tableaux partent à 0 via `DataGridScreen::new()`)
- [x] **Load All dans DataGrid** — touche `A` (`sortable && has_more`) → `spawn_load_all()` : requête sans LIMIT (`limit = total_count.max(10_000)`) remplaçant toutes les pages ; résultat via `DbEvent::DataLoaded` ; aide bar affiche `A: load all` dynamiquement
- [x] **Autocomplétion SQL (Tab)** — `Tab` déclenche un popup flottant de suggestions si le préfixe courant (≥ 2 chars) matche des noms de tables/colonnes ou des mots-clés SQL ; navigation `↑/↓`/`Tab`, `Enter` insère (supprime le préfixe + insère la complétion via `editor.input()`), `Esc` ferme ; tout autre caractère met à jour le popup en temps réel ou le ferme si plus de match ; items schema alimentés par `DbEvent::AllSchemasLoaded` → `set_completions()` (tables + toutes colonnes, triés, dédupliqués) ; constante `SQL_KEYWORDS` (80 entrées : DML, DDL, clauses, agrégats, fonctions fenêtrées, types) dans `sql_editor.rs` ; schema items prioritaires (max 7), keywords en complément (max 10 total) ; matching case-insensitive, keywords retournés en MAJUSCULES ; popup clamped dans la zone de l'éditeur, s'affiche au-dessus si pas de place en dessous ; badge `N/total` dans le titre ; `widget::Clear` pour effacer le fond
- [ ] **Reconnexion automatique** — détection de `DbError::ConnectionLost` dans `handle_db_event` ; retry async avec back-off exponentiel (3 tentatives) ; badge `[RECONNECTING…]` en status bar
- [x] **Chiffrement des credentials dans config.toml** — ✅ implémenté (voir section Fait)
- [ ] **Lancement CLI externe (pgcli/mycli/litecli)** — _(priorité basse)_ touche `X` depuis `TableList` ou `SqlEditor` → détecte le type de connexion (postgres → `pgcli`, mysql → `mycli`, sqlite → `litecli`) ; suspend le TUI (`terminal::disable_raw_mode` + `LeaveAlternateScreen`), lance le CLI via `std::process::Command::new(cli).args([url]).spawn()?.wait()`, puis restaure le TUI (`EnterAlternateScreen` + `enable_raw_mode`) ; nécessite que le CLI soit installé dans `$PATH` ; non disponible pour Redis/MongoDB/DuckDB/Turso ; message d'erreur si CLI introuvable
- [x] **Connecteur DuckDB** — trait `SqlClient` via `duckdb-rs` 1.x (crate 1.10504.0) ; feature-gated `--features duckdb` ; URL `duckdb://path/to/file.db` ou `duckdb://:memory:` ; analytique local (Parquet, CSV, JSON) ; `spawn_blocking` autour de l'API synchrone ; types : LIST/ARRAY → `NestedArray`, STRUCT/MAP → `NestedDoc`, DATE32/TIME64/TIMESTAMP formatés via chrono ; PK via `duckdb_constraints()` ; FK déclaratives détectées ; `sql_literal` génère syntaxe array native `['a','b']` pour `VARCHAR[]` ; ⚠️ bug DuckDB : UPDATE de types complexes sur table parente FK échoue (voir section Bugs connus) — seed sans FK pour contourner
- [ ] **Connecteur Oracle** — _(priorité basse)_ trait `SqlClient` via crate `oracle` (odpi-c based, feature-gated `--features oracle`) ; URL `oracle://user:password@host:1521/service_name` ; nécessite Oracle Instant Client installé sur le système (variable `LD_LIBRARY_PATH` / `DYLD_LIBRARY_PATH`) ; schema explorer étendu : tables, views, packages, fonctions, procédures stockées via `ALL_OBJECTS` / `ALL_ARGUMENTS` ; types spécifiques Oracle : `NUMBER`, `DATE`, `CLOB`, `BLOB`, `RAW`, `ROWID` ; `sqlx` ne supporte pas Oracle — implémentation directe via `oracle::Connection`
- [ ] **Onglets multiples** — `Vec<Tab { name, active_client, state, screens }>` dans `App` ; `Ctrl+T` nouvelle connexion, `Ctrl+W` fermer, `Alt+1..9` / `[`/`]` naviguer ; tab bar en haut de l'écran
- [ ] **Snippets SQL** — `~/.config/rowdy/snippets.toml` ; `Ctrl+P` dans SQL editor ouvre palette ; saisie nom filtrée, `Enter` insère, `Ctrl+S` sauvegarde sélection courante comme snippet
- [ ] **Recherche globale dans DataGrid** — `Ctrl+F` ouvre prompt de recherche plein texte ; parcourt toutes les colonnes de la ligne courante et des suivantes ; highlight de la correspondance ; `n`/`N` suivant/précédent
- [x] **Validation de format dans EditRecord** — `validate_field()` sur sortie du mode édition ; types couverts : DATE/TIME/TIMESTAMP/UUID/JSON/INT/FLOAT/INET/CIDR ; valeur en rouge si invalide ; hint format en cyan pendant l'édition ; `Ctrl+S` bloqué si erreurs ; `format_hint()` + `is_valid_uuid()` dans `edit_record.rs`
- [x] **Export JSON avec résolution FK récursive** — `export_json_with_fk()` dans `src/export.rs` ; pour chaque colonne FK, récupère la ligne référencée et l'embarque en `<col>__ref` ; récursif jusqu'à 3 niveaux (paramètre `max_depth`), détection de cycles via `HashSet<(table.col=val)>`, cache des schémas per-table ; colonnes JSON/JSONB inlinées directement ; fallback sync pour SQL result grid (pas de schéma) ; le résultat revient via `DbEvent::ExportDone/ExportFailed` (canal mpsc existant).
- [x] **Affichage NUMERIC/DECIMAL/REAL** — `format_decimal()` dans `db/types.rs` (PostgreSQL NUMERIC + MySQL DECIMAL via BigDecimal) et `format_float()` dans `data_grid.rs` (libsql REAL + FLOAT4/FLOAT8) : suppression des zéros trailing, minimum 2 décimales conservées (`10.6900` → `10.69`, `12.9000` → `12.90`, `1.0000` → `1.00`)
- [x] **Export JSON — choix simple vs FK** — prompt d'export étendu : `j` = JSON simple (synchrone, sans FK), `J` = JSON avec résolution FK récursive (comportement précédent) ; `DataGridAction::ExportJsonSimple` ajouté ; depuis SQL Result Grid `j` et `J` sont équivalents (pas de schéma).

## Bugs connus / Limitations moteur

### DuckDB — FK violation sur UPDATE de types complexes (VARCHAR[], STRUCT)

**Symptôme** : `UPDATE "table" SET "col_array" = [...] WHERE "id" = N` échoue avec
`Constraint Error: Violates foreign key constraint because key "fk_col: N" is still referenced…`
même lorsque la PK n'est pas modifiée.

**Cause** : DuckDB v1.x traite l'UPDATE de colonnes de type complexe (`VARCHAR[]`, `STRUCT`, `MAP`)
comme un DELETE + INSERT en interne (stockage OLAP en colonnes). Lors du DELETE virtuel, le moteur
vérifie les contraintes FK entrantes et lève une violation si des lignes enfants existent — même si
la PK de la ligne parente n'a pas changé. Les scalaires (`INTEGER`, `VARCHAR`, `DECIMAL`, etc.)
sont mis à jour en place et ne déclenchent pas ce bug.

**Contournements testés — aucun ne fonctionne dans duckdb-rs 1.10504.0** :
- `PRAGMA foreign_keys = false` — ignoré ou non supporté
- `SET enable_foreign_keys = false` — paramètre inconnu du moteur
- `FOREIGN KEY ... NOT ENFORCED` — syntaxe rejetée par le parser

**Solution appliquée dans le seed `seed/duckdb.sql`** : FK retirées des `CREATE TABLE`.
Les relations sont documentées par convention de nommage (`author_id → authors.id`)
et des commentaires inline. La navigation FK Rowdy (badges magenta, sous-grilles)
n'est donc pas disponible sur les tables DuckDB du seed.

**Impact sur le connecteur** (`src/db/connectors/duckdb.rs`) :
- `get_schema()` cherche toujours les FK via `duckdb_constraints()` — fonctionnel si
  l'utilisateur définit ses propres FK dans son schéma (ex. bases sans types complexes).
- `execute()` tente deux retries (`SET enable_foreign_keys` puis `PRAGMA`) avant de
  propager l'erreur FK avec un message explicatif.
- `sql_literal()` dans `edit_record.rs` génère la syntaxe native `['a', 'b']` pour les
  types `VARCHAR[]`/`ARRAY` afin d'éviter le cast implicite string→array, mais cela ne
  suffit pas à contourner le bug DuckDB.

**À surveiller** : le bug est lié à l'implémentation du stockage OLAP columnar de DuckDB.
Tester à nouveau lors d'une mise à jour majeure de la crate `duckdb` (≥ 2.x ou version
avec support `NOT ENFORCED`).

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
- `DataGridScreen.table_name` = nom SQL brut ; `display_name: Option<String>` = label affiché (ex. `books [id=1]` pour FkGrid, `users › address` pour nested MongoDB)
- `Value::NestedDoc(json)` / `Value::NestedArray(json)` — variants produits par le connecteur MongoDB pour les sous-documents/tableaux BSON ; rendus comme badges verts dans le DataGrid ; `Enter` ouvre `json_to_result()` sans async
- `connect_nosql()` — factory dans `connectors/mod.rs`, toujours compilée ; retourne une erreur lisible si le feature `mongodb` n'est pas activé à la compilation
- `edit_record_stack: Vec<(EditRecordScreen, usize)>` — pile de navigation imbriquée dans `App` ; chaque entrée = (écran parent, index du champ imbriqué) ; `open_nested_edit_record(field_idx)` push, `pop_nested_edit_record()` pop + applique JSON reconstruit ; `reconstruct_nested_json()` pour les objets, `reconstruct_nested_array()` pour les tableaux
- `EditRecordScreen.is_nosql / is_array / is_insert / is_nested` — flags pour l'édition MongoDB : `is_nosql` active le mode MongoDB ; `is_insert` route `Ctrl+S` vers `build_mongo_insert()` + "New Document Preview" ; `is_array` active touches `a`/`D` + preview "Array Preview" + barre d'aide dédiée ; `is_nested` (sous-éditeurs créés par `open_nested_edit_record`) route la preview vers `reconstruct_nested_json()` "Object Preview" et bloque `Ctrl+S` avec message "Esc to go back" ; priorité preview : `is_array` > `is_nested` > `is_insert` > replace
