# Changelog

All notable changes to Rowdy are documented here.  
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [0.9.3] — 2026-06-19

### Added

- **Snippets SQL** — `Ctrl+P` ouvre une palette flottante centrée avec filtre live (saisie en temps réel), navigation `↑/↓`, `Enter` pour insérer le snippet dans l'éditeur, `D` pour supprimer, `Esc` pour fermer ; `Ctrl+S` dans l'éditeur ouvre un prompt « Save Snippet » pour nommer et sauvegarder la requête courante ; snippets persistés dans `~/.config/rowdy/snippets.toml` (upsert par nom) ; help bar dynamique adaptée selon le popup actif ; flash message de confirmation à la sauvegarde et à la suppression

### Fixed

- **Panneau schéma / ERD — MongoDB et Redis** : `schemas_loading` restait bloqué à `true` pour les connecteurs non-SQL — le panneau affichait « Loading schema… » indéfiniment et la touche `r` (ERD) était bloquée sur « Schema still loading… » ; nouveau champ `schemas_supported: bool` dans `TableListScreen` (défaut `true`, mis à `false` pour NoSQL et KV) : affichage immédiat de « Schema not available for this connector type »
- **Palette snippets — contraste** : ligne sélectionnée en `White Bold` sur fond `Magenta` (nom) et `LightCyan` sur `Magenta` (aperçu SQL) pour une meilleure lisibilité
- **Palette snippets — scroll** : la sélection n'était plus visible au-delà des premiers items ; `enumerate()` étant appelé avant `.skip()`, `display_idx` était déjà l'index absolu — ajouter `scroll_offset` en plus doublait le décalage et rendait `real_idx == p.selected` toujours faux après le premier scroll ; suppression de `real_idx`, utilisation directe de `abs_idx`

---

## [0.9.2] — 2026-06-19

### Fixed

- **Package crates.io** : fichiers `*.cast` et `*.pdf` exclus du package publié via `.gitignore` et le champ `exclude` dans `Cargo.toml` — réduit la taille du crate de 1.96 MB à 178 KB compressé
- **README** : remplacement du badge image asciinema (bloqué par la CSP de crates.io) par un lien texte vers la démo
- **Panneau schéma / ERD — MongoDB et Redis** : `schemas_loading` restait bloqué à `true` pour les connecteurs non-SQL car `spawn_load_all_schemas()` ne déclenchait rien mais ne réinitialisait jamais le flag — le panneau affichait « Loading schema… » indéfiniment, et la touche `r` (ERD) était bloquée sur « Schema still loading… » ; nouveau champ `schemas_supported: bool` dans `TableListScreen` (défaut `true`, mis à `false` pour NoSQL et KV) : le panneau affiche désormais « Schema not available for this connector type » et l'ERD « ERD not available for this connector type » immédiatement à la connexion

### Added

- **Snippets SQL** — `Ctrl+P` ouvre une palette flottante centrée avec filtre live (saisie en temps réel), navigation `↑/↓`, `Enter` pour insérer le snippet dans l'éditeur, `D` pour supprimer, `Esc` pour fermer ; `Ctrl+S` dans l'éditeur ouvre un prompt « Save Snippet » pour nommer et sauvegarder la requête courante ; snippets persistés dans `~/.config/rowdy/snippets.toml` (upsert par nom) ; help bar dynamique adaptée selon le popup actif ; flash message de confirmation à la sauvegarde et à la suppression
- **Palette snippets — contraste** : ligne sélectionnée en `White Bold` sur fond `Magenta` (nom) et `LightCyan` sur `Magenta` (aperçu SQL) au lieu de `Black/Magenta` illisible

---

## [0.9.1] — 2026-06-18

### Fixed

- **Recherche Ctrl+F** : les caractères `n` et `N` tapés dans le prompt déclenchaient la navigation au lieu de s'ajouter à la requête ; pendant que le prompt est ouvert, seules les flèches `↓`/`↑` naviguent — `n`/`N` reprennent leur rôle après `Enter` (mode nav)

---

## [0.9.0] — 2026-06-18

### Added

#### Recherche plein texte dans la Data Grid (Ctrl+F)
- `Ctrl+F` ouvre un prompt de recherche sur toutes les cellules chargées en mémoire
- Saisie en temps réel : calcule les correspondances dans toutes les colonnes (`value_display()`, case-insensitive) et saute au premier match depuis la position courante
- `n` / `↓` : match suivant (wrap) ; `N` / `↑` : match précédent
- `Enter` ferme le prompt tout en conservant le highlight vert sur toutes les correspondances et la navigation `n`/`N`
- `Esc` efface la recherche et tous les highlights
- Disponible en mode `read_only` et `prod_readonly`
- Recommandé après `A` (Load All) pour chercher dans la table entière
- `Ctrl+F: search` affiché dans la barre d'aide dans les trois variantes (normal, read-only, prod-readonly)

#### Persistance du curseur DataGrid entre rechargements
- Après un filtre, un tri ou une sauvegarde d'enregistrement, le curseur (ligne + colonne) reste sur la même cellule au lieu de revenir en haut
- `preserved_row: Option<usize>` dans `DataGridScreen` : sauvegardé par `reset_data()`, restauré par `set_result()` (clampé au nouveau nombre de lignes)
- `col_offset` (scroll horizontal) également préservé lors des rechargements
- Nouvelles ouvertures de table : comportement inchangé (ligne 0)

### Fixed

- **Recherche Ctrl+F** : les caractères `n` et `N` tapés dans le prompt s'ajoutaient à la requête mais déclenchaient aussi la navigation vers le match suivant/précédent ; pendant que le prompt est ouvert, seules les flèches `↓`/`↑` naviguent — `n`/`N` reprennent leur rôle de navigation uniquement après `Enter` (prompt fermé, mode nav)

---

## [0.8.5] — 2026-06-18

### Added

#### Tests d'intégration par connecteur (117 tests)
- Modules `#[cfg(test)]` inline dans chaque fichier connecteur (crate binaire sans `lib.rs`) — accès aux types privés sans `pub`
- Pattern uniforme : variable d'env optionnelle, skip gracieux si absente, `AtomicU32` pour noms uniques, `println!` visibles avec `--nocapture`
- **SQLite** — 14 tests sur `:memory:` (zéro infrastructure) : connect/disconnect, execute INSERT/UPDATE/erreur, fetch_all vide/count/scalaires/BLOB, TABLE vs VIEW, schema PK+FK via PRAGMA
- **PostgreSQL** — 14 tests (`POSTGRES_URL`) : casts inline `42::INT4`/`TRUE::BOOL` pour tester les types sans seed ; schema PK+FK+type_names ; TABLE vs VIEW ; `AtomicU32` pour tables `_rowdy_pg_test_N`
- **MySQL** — 17 tests (`MYSQL_URL`) : 4 tests purs `normalize_ssl_mode` (aucune DB requise) validant `REQUIRED`/`Required`/`required`/sans-param ; BOOLEAN → `tinyint` dans `information_schema` ; ENGINE=InnoDB pour FK ; tables `_rowdy_my_test_N`
- **Redis** — 15 tests (`REDIS_URL`) : 5 variantes `KvKeyDetail` (String/Hash/List/Set/ZSet) ; setup Hash/List/Set/ZSet via connexion brute `redis::aio` ; TTL = -1 (persistant) et TTL > 0 (expiry 300 s) ; clés préfixées `_rowdy_redis_test_N_*`
- **DuckDB** — 20 tests sur `:memory:` (zéro infrastructure) : 4 tests purs `parse_url` ; LIST → `NestedArray`, STRUCT → `NestedDoc` avec vérification JSON ; FK via `duckdb_constraints()` ; TABLE vs VIEW ; compilés uniquement avec `--features duckdb`
- **Turso** — 15 tests (`TURSO_URL`) : 3 tests purs `parse_url` (valid/token-vide/sans-authToken) ; schema FK via `PRAGMA foreign_key_list` ; VIEW via `sqlite_master` ; cleanup `DROP TABLE IF EXISTS` ; tables `_rowdy_ts_test_N`
- **MongoDB** — 22 tests (`MONGODB_URL`) : 7 tests purs `parse_filter` (vide/`{}`/objet/JSON invalide), `parse_pipeline` (valide/invalide), `id_to_bson` (ObjectId 24-hex/string) ; CRUD complet `insert_one`/`find`/`count`/`replace_one`/`delete_one` ; aggregate `$match + $count` ; pagination limit/offset ; `list_collections` ; cleanup via `get_db().collection.drop()` interne ; validé avec connexion X.509 MongoDB Atlas

---

## [0.8.4] — 2026-06-18

### Added

#### Onglets multiples (multi-tab sessions)
- `App` refactorisé : struct `Tab` isole tout l'état per-connexion (screens, client actif, historique, canaux `mpsc`, flags de reconnexion) ; `App { tabs: Vec<Tab>, active_tab: usize }` est un thin coordinator
- `Tab::new()` initialise un onglet avec un écran de connexion vierge (profils chargés depuis la config)
- `Ctrl+T` ouvre un nouvel onglet (écran de connexion indépendant)
- `[` / `]` navigue entre onglets — actif uniquement quand `tabs.len() > 1` et que l'état courant n'est pas un mode de saisie texte (SQL Editor, EditRecord) pour éviter les conflits de frappe
- `Ctrl+W` ferme l'onglet courant ; `q` en bas d'écran ferme l'onglet courant si plusieurs sont ouverts, ou quitte Rowdy si c'est le dernier
- `wants_close: bool` vs `should_quit: bool` dans `Tab` — `wants_close` déclenche la logique close/quit dans `App::run`, `should_quit` est réservé à la sortie absolue
- Barre d'onglets d'1 ligne affichée en haut lorsque ≥ 2 onglets sont ouverts ; onglet actif jaune gras, inactifs en blanc sur fond gris foncé ; nom dérivé de `connected_db_info` ou "New Tab"
- Drain de tous les canaux `db_rx` (un par onglet) à chaque tick de l'event loop — les requêtes async de n'importe quel onglet progressent en arrière-plan même si l'onglet n'est pas actif

#### Reconnexion automatique
- `is_connection_lost(msg: &str) -> bool` : détecte les pertes réseau par matching de mots-clés dans le message d'erreur (`connection reset`, `broken pipe`, `connection closed`, `server closed`, `lost connection`, `transport error`, `network error`, `connection timed out`, `unexpected eof`)
- Déclenchée sur `DbEvent::DataLoadFailed` / `TablesLoadFailed` / `QueryFailed` / `EditFailed` lorsque `is_connection_lost` retourne `true`
- `ReconnectInfo { url: String, db_type: String }` stocké dans `Tab` dès la connexion initiale réussie
- `Tab::spawn_reconnect(attempt)` : délai `Duration::from_secs(1u64 << attempt.min(2))` (1 s / 2 s / 4 s), puis reconnexion via la même factory `connect_sql/kv/nosql`
- 3 tentatives max (`reconnect_attempt: u8`) ; au-delà : `DbEvent::ReconnectFailed` + retour à l'écran de connexion
- `DbEvent::Reconnected(ActiveClient)` : remplace le client actif, relance le chargement des tables, flash "Reconnected" vert
- Badge `RECONNECTING…` fond jaune en barre de statut pendant les tentatives (`tab.reconnecting`)
- Failure flash : "Reconnect failed: …" rouge + `active_client = None` + retour à `AppState::Connection`

---

## [0.8.3] — 2026-06-17

### Added

#### Chiffrement des credentials (OS keyring)
- Les mots de passe et tokens stockés dans `~/.config/rowdy/config.toml` sont désormais chiffrés via le trousseau natif du système d'exploitation : Keychain macOS, libsecret Linux, Windows Credential Manager
- Activé par défaut via la feature `secure-storage` (désactivable avec `--no-default-features`)
- À l'enregistrement d'un profil (`Ctrl+S`), le secret est extrait de l'URL — mot de passe dans l'authority (`user:pass@host`) ou token en query param (`authToken=TOKEN`) — et stocké dans le keyring sous la clé `rowdy/<profile_name>`
- L'URL dans `config.toml` reçoit le placeholder `__keyring__` à la place du secret ; le fichier de configuration ne contient plus aucun secret en clair
- La résolution est transparente à la connexion : `spawn_connect` substitue le placeholder avant de passer l'URL au connecteur
- Suppression d'un profil (`D`) : l'entrée keyring est nettoyée automatiquement
- Fallback propre si le keyring est indisponible (headless Linux sans libsecret) : message d'erreur affiché en status bar, connexion annulée

### Fixed

#### Keyring — vérification write/read et upgrade v3 → v4
- Bug `keyring` v3 sur macOS Sequoia : `set_password` retournait `Ok(())` sans écrire réellement dans le trousseau natif, ce qui produisait `__keyring__` dans `config.toml` sans entrée correspondante — la connexion suivante échouait avec `No matching entry found`
- Crate `keyring` upgradée de v3.6.3 à v4 (meilleure compatibilité macOS Sequoia avec la nouvelle API `SecItem`)
- `store_in_keyring` vérifie maintenant l'écriture par un `get_password` immédiat après `set_password` : si la relecture échoue, l'URL originale (avec le mot de passe en clair) est conservée dans `config.toml` — le placeholder `__keyring__` n'est écrit que si le secret est effectivement récupérable

---

## [0.8.2] — 2026-06-17

### Added

#### Connecteur DuckDB (`--features duckdb`)
- Trait `SqlClient` implémenté via `duckdb-rs` 1.x (crate `duckdb` 1.10504.0, linking statique)
- Feature-gated `--features duckdb` — non inclus par défaut (linking C++ long à compiler)
- URL : `duckdb:///path/to/file.db` ou `duckdb://:memory:` (base en mémoire)
- Cas d'usage : analytique local, requêtes directes sur fichiers Parquet/CSV/JSON sans ETL
- Types complexes : `LIST`/`ARRAY` → `Value::NestedArray`, `STRUCT`/`MAP` → `Value::NestedDoc` (navigation par drill-in comme MongoDB)
- Types temporels : `DATE32` / `TIME64` / `TIMESTAMP` formatés via chrono
- PK détectées via `duckdb_constraints()` ; FK déclaratives détectées (voir bug connu ci-dessous)
- `sql_literal` génère la syntaxe array native DuckDB `['a', 'b']` pour les colonnes `VARCHAR[]`
- `spawn_blocking` autour de l'API synchrone `duckdb-rs` (pas de runtime async natif)
- ⚠️ **Bug moteur DuckDB v1.x** : UPDATE sur colonnes de type complexe (`VARCHAR[]`, `STRUCT`) avec FK entrantes → fausse violation de contrainte ; contournement : retirer les FK du schéma de seed (voir `seed/duckdb.sql` et section Bugs connus dans CLAUDE.md)

---

## [0.8.1] — 2026-06-17

### Added

#### CRUD complet MongoDB depuis le DataGrid
- **Insert** : touche `a` depuis le DataGrid MongoDB → `EditRecordScreen` avec les champs de la collection (types inférés depuis la première ligne existante) → `Ctrl+S` confirme via modal et appelle `insert_one`
- **Delete** : touche `D` depuis le DataGrid MongoDB → modal de confirmation `"Delete document with _id: …?"` → `delete_one` ; `N`/`Esc` annule
- Après insert ou delete : rechargement automatique de la collection (même chemin que `replace_one`)
- Help bar du DataGrid MongoDB affiche `a: insert   D: delete` (uniquement en mode non read-only)
- Preview panel "New Document Preview" affiche le JSON en cours de saisie en temps réel
- Note : si la collection est vide, `a` ne propose pas de champs — insérer d'abord via l'éditeur MQL

#### Édition inline JSON pour les champs `[obj]`
- Touche `i` sur un champ `[obj]` dans EditRecord → édition du JSON brut directement dans le champ texte (ex. `{"city":"Paris","zip":"75001"}`)
- Touche `Enter` sur `[obj]` → drill-in dans le sous-éditeur (comportement précédent)
- La help bar adapte son message selon la touche : `Enter: drill-in   i: edit JSON` quand le curseur est sur un champ objet

### Fixed

- Champs `[obj]`/`[arr]` dans l'écran d'insertion initialisés à `{}`/`[]` (au lieu de `""`) pour permettre le drill-in immédiat
- Types des champs inférés depuis la première ligne de la collection lors d'un insert (objet, tableau, int, float, bool, string) — évite de sérialiser `{"city":"Paris"}` comme une string JSON
- Sous-éditeurs d'objets (`is_nested`) : preview utilise maintenant `reconstruct_nested_json()` "Object Preview" au lieu de `build_mongo_replace()` qui échouait avec "No _id field"
- `Ctrl+S` dans un sous-éditeur (objet ou array) affiche désormais `"Esc: confirm & go back to parent"` au lieu de tenter une sauvegarde MongoDB sans `_id`

---

## [0.8.0] — 2026-06-17

### Added

#### Édition de documents MongoDB
- `insert_one`, `replace_one`, `delete_one` ajoutés au trait `NoSqlClient` et implémentés dans `MongoDbConnector`
- Helper `id_to_bson()` : reconstruit le bon type BSON pour le filtre `_id` (ObjectId 24-char hex ou string)
- `Enter` sur une ligne MongoDB ouvre un `EditRecordScreen` en mode `is_nosql` — schéma synthétique construit à la volée depuis les colonnes du résultat + types inférés (`string`, `int`, `float`, `bool`, `object`, `array`)
- Le champ `_id` est affiché avec le badge `[PK]` et non éditable
- Badges `[obj]` vert et `[arr]` vert dans l'éditeur pour les champs imbriqués (cohérence avec le DataGrid)
- `Ctrl+S` reconstruit le JSON du document (sans `_id`), ouvre un modal de confirmation, puis appelle `replace_one`
- Après sauvegarde : rechargement automatique de la collection via `find` + `count`
- Les grilles MongoDB ne sont plus `read_only` par défaut — le flag suit uniquement `prod_readonly` (URL `?readonly=true`)

#### Navigation imbriquée récursive dans EditRecord (objets)
- `Enter` sur un champ `[obj]` ouvre un sous-`EditRecord` pour le sous-document
- Le titre devient un breadcrumb : `users › address`, `users › address › city`
- `Esc` reconstruit le JSON de l'objet modifié et l'écrit dans le champ parent
- Pile `edit_record_stack: Vec<(EditRecordScreen, usize)>` dans `App` — profondeur illimitée
- `Ctrl+S` depuis un niveau imbriqué affiche `"Press Esc to confirm nested edit first"` — sauvegarde uniquement possible depuis la racine

#### Éditeur d'arrays item par item
- `Enter` sur un champ `[arr]` ouvre un éditeur liste numérotée `[0]`, `[1]`…
- `a` : ajoute un item vide en fin de liste et entre immédiatement en mode édition
- `D` : supprime l'item sélectionné et renumérote tous les suivants
- Les items scalaires s'éditent en inline (curseur), les items `[obj]`/`[arr]` font un drill-in récursif
- `Esc` reconstruit le JSON array et remonte dans le niveau parent
- Preview panel affiche le JSON array reconstruit en temps réel
- Barre d'aide dédiée : `j/k: item   Enter: edit   a: add   D: delete   Esc: confirm & back`

### Changed

#### Lisibilité du texte grisé dans l'UI
- Tous les `fg(Color::DarkGray)` remplacés par `fg(Color::Gray)` dans l'ensemble des fichiers UI — le texte secondaire (champs PK, valeurs NULL, preview SQL, barres d'aide) est nettement plus lisible sur fond noir
- Les `bg(Color::DarkGray)` (highlight d'édition, status bar) sont inchangés

#### Connecteur MongoDB (feature optionnelle)
- Nouveau trait `NoSqlClient` (`list_collections`, `find`, `aggregate`, `count`) dans `src/db/traits/nosql_client.rs`
- `MongoDbConnector` via `mongodb` 3 dans `src/db/connectors/mongodb.rs`, compilé uniquement avec `--features mongodb` ; dépendance absente par défaut pour ne pas pénaliser les autres utilisateurs
- Factory `connect_nosql()` dans `connectors/mod.rs` — retourne une erreur explicite (`"MongoDB support not compiled in — reinstall with --features mongodb"`) si le feature est absent
- `ActiveClient::NoSql(Arc<dyn NoSqlClient>)` et `DbEvent::NoSqlConnected` ajoutés dans `app.rs`
- `"mongodb"` intégré au sélecteur de type (Tab/←→) dans l'écran de connexion ; URL attendue : `mongodb://host:27017/dbname` (nom de DB obligatoire dans le path)
- Connexion avec ping de vérification ; `list_collections()` triée alphabétiquement affichée dans la vue liste
- `spawn_load_data` : `find("{}", PAGE_SIZE, 0)` + `count` parallèle, grille read-only
- `spawn_load_more` : pagination par offset sur `find`
- MQL editor (F5/Ctrl+Enter depuis la liste des collections) : filtre JSON `{…}` → `find`, pipeline JSON `[…]` → `aggregate` ; titre "MQL Editor │ … │ collection: nom" ; placeholder MQL dédié

#### Navigation dans les champs imbriqués MongoDB
- `Value::NestedDoc(String)` et `Value::NestedArray(String)` ajoutés à l'enum `Value` — portent le JSON sérialisé du sous-document ou du tableau BSON
- Badge vert `[obj]` sur les cellules contenant un objet BSON imbriqué ; badge vert `[arr:N]` avec compte des éléments pour les tableaux
- La preview bar (bande grise sous la grille) affiche le JSON réel du champ sélectionné au lieu de `{…}`
- `Enter` sur un badge (autorisé même en mode `read_only`) ouvre une sous-grille de navigation :
  - **Objet** → 1 ligne × N colonnes (une par clé)
  - **Tableau d'objets** → N lignes × union des clés
  - **Tableau scalaire** → colonnes `index` + `value`
  - Les valeurs imbriquées à l'intérieur produisent récursivement de nouveaux `NestedDoc`/`NestedArray`
- Breadcrumb dans la barre d'info : `users › address › city` construit via `display_name`
- Réutilise la pile `fk_history` et la navigation `Esc` existantes (aucun nouvel état `AppState`)
- Help bar en mode read-only complété : `Enter: explore`

#### Hooks pre-connect / post-disconnect par profil
- Champs optionnels `pre_connect` et `post_disconnect` dans `ConnectionProfile` (TOML) — rétrocompatibles (`skip_serializing_if = "Option::is_none"`)
- `pre_connect` : exécuté via `sh -c` **avant** l'établissement de la connexion DB — cas d'usage : ouvrir un tunnel SSH, activer un VPN, initialiser un proxy
  - Rowdy affiche "Running pre-connect script…" pendant l'exécution, puis "Connecting…" une fois terminé
  - Un code de retour non-zéro n'est pas bloquant : la connexion DB est tentée malgré tout (tunnel déjà ouvert, etc.)
- `post_disconnect` : exécuté via `sh -c` dans deux cas :
  - Retour à l'écran de connexion (`q` depuis la liste des tables) — fire & forget, non-bloquant
  - Fermeture de l'application (`Ctrl-C` ou `q`) — **attendu** avant la sortie pour une fermeture propre du tunnel
- Saisie directe dans l'écran de connexion (champs Pre-connect et Post-disconnect) et sauvegarde avec `Ctrl+S`

#### Édition d'un profil existant depuis l'écran de connexion
- `e` en mode Normal charge le profil sélectionné dans le panneau d'édition (type, URL, pre-connect, post-disconnect, nom)
- Le titre du panneau passe à **"Edit: nom_du_profil"** pour distinguer clairement édition et création
- `Ctrl+S` → écran de nom pré-rempli avec le nom existant ; `Enter` confirme, ou modifier le nom avant
- `Config::save_profile` match désormais par **nom en priorité**, puis par URL : un profil édité est toujours mis à jour en place même si l'URL change

#### Écran de connexion — navigation multi-champs
- Le panneau "New Connection" expose désormais 4 champs distincts : **Type**, **URL**, **Pre-connect script**, **Post-disconnect script**
- `Tab` cycle le focus entre les champs (Type → URL → Pre-connect → Post-disconnect → Type) ; champ actif surligné en jaune
- `←` / `→` (ou toute touche alphanumérique) change le type de BDD quand le champ **Type** est actif
- `Enter` se connecte avec l'URL et les scripts courants quel que soit le champ actif
- `Ctrl+S` sauvegarde l'ensemble (URL + scripts + nom de profil)

#### Vue ERD graphique (niveau 2)
- Touche `r` depuis la liste des tables ouvre une vue ERD centrée sur la table sélectionnée
- Layout en étoile : table centrale (encadré jaune) entourée des tables liées par FK
  - Gauche : tables dont une FK pointe vers la table centrale (incoming)
  - Droite : tables référencées par la table centrale (outgoing)
- Flèches `──►` avec coudes (`┐ └ ┌ ┘`) routées depuis la colonne FK exacte dans la boîte centrale
- Navigation `j/k` ou `Tab` pour cycler entre toutes les boîtes, `Enter` pour recentrer sur la boîte sélectionnée
- Retour à la liste avec `q` / `Esc`
- Badge `ERD` en barre de statut ; réutilise le schema déjà chargé (pas de requête supplémentaire)

#### Panneau schema/ERD intégré dans la liste des tables
- Le panneau droit de la liste des tables affiche le schema de la table sélectionnée en temps réel
- Chargement automatique en arrière-plan après connexion SQL via `get_schema()` sur toutes les tables
- Colonnes avec badges `[PK]` jaune / `[FK]` magenta, nom, type SQL, et flèche `──►` vers la table référencée
- Section **Outgoing FK** : clés étrangères qui partent de la table sélectionnée
- Section **Incoming FK** : clés étrangères d'autres tables qui pointent vers la table sélectionnée
- Non disponible pour les connecteurs KV (Redis) : liste pleine largeur conservée

### Fixed

#### Affichage des valeurs NUMERIC/DECIMAL/REAL — zéros trailing supprimés
- PostgreSQL `NUMERIC` et MySQL `DECIMAL` : `BigDecimal::to_string()` produisait des zéros superflus dus à l'encodage interne base-10000 (ex. `10.69` → `10.6900`)
- libsql/Turso `REAL` et floats PostgreSQL `FLOAT4`/`FLOAT8` : `{f:.4}` fixait 4 décimales même pour `1.0`
- Nouvelle règle : suppression des zéros trailing, minimum 2 décimales conservées
  - `10.6900` → `10.69`
  - `12.9000` → `12.90`
  - `1.0000` → `1.00`
  - `10.1234` → `10.1234` (aucune troncature de chiffres significatifs)
- Helper `format_decimal()` dans `db/types.rs` (PostgreSQL/MySQL) ; `format_float()` dans `data_grid.rs` (libsql/FLOAT)

#### Validation de format dans EditRecord
- À la sortie du mode édition (Esc/Enter), la valeur est validée contre le type SQL du champ
- Types contrôlés : `DATE` (YYYY-MM-DD), `TIME` (HH:MM:SS), `TIMESTAMP`/`DATETIME` (YYYY-MM-DD HH:MM:SS ou ISO8601), `UUID` (8-4-4-4-12 hex), `JSON`/`JSONB` (JSON parseable), `INT`/`BIGINT`, `FLOAT`/`NUMERIC`/`DECIMAL`, `INET`/`CIDR`
- Valeur invalide affichée en **rouge gras** dans la liste des champs
- Barre d'aide en **cyan** pendant l'édition : `Format: YYYY-MM-DD   ← →: cursor   Enter/Esc: done`
- Barre d'aide en **rouge** en mode navigation si le champ sélectionné est invalide : `✗ expected YYYY-MM-DD`
- `Ctrl+S` bloqué tant que des erreurs existent : `N field(s) with invalid format`
- Les champs vides et `NULL` ne sont pas validés (valeurs optionnelles autorisées)

#### Vue détail des clés Redis
- `Enter` sur une clé Redis dans la liste ouvre son contenu dans un Data Grid read-only
- Détection automatique du type via la commande Redis `TYPE` :
  - `string` → colonne `value` (1 ligne)
  - `hash` → colonnes `field` / `value` (trié par field)
  - `list` → colonnes `index` / `value` (ordre d'insertion)
  - `set` → colonne `member` (trié alphabétiquement)
  - `zset` → colonnes `member` / `score` (ordre croissant de score)
- TTL affiché dans la barre d'info : `session:abc [TTL: 3542s]` ou `[no expiry]`
- Toutes les fonctionnalités du Data Grid disponibles : navigation, preview panel, resize, export CSV/JSON
- Liste des clés triée alphabétiquement et affichée sans badge `[T]` (badges réservés aux connecteurs SQL)
- Script de seed `seed/redis.sh` — 79 clés couvrant les 5 types, thème librairie cohérent avec les seeds SQL

### Changed

#### Export JSON — choix simple vs résolution FK
- Le prompt d'export distingue désormais deux variantes JSON : `j = JSON   J = JSON+FK`
- `j` (minuscule) → JSON simple, synchrone, sans résolution des clés étrangères
- `J` (majuscule) → JSON avec résolution FK récursive jusqu'à 3 niveaux (comportement précédent de `j`)
- Depuis la vue SQL Result (`F4`), `j` et `J` produisent le même résultat (pas de schéma de table disponible)

## [0.7.1] — 2026-06-15

### Added

#### Distinction TABLE / VIEW dans la liste des tables
- La liste des tables affiche un badge `[T]` (gris) pour les tables et `[V]` (cyan) pour les vues
- Introspection via `information_schema.tables` (PostgreSQL, MySQL) ou `sqlite_master` (SQLite, Turso)
- Nouveau type `TableObject { name, kind: TableKind }` et méthode `get_table_objects()` sur le trait `SqlClient`
- L'ouverture d'une VIEW positionne automatiquement `prod_readonly = true` — l'édition est bloquée comme en mode read-only URL
- Badge `VIEW` cyan dans la barre de statut (distinct du badge `READ-ONLY` rouge lié à `?readonly=true`)

#### Export JSON avec résolution FK récursive
- La touche `j` dans le prompt d'export lance désormais une résolution asynchrone des clés étrangères
- Pour chaque colonne FK, la ligne référencée est récupérée et embarquée sous la clé `<col>__ref` dans le JSON
- Résolution récursive jusqu'à 3 niveaux (paramètre `max_depth`) : `order → customer__ref → address__ref`
- Détection des cycles via un ensemble de visites `(table.col=val)` — évite les boucles infinies
- Cache des schémas par table partagé sur toute la récursion — une seule requête `get_schema` par table référencée
- Les colonnes de type JSON/JSONB sont parsées et inlinées directement dans l'objet JSON
- Fallback sync (JSON simple sans résolution FK) pour les résultats SQL Editor (pas de schéma de table associé)
- Le résultat revient via `DbEvent::ExportDone` / `DbEvent::ExportFailed` — message flash dans la status bar à la fin

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

[Unreleased]: https://github.com/TSODev/rowdy/compare/v0.9.1...HEAD
[0.9.1]: https://github.com/TSODev/rowdy/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/TSODev/rowdy/compare/v0.8.5...v0.9.0
[0.8.5]: https://github.com/TSODev/rowdy/compare/v0.8.4...v0.8.5
[0.8.4]: https://github.com/TSODev/rowdy/compare/v0.8.3...v0.8.4
[0.8.3]: https://github.com/TSODev/rowdy/compare/v0.8.2...v0.8.3
[0.8.2]: https://github.com/TSODev/rowdy/compare/v0.8.1...v0.8.2
[0.8.1]: https://github.com/TSODev/rowdy/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/TSODev/rowdy/compare/v0.7.5...v0.8.0
[0.7.1]: https://github.com/TSODev/rowdy/compare/v0.7.0...v0.7.1
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
