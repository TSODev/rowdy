# Rowdy — Guide d'utilisation

## Installation

### Depuis les sources

```bash
git clone https://github.com/TSODev/rowdy.git
cd rowdy
cargo build --release
./target/release/rowdy-db
```

### Depuis crates.io

```bash
cargo install rowdy-db
```

---

## Lancement

```bash
rowdy-db
```

Rowdy démarre directement sur l'**écran de connexion**.

---

## Configuration des profils

Créez le fichier `~/.config/rowdy/config.toml` pour enregistrer vos connexions :

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
name = "MongoDB Local"
type = "mongodb"
url = "mongodb://localhost:27017/mydb"

[[connections]]
name = "MongoDB Atlas"
type = "mongodb"
url = "mongodb+srv://user:password@cluster0.xxxxx.mongodb.net/mydb"
```

Les profils apparaissent dans le panneau gauche de l'écran de connexion au démarrage.

### Hooks pre-connect / post-disconnect

Chaque profil peut inclure des scripts shell optionnels exécutés automatiquement avant la connexion et après la déconnexion. Cas d'usage typique : tunnel SSH vers un serveur distant.

```toml
[[connections]]
name = "VPS Postgres (tunnel SSH)"
type = "postgres"
url = "postgres://user:password@localhost:5432/mydb"
pre_connect = "ssh -f -N -L 5432:localhost:5432 user@remote-host"
post_disconnect = "pkill -f 'ssh -L 5432:localhost:5432'"
```

| Champ | Comportement |
|-------|-------------|
| `pre_connect` | Exécuté via `sh -c` avant l'établissement de la connexion DB. Rowdy affiche "Running pre-connect script…" puis "Connecting…" une fois le script terminé. |
| `post_disconnect` | Exécuté via `sh -c` lors du retour à l'écran de connexion (`q` depuis la liste des tables) **et** à la fermeture de l'application (`Ctrl-C` ou `q`). À la fermeture, Rowdy attend la fin du script avant de quitter. |

> Les scripts sont exécutés même en cas d'échec (code de retour non-zéro). Si le tunnel SSH est déjà ouvert, `ssh -f` retourne une erreur — Rowdy continue la connexion DB malgré tout.

Ces champs peuvent être saisis directement depuis l'écran de connexion (voir ci-dessous) et sauvegardés avec `Ctrl+S`.

---

## Écran de connexion

```
┌─ Saved Profiles ────┬─ New Connection ──────────────────────┐
│                     │ ┌─ Type ──────────────────────────┐   │
│   [postgres] Local  │ │  < postgres >  (Tab to cycle)   │   │
│ > [sqlite]   Dev    │ └─────────────────────────────────┘   │
│   [redis]    Cache  │ ┌─ URL ───────────────────────────┐   │
│                     │ │  Press 'n' to enter a URL…      │   │
│                     │ └─────────────────────────────────┘   │
└─────────────────────┴───────────────────────────────────────┘
  j/k: move   Enter: connect   n: new   q: quit
```

### Mode Normal (navigation dans les profils)

| Touche | Action |
|--------|--------|
| `j` / `↓` | Profil suivant |
| `k` / `↑` | Profil précédent |
| `Enter` | Se connecter avec le profil sélectionné |
| `n` | Nouvelle connexion (panneau droit vierge) |
| `e` | **Éditer** le profil sélectionné — charge tous ses champs dans le panneau droit |
| `D` | Supprimer le profil sélectionné (avec confirmation) |
| `q` | Quitter Rowdy |
| `Ctrl-C` | Quitter Rowdy (toujours disponible) |

**Édition d'un profil (`e`)** : le panneau droit affiche le titre **"Edit: nom"** et pré-remplit les champs Type, URL, Pre-connect et Post-disconnect. Modifiez ce que vous souhaitez, puis :
- `Enter` → se connecte directement avec les valeurs modifiées
- `Ctrl+S` → ouvre le champ Nom (pré-rempli avec le nom existant) pour re-sauvegarder ; `Enter` confirme, ou changez le nom avant de valider

### Mode Saisie (`n`)

Le panneau de saisie comprend quatre champs navigables. Le champ actif est surligné en jaune.

| Touche | Action |
|--------|--------|
| `Tab` | Passer au champ suivant : Type → URL → Pre-connect → Post-disconnect → Type |
| `←` / `→` | Changer le type de BDD quand le champ **Type** est actif |
| _(frappe)_ | Saisir du texte dans le champ actif ; sur le champ Type, toute touche cycle le type |
| `Backspace` | Effacer un caractère dans le champ actif |
| `Enter` | Se connecter avec l'URL et les scripts saisis |
| `Ctrl+S` | Sauvegarder URL + scripts pré/post comme profil nommé |
| `Esc` | Annuler et revenir en mode Normal |

Les champs **Pre-connect** et **Post-disconnect** sont optionnels. Laissez-les vides si vous n'avez pas besoin de scripts.

### Mode Sauvegarde (`Ctrl+S`)

| Touche | Action |
|--------|--------|
| _(frappe)_ | Saisir le nom du profil |
| `Backspace` | Effacer un caractère |
| `Enter` | Enregistrer dans `~/.config/rowdy/config.toml` |
| `Esc` | Annuler et revenir en mode Saisie |

Si l'URL existe déjà dans le fichier, le profil est mis à jour (nom + type). Le profil sauvegardé apparaît immédiatement dans la liste de gauche et est sélectionné.

### Mode Confirmation de suppression (`D`)

La barre d'aide affiche en rouge : `Delete "nom"? y: delete from file   n: remove from list only   Esc: cancel`

| Touche | Action |
|--------|--------|
| `y` | Supprime le profil de la liste **et** du fichier de config |
| `n` | Supprime le profil de la liste seulement (fichier intact) |
| `Esc` | Annule, revient en mode Normal |

**Formats d'URL :**

| Type | Format |
|------|--------|
| PostgreSQL | `postgres://user:password@host:5432/dbname` |
| SQLite | `sqlite:///chemin/vers/fichier.db` ou `sqlite::memory:` |
| libsql / Turso | `libsql://your-db-org.turso.io?authToken=TOKEN` |
| MySQL | `mysql://user:password@host:3306/dbname` |
| Redis | `redis://host:6379` ou `redis://:password@host:6379` |
| MongoDB | `mongodb://user:password@host:27017/dbname` (nom de DB obligatoire) |

---

## Vue liste des tables

Après une connexion réussie, Rowdy charge automatiquement la liste des tables et vues (ou des clés pour Redis). Pour les connecteurs SQL, le schéma de toutes les tables est chargé en arrière-plan et affiché dans un **panneau schema à droite**.

```
┌─ Tables (12) ──────────┬─ books ──────────────────────────────────────────┐
│                        │                                                   │
│ > [T] authors          │  Columns                                          │
│   [T] books            │  [PK]  id                   integer               │
│   [T] orders           │  [FK]  author_id             integer  →authors.id  │
│   [V] v_summary        │        title                 text                  │
│                        │        created_at            timestamp             │
│                        │                                                   │
│                        │  Outgoing FK                                      │
│                        │  author_id  ──►  authors.id                       │
│                        │                                                   │
│                        │  Incoming FK                                      │
│                        │  orders.book_id  ──►  books.id                    │
└────────────────────────┴───────────────────────────────────────────────────┘
  j/k: move   Enter: open   e: SQL editor   r: ERD   /: filter   q: disconnect
```

### Panneau schema (droite)

Le panneau droit affiche en temps réel le schéma de la table sélectionnée :
- **Colonnes** avec badges `[PK]` jaune, `[FK]` magenta, type SQL en gris
- **Outgoing FK** : clés étrangères qui partent de cette table
- **Incoming FK** : clés étrangères d'autres tables qui pointent vers cette table

> Le chargement du schéma s'effectue une seule fois à la connexion, en arrière-plan (le panneau affiche "Loading schema…" pendant ce temps). Non disponible pour Redis.

### Badges TABLE / VIEW (connecteurs SQL uniquement)

| Badge | Couleur | Signification |
|-------|---------|---------------|
| `[T]` | Gris | Table normale — navigation et édition disponibles |
| `[V]` | Cyan | Vue SQL (`VIEW`) — lecture seule, édition bloquée |

L'ouverture d'une vue active automatiquement le mode lecture seule : la barre de statut affiche le badge **`VIEW`** cyan. `Enter` sur une ligne n'ouvre pas l'écran d'édition.

> **Redis** : les clés s'affichent sans badge, triées alphabétiquement. `Enter` ouvre la vue détail de la clé (voir ci-dessous).

### Navigation

| Touche | Action |
|--------|--------|
| `j` / `↓` | Table suivante |
| `k` / `↑` | Table précédente |
| `Enter` | Ouvrir la table dans le Data Grid |
| `e` | Ouvrir l'éditeur SQL |
| `r` | Ouvrir la vue ERD graphique centrée sur la table sélectionnée |
| `/` | Activer le filtre |
| `q` / `Esc` | Se déconnecter et revenir à l'écran de connexion |

### Filtre (`/`)

| Touche | Action |
|--------|--------|
| _(frappe)_ | Filtrer les tables par nom (insensible à la casse) |
| `Backspace` | Effacer un caractère |
| `Enter` | Valider le filtre et revenir en navigation |
| `Esc` | Annuler le filtre |

Le compteur affiche `Tables (N / total)` quand un filtre est actif.

---

## Data Grid

Depuis la vue liste des tables, appuyez sur `Enter` pour ouvrir une table.

```
 users │ row 2/3 │ col 2/4 │ 3/150 rows  [name≈ob]
┌─────┬──────────────────┬─────────────────────┬ cr… ┬──────────────────────┐
│ id  │ name             │ email               │  …  │ created_at           │
├─────┼──────────────────┼─────────────────────┼─────┼──────────────────────┤
│   2 │ Bob              │ bob@example.com     │  …  │ 2024-02-20 14:10:00  │
└─────┴──────────────────┴─────────────────────┴─────┴──────────────────────┘
  j/k: rows   h/l: cols   g/G: first/last   Space: collapse   f: filter   F: clear   q: back
```

### Navigation

| Touche | Action |
|--------|--------|
| `j` / `↓` | Ligne suivante — charge la page suivante si dernière ligne atteinte |
| `k` / `↑` | Ligne précédente |
| `h` / `←` | Colonne précédente |
| `l` / `→` | Colonne suivante |
| `g` | Première ligne |
| `G` | Dernière ligne chargée |
| `PgDown` | +10 lignes |
| `PgUp` | -10 lignes |
| `Space` | Réduire / agrandir la colonne sélectionnée (collapse) |
| `-` | Réduire la largeur de la colonne sélectionnée (−5, min 4) |
| `=` | Agrandir la largeur de la colonne sélectionnée (+5, max 80) |
| `Enter` | Cellule FK → ouvre la sous-grille liée ; cellule normale → édition de la ligne |
| `E` | Ouvrir le prompt d'export (puis `c`=CSV, `j`=JSON, `J`=JSON+FK, `Esc`=annuler) |
| `q` / `Esc` | Retour à la liste des tables |

### Colonnes

- La **colonne sélectionnée** est indiquée par un en-tête souligné en jaune (`h/l` pour naviguer).
- Les **colonnes filtrées** sont mises en évidence en cyan dans l'en-tête.
- `Space` **collapse** une colonne à 3 caractères pour gagner de la place, ou la **restaure**.
- `-` / `=` ajuste finement la largeur par pas de 5 caractères (min 4, max 80) — la valeur est mémorisée pour la session.
- Les colonnes défilent automatiquement pour garder la colonne sélectionnée toujours visible.
- La largeur naturelle est calculée d'après le contenu (max 40 caractères). Valeurs longues tronquées avec `…`.

### Prévisualisation de cellule

Une barre de 2 lignes (fond gris) est affichée entre la table et la barre d'aide. Elle indique :

```
 ▸ col_name : valeur complète sans troncature
```

Elle se met à jour en temps réel lors du déplacement de curseur.

### Pagination (infinite scroll)

Les données sont chargées par pages de **200 lignes**. Appuyer sur `j` à la dernière ligne déclenche automatiquement le chargement de la page suivante. La barre d'info affiche `chargé/total rows` dès que le `COUNT(*)` est disponible, et `N+ rows` dans l'intervalle.

### Filtres cumulatifs

| Touche | Action |
|--------|--------|
| `f` | Ouvrir la saisie de filtre pour la colonne sélectionnée |
| `Enter` | Appliquer le filtre (recharge depuis le début) |
| `Esc` | Annuler la saisie sans appliquer |
| `d` | Supprimer le filtre de la colonne courante et recharger |
| `F` | Effacer tous les filtres et recharger |

Les filtres utilisent `LIKE '%valeur%'` et s'accumulent sur plusieurs colonnes (AND). La valeur de filtre active s'affiche dans la barre d'info : `[name≈bob] [email≈@gmail]`.

> **Note :** `LIKE` est sensible à la casse sur PostgreSQL. Sur SQLite et MySQL les comparaisons ASCII sont insensibles à la casse. Pour les colonnes non-texte (entiers, dates), PostgreSQL peut retourner une erreur — filtrer de préférence sur des colonnes de type texte.  
> Redis n'est pas supporté dans le Data Grid (utiliser l'éditeur SQL).

### Cellule sélectionnée

La cellule courante (intersection ligne × colonne) est mise en évidence en **bleu**. Le reste de la ligne sélectionnée est jaune.

- Cellule **normale** → `Enter` ouvre l'écran d'édition de l'enregistrement.
- Cellule **FK** (badge magenta `[table_liée]`) → `Enter` ouvre une sous-grille affichant l'enregistrement lié.

### Clés étrangères (FK badges)

Les colonnes reconnues comme clés étrangères affichent un badge magenta :

```
│  1  │ 3 [orders] │ 7 [books] │ 2 │ 29.99 │
```

`Enter` sur une telle cellule ouvre une **sous-grille FK**.

---

## Sous-grille FK

Lorsque vous pressez `Enter` sur une cellule FK, Rowdy exécute `SELECT * FROM table_liée WHERE col = valeur` et affiche le résultat dans une sous-grille. La barre d'info indique le contexte : `books [id=3]`.

La sous-grille FK fonctionne comme le Data Grid normal :

| Touche | Action |
|--------|--------|
| `j` / `k` | Ligne suivante / précédente |
| `h` / `l` | Colonne suivante / précédente |
| `-` / `=` | Redimensionner la colonne |
| `Enter` | Cellule FK → niveau FK suivant (récursif) ; cellule normale → édition |
| `Esc` / `q` | Remonter d'un niveau (ou retour au Data Grid si niveau racine) |

La navigation FK est **récursive** : chaque `Enter` sur un badge FK empile un nouveau niveau. `Esc` dépile et remonte d'un niveau à la fois.

---

## Édition d'un enregistrement

Depuis le Data Grid, appuyez sur `Enter` pour ouvrir la vue d'édition de la ligne sélectionnée.

```
┌─ Edit: books ──────────────────────────────────────────────────────┐
│   id           [PK]      integer         5                         │
│ > title                  character var…  The Great Journey — vol.5 │
│   author_id    [→authors]integer         3                         │
│   isbn                   text            978-2-07-036024-5         │
└────────────────────────────────────────────────────────────────────┘
┌─ SQL Preview ──────────────────────────────────────────────────────┐
│  UPDATE "books" SET "title" = 'The Great Journey — vol. 5'        │
│  WHERE "id" = 5                                                    │
└────────────────────────────────────────────────────────────────────┘
  j/k: field   Enter/i: edit   Space: toggle bool   Ctrl+S: save   Esc: back
```

### Navigation (mode Normal)

| Touche | Action |
|--------|--------|
| `j` / `↓` | Champ suivant |
| `k` / `↑` | Champ précédent |
| `Enter` / `i` | Activer l'édition du champ sélectionné |
| `Space` | Basculer `true` ↔ `false` sur un champ booléen (sans passer en mode édition) |
| `Ctrl+S` | Sauvegarder (exécute l'UPDATE et recharge la grille) |
| `Esc` / `q` | Retour au Data Grid sans sauvegarder |

### Édition d'un champ (mode Édition)

| Touche | Action |
|--------|--------|
| _(frappe)_ | Insérer un caractère à la position du curseur |
| `←` / `→` | Déplacer le curseur |
| `Home` | Aller au début |
| `End` | Aller à la fin |
| `Backspace` | Effacer le caractère avant le curseur |
| `Delete` | Effacer le caractère après le curseur |
| `Enter` / `Esc` | Valider et revenir en mode Normal |

### Badges

| Badge | Signification |
|-------|---------------|
| `[PK]` (cyan) | Clé primaire — lecture seule, non éditable |
| `[→table]` (magenta) | Clé étrangère pointant vers `table` |

### Colonne type

Le type SQL brut du champ est affiché en bleu entre le badge et la valeur (`integer`, `character varying`, `boolean`, `timestamp with time zone`…). Les types longs sont tronqués à 16 caractères.

### Types supportés et validation

| Type | Comportement | Format attendu |
|------|-------------|----------------|
| `INT`, `BIGINT`, `SMALLINT` | SQL sans guillemets | entier valide |
| `FLOAT`, `NUMERIC`, `DECIMAL` | SQL sans guillemets | nombre valide |
| `BOOLEAN` | `Space` pour toggler ; SQL génère `TRUE`/`FALSE` | — |
| `DATE` | Édition texte + validation | `YYYY-MM-DD` |
| `TIME` | Édition texte + validation | `HH:MM:SS` |
| `TIMESTAMP`, `DATETIME` | Édition texte + validation | `YYYY-MM-DD HH:MM:SS` ou ISO8601 |
| `UUID` | Édition texte + validation | `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx` |
| `JSON`, `JSONB` | Édition texte + validation | JSON valide |
| `INET`, `CIDR` | Édition texte + validation | adresse IP |
| `TEXT`, `VARCHAR`… | Édition texte standard | — |

**Validation :** à la sortie du mode édition (Esc/Enter), la valeur est validée contre le type du champ :
- La barre d'aide affiche le format attendu en **cyan** pendant la saisie : `Format: YYYY-MM-DD`
- Une valeur invalide s'affiche en **rouge gras** ; la barre d'aide indique l'erreur : `✗ expected YYYY-MM-DD`
- `Ctrl+S` est bloqué tant que des champs invalides existent : `N field(s) with invalid format`
- Les valeurs vides et `NULL` ne sont pas validées

Les champs modifiés sont surlignés en **vert**. L'aperçu SQL se met à jour en temps réel et n'affiche `-- No changes` que si aucune valeur n'a été modifiée.

> **Note :** si la table ne possède pas de clé primaire, `Ctrl+S` affiche `-- No primary key` et la sauvegarde est impossible.

---

## Éditeur SQL

Depuis la vue liste des tables, appuyez sur `e` pour ouvrir l'éditeur SQL.

```
┌─ SQL Editor │ [sqlite] sqlite:///dev.db ────────────────────────┐
│SELECT id, name                                                   │
│FROM users                                                        │
│WHERE id > 1                                                      │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
┌─ Results: 2 rows ────────────────────────────────────────────────┐
│  id   name                                                       │
│> 2    Bob                                                        │
│  3    Charlie                                                    │
└──────────────────────────────────────────────────────────────────┘
  F5 / Ctrl+Enter: execute   Tab: results pane   Ctrl+Q: back
```

### Mode Éditeur (focus jaune sur la zone SQL)

| Touche | Action |
|--------|--------|
| _(frappe)_ | Saisir du SQL multi-lignes |
| `F5` | Exécuter la requête |
| `Ctrl+Enter` | Exécuter la requête |
| `Alt+↑` | Rappeler la requête précédente depuis l'historique |
| `Alt+↓` | Rappeler la requête suivante (vide = effacer) |
| `F4` | Ouvrir le résultat SELECT dans le Data Grid complet |
| `Tab` | Basculer vers le panneau Résultats (si résultat disponible) |
| `Ctrl+Q` | Retour à la liste des tables |

Toutes les touches d'édition standard sont supportées : flèches, `Backspace`, `Delete`, `Home`, `End`, `Ctrl+A` (tout sélectionner), `Ctrl+Z` (annuler), copier/coller selon le terminal.

### Historique des requêtes

Chaque requête exécutée est sauvegardée automatiquement dans `~/.config/rowdy/history.toml` (jusqu'à 200 entrées, dédoublonnées). Utilisez `Alt+↑` pour remonter dans l'historique et `Alt+↓` pour revenir vers les requêtes plus récentes. Revenir à zéro avec `Alt+↓` efface l'éditeur.

Le curseur d'historique se réinitialise dès qu'une nouvelle requête est exécutée.

### Mode Résultats (focus jaune sur le tableau)

| Touche | Action |
|--------|--------|
| `j` / `↓` | Ligne suivante |
| `k` / `↑` | Ligne précédente |
| `h` / `←` | Décaler les colonnes vers la gauche |
| `l` / `→` | Décaler les colonnes vers la droite |
| `g` | Première ligne |
| `G` | Dernière ligne |
| `PgDown` | +10 lignes |
| `PgUp` | -10 lignes |
| `F4` | Ouvrir le résultat dans le Data Grid complet |
| `Tab` / `Esc` | Retour dans l'éditeur |

### Data Grid en lecture seule (`F4`)

`F4` transfère le résultat du SELECT dans un Data Grid complet (`SQL Result`) avec toutes les fonctionnalités de navigation : `j/k/h/l`, resize `-/=`, panel preview, collapse. Les filtres et l'édition sont désactivés. `q`/`Esc` retourne à l'éditeur SQL avec la requête et le résultat intacts. L'export (`E`) est disponible depuis ce mode.

### Détection automatique SELECT / DML

- Requêtes commençant par `SELECT`, `WITH`, `EXPLAIN`, `SHOW`, `DESCRIBE`, `PRAGMA` → `fetch_all` (résultats tabulaires)
- Autres requêtes (`INSERT`, `UPDATE`, `DELETE`, `CREATE`, …) → `execute` (affiche le nombre de lignes affectées)

> **Note :** l'éditeur SQL est uniquement disponible avec les connecteurs SQL (PostgreSQL, SQLite, MySQL).  
> Redis n'est pas supporté (pas de modèle relationnel).

---

## Export CSV / JSON

Depuis n'importe quelle grille de données (Data Grid, sous-grille FK, SQL Result), appuyez sur `E` pour exporter les données chargées.

```
 Export:  c = CSV   j = JSON   J = JSON+FK   Esc = cancel
```

| Touche | Action |
|--------|--------|
| `c` | Exporter en CSV (RFC 4180 : guillemets si la valeur contient une virgule, un guillemet ou un saut de ligne) |
| `j` | Exporter en JSON simple (tableau d'objets, sans résolution des FK) |
| `J` | Exporter en JSON avec résolution FK récursive (voir ci-dessous) |
| `Esc` | Annuler |

Le fichier est écrit dans votre répertoire personnel : `~/rowdy_<table>_<timestamp>.csv` ou `.json`.

La status bar confirme le nom du fichier créé : `Saved: ~/rowdy_books_1718453421.json`

### Export JSON avec résolution FK (`J`)

Lorsqu'un schéma de table est disponible (Data Grid ou sous-grille FK), `J` résout les clés étrangères. Pour chaque colonne FK, la ligne référencée est récupérée dans la base et embarquée sous la clé `<colonne>__ref` :

```json
[
  {
    "id": 1,
    "title": "Dune",
    "author_id": 3,
    "author_id__ref": {
      "id": 3,
      "first_name": "Frank",
      "last_name": "Herbert",
      "country": "US"
    },
    "category_id": 2,
    "category_id__ref": {
      "id": 2,
      "label": "Science Fiction"
    }
  }
]
```

La résolution est **récursive jusqu'à 3 niveaux** : si la table liée contient elle-même des FK, elles sont également résolues (`order → customer__ref → address__ref`). Les cycles (`A → B → A`) sont détectés et stoppés automatiquement.

L'export s'effectue de manière **asynchrone** : l'interface reste réactive pendant les requêtes de résolution. La status bar affiche "JSON export with FK resolution in progress…" puis "Saved: ~/rowdy_…json" à la fin.

> **Note :** `J` (résolution FK) n'est disponible que depuis les Data Grid et FK Grid (schéma de table connu). Depuis la vue SQL Result (`F4`), `j` et `J` produisent le même résultat : un tableau JSON simple sans résolution FK.

> **Note :** l'export porte sur les données actuellement chargées en mémoire (jusqu'à la page en cours pour le scroll infini). Pour exporter une table complète, chargez toutes les pages avant d'exporter.

---

## Mode read-only (production)

Pour se connecter en lecture seule et bloquer toute modification accidentelle, ajoutez `?readonly=true` (ou `&readonly=true` si d'autres paramètres sont déjà présents) à l'URL de connexion.

### Activation

**Saisie manuelle :**
```
postgres://user:pass@prod-host/mydb?readonly=true
```

**Dans le fichier de config :**
```toml
[[connections]]
name = "Production (read-only)"
type = "postgres"
url = "postgres://user:pass@prod-host/mydb?readonly=true"
```

Si d'autres paramètres existent déjà, utilisez `&` :
```
postgres://user:pass@prod-host/mydb?sslmode=require&readonly=true
```

Le paramètre `readonly=true` est strippé de l'URL avant connexion — il n'est pas transmis au driver.

### Indicateur visuel

La barre de statut affiche un badge rouge permanent :

```
 DATA GRID   ● [postgres] prod-host/mydb   READ-ONLY
```

### Ce qui est bloqué

| Action | Comportement |
|--------|-------------|
| `Enter` sur une cellule (Data Grid / FK View) | Message d'erreur flash, aucune ouverture de l'écran d'édition |
| `INSERT`, `UPDATE`, `DELETE`, `DROP`, `CREATE`… dans l'éditeur SQL | Message d'erreur rouge, requête non exécutée |

### Ce qui reste disponible

Navigation, filtres, pagination infinite scroll, redimensionnement de colonnes, prévisualisation de cellule, sous-grilles FK, export CSV/JSON — tout fonctionne normalement.

### Désactivation

Le mode read-only est lié à la session de connexion. Se déconnecter (`q` depuis la liste des tables) le réinitialise automatiquement.

---

## Barre de statut

Une ligne permanente est affichée en bas de l'écran depuis tous les écrans. Elle indique :

```
 DATA GRID   ● [postgres] postgres://user@localhost/my_db  [1 247 rows]
```

| Élément | Description |
|---------|-------------|
| Badge mode | Écran actif (`CONNECTION`, `TABLES`, `DATA GRID`, `FK VIEW`, `EDIT`, `SQL EDITOR`, `QUERY RESULT`, `ERD`) |
| `●` vert / `○` rouge | Connecté / déconnecté |
| Info DB | Type de BDD + URL (mot de passe et tokens masqués, ex. `user:***@host`, `authToken=***`) |
| `[N rows]` | Nombre total de lignes (DataGrid / FK View / SQL Result seulement) |
| Badge `VIEW` (fond cyan) | Vue SQL ouverte — édition bloquée (les vues sont en lecture seule) |
| Badge `READ-ONLY` (fond rouge) | Connexion en mode lecture seule — toute écriture est bloquée |
| Message flash | Confirmation (vert) ou erreur (rouge) pendant ~4 secondes après une action |

---

## Raccourcis globaux

| Touche | Action |
|--------|--------|
| `Ctrl-C` | Quitter Rowdy depuis n'importe quel écran |
| `q` | Quitter / reculer (selon le contexte) |

---

## Vue détail d'une clé Redis

Depuis la liste des clés Redis, appuyez sur `Enter` pour ouvrir le contenu de la clé sélectionnée dans un Data Grid read-only.

Rowdy détecte automatiquement le type Redis de la clé et adapte l'affichage :

| Type Redis | Colonnes | Description |
|------------|----------|-------------|
| `string`   | `value`  | Valeur brute — 1 ligne |
| `hash`     | `field` / `value` | Un champ par ligne, trié alphabétiquement par field |
| `list`     | `index` / `value` | Éléments dans l'ordre d'insertion (0-indexé) |
| `set`      | `member` | Membres triés alphabétiquement |
| `zset`     | `member` / `score` | Membres par score croissant |

La barre d'info affiche le nom de la clé et son TTL :

```
 session:a1b2c3d4 [TTL: 3542s]     counter:books:total [no expiry]
```

Toutes les fonctionnalités de navigation du Data Grid sont disponibles : `j/k/h/l`, resize `-/=`, panel de prévisualisation, collapse `Space`, export `E` (CSV ou JSON). `q` retourne à la liste des clés.

> **Note :** la vue détail est en lecture seule — l'édition et les filtres sont désactivés.

---

## Vue ERD graphique

Depuis la liste des tables, appuyez sur `r` pour ouvrir la vue ERD centrée sur la table sélectionnée.

```
┌──────────────┐                    ┌──────────────┐
│  authors     │                    │    books     │
├──────────────┤                    ├──────────────┤
│[PK] id       ├────────────────────►[PK] id       │
│     name     │    author_id       │[FK] author_id│──────┐
└──────────────┘                    │     title    │      │
                                    └──────────────┘      │
                                                          │  ┌──────────────┐
                                    ┌──────────────┐      │  │   genres     │
                                    │   orders     │      └──►──────────────│
                                    ├──────────────┤         │[PK] id       │
                ┌───────────────────►[PK] id       │         │     label    │
                │                   │[FK] book_id  │         └──────────────┘
                │                   └──────────────┘
```

- **Table centrale** (encadré jaune) : la table sélectionnée avec toutes ses colonnes
- **Gauche** (Incoming FK) : tables dont une FK pointe vers la table centrale
- **Droite** (Outgoing FK) : tables référencées par la table centrale
- Les **flèches** partent depuis la ligne de la colonne FK exacte dans la table centrale

### Navigation

| Touche | Action |
|--------|--------|
| `j` / `k` ou `Tab` | Cycler entre toutes les boîtes visibles |
| `Enter` | Recentrer la vue sur la boîte sélectionnée (navigue dans le graphe) |
| `q` / `Esc` | Retour à la liste des tables |

> La vue ERD réutilise le schéma déjà chargé — aucune requête supplémentaire n'est effectuée.

---

## MongoDB

> **Feature optionnelle.** MongoDB n'est pas inclus dans le binaire par défaut pour ne pas alourdir les autres utilisateurs. Il faut compiler ou installer avec :
> ```bash
> cargo build --release --features mongodb
> cargo install rowdy-db --features mongodb
> ```

### Connexion

L'URL doit inclure le **nom de la base de données** dans le chemin — c'est obligatoire :

```
mongodb://user:password@host:27017/dbname
mongodb+srv://user:password@cluster0.xxxxx.mongodb.net/dbname
```

Rowdy vérifie la connectivité avec un ping au moment de la connexion. Les collections de la base sont ensuite chargées et affichées dans la vue liste.

### Liste des collections

La vue liste fonctionne exactement comme pour les connecteurs SQL : navigation `j/k`, filtre `/`, `Enter` pour ouvrir une collection dans le Data Grid. Il n'y a pas de badges `[T]`/`[V]` ni de panneau schema (MongoDB n'a pas de schéma fixe).

### Data Grid MongoDB

Chaque document de la collection est affiché sur une ligne. Les colonnes sont déduites de l'**union** de tous les champs des documents de la page courante. Le champ `_id` est toujours affiché en premier.

Les champs imbriqués (sous-documents BSON et tableaux) sont représentés par des **badges verts** :

| Badge | Signification |
|-------|---------------|
| `[obj]` | Sous-document BSON — objet imbriqué |
| `[arr:N]` | Tableau BSON de N éléments |

La **preview bar** affiche le JSON complet du champ sélectionné.

`Enter` sur un badge `[obj]` ou `[arr]` ouvre une sous-grille de navigation (lecture seule) avec le contenu converti en tableau :
- Objet → 1 ligne × N colonnes (une par clé)
- Tableau d'objets → N lignes × union des clés
- Tableau scalaire → colonnes `index` + `value`

La navigation est **récursive** avec breadcrumb : `users › address › city`. `Esc` remonte d'un niveau.

### Éditeur MQL (`e` depuis la liste des collections)

L'éditeur SQL s'adapte au mode MongoDB sous le nom **MQL Editor**. Le titre affiche `MQL Editor │ … │ collection: nom`.

| Syntaxe | Opération |
|---------|-----------|
| `{ "field": "value" }` | `find` avec ce filtre JSON |
| `[{ "$match": … }, …]` | `aggregate` avec ce pipeline JSON |
| _(vide)_ | `find` sans filtre — tous les documents |

`F5` / `Ctrl+Enter` exécute. Le résultat s'affiche dans le panneau bas. `F4` l'ouvre dans un Data Grid complet.

### Édition de documents (`Enter` sur une ligne)

En mode normal (sans `?readonly=true`), `Enter` sur une ligne ouvre l'écran d'édition MongoDB.

```
┌─ Edit: users ──────────────────────────────────────────────────────┐
│   _id        [PK]  string   64abc123def456789012abcd               │
│ > name             string   Alice                                   │
│   age              int      30                                      │
│   address    [obj] object   {"city":"Paris","zip":"75001"}          │
│   tags       [arr] array    ["mongodb","database"]                  │
└────────────────────────────────────────────────────────────────────┘
┌─ Document Preview ─────────────────────────────────────────────────┐
│  {"name":"Alice","age":30,"address":{…},"tags":[…]}                │
└────────────────────────────────────────────────────────────────────┘
  j/k: field   Enter: edit / drill-in   Ctrl+S: save   Esc: back
```

- **`_id`** : badge `[PK]`, toujours non éditable
- **Champs scalaires** (`string`, `int`, `float`, `bool`) : `Enter` ou `i` → édition inline avec curseur
- **`[obj]`** : `Enter` → drill-in dans le sous-document (voir ci-dessous)
- **`[arr]`** : `Enter` → éditeur d'items (voir ci-dessous)
- **Preview** : le panneau bas affiche le JSON du document reconstruit en temps réel
- **`Ctrl+S`** : ouvre un modal de confirmation puis exécute `replace_one` sur la collection

### Navigation imbriquée récursive dans les objets

`Enter` sur un champ `[obj]` ouvre un sous-écran d'édition pour ce sous-document. Le titre indique le **breadcrumb** de navigation :

```
┌─ Edit: users › address ────────────────────────────────────────────┐
│ > city             string   Paris                                   │
│   zip              string   75001                                   │
│   country          string   FR                                      │
└────────────────────────────────────────────────────────────────────┘
```

- Les sous-objets dans l'objet imbriqué sont eux-mêmes drillables (récursif, profondeur illimitée)
- **`Esc`** : valide les modifications du niveau courant, reconstruit le JSON et remonte au niveau parent
- **`Ctrl+S`** depuis un niveau imbriqué affiche `"Press Esc to confirm nested edit first"` — la sauvegarde vers MongoDB n'est possible que depuis le niveau racine

### Éditeur d'arrays item par item

`Enter` sur un champ `[arr]` ouvre un éditeur de liste :

```
┌─ Edit: users › tags ───────────────────────────────────────────────┐
│ > [0]   string   mongodb                                            │
│   [1]   string   database                                           │
│   [2]   string   nosql                                              │
└────────────────────────────────────────────────────────────────────┘
┌─ Array Preview ────────────────────────────────────────────────────┐
│  ["mongodb","database","nosql"]                                     │
└────────────────────────────────────────────────────────────────────┘
  j/k: item   Enter: edit   a: add   D: delete   Esc: confirm & back
```

| Touche | Action |
|--------|--------|
| `j` / `k` | Item suivant / précédent |
| `Enter` | Éditer l'item sélectionné (inline pour scalaire, drill-in pour `[obj]`/`[arr]`) |
| `a` | Ajouter un item vide en fin de liste — entre immédiatement en mode édition |
| `D` | Supprimer l'item sélectionné et renuméroter les suivants |
| `Esc` | Valider les modifications, reconstruire le JSON array et remonter au niveau parent |

Le **preview panel** affiche le JSON array reconstruit en temps réel. Les items de type objet (`[obj]`) ou tableau imbriqué (`[arr]`) sont eux-mêmes drillables récursivement.

> **Note :** les items ajoutés avec `a` sont créés avec le type `string`. Si vous saisissez un entier, il sera sérialisé comme chaîne JSON (`"42"`) — pour forcer un type numérique, éditez directement le JSON du champ parent en inline.

---

## Bases de données supportées

| Moteur | Type | Driver | Statut |
|--------|------|--------|--------|
| PostgreSQL | SQL | `sqlx` | ✅ Supporté |
| SQLite | SQL | `sqlx` | ✅ Supporté |
| libsql / Turso | SQL | `libsql` (HTTP) | ✅ Supporté |
| MySQL / MariaDB | SQL | `sqlx` | ✅ Supporté |
| Redis | Clé-valeur | `redis-rs` | ✅ Supporté — liste des clés + vue détail (string/hash/list/set/zset) |
| MongoDB | Document | `mongodb` 3 | ✅ Supporté (`--features mongodb`) — browse, MQL, édition documents + nested |
