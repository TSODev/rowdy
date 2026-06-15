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
```

Les profils apparaissent dans le panneau gauche de l'écran de connexion au démarrage.

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
| `n` | Passer en mode saisie manuelle |
| `D` | Supprimer le profil sélectionné (avec confirmation) |
| `q` | Quitter Rowdy |
| `Ctrl-C` | Quitter Rowdy (toujours disponible) |

### Mode Saisie (`n`)

| Touche | Action |
|--------|--------|
| `Tab` | Changer le type de BDD (`postgres` → `sqlite` → `libsql` → `mysql` → `redis`) |
| _(frappe)_ | Saisir l'URL de connexion |
| `Backspace` | Effacer un caractère |
| `Enter` | Se connecter à l'URL saisie |
| `Ctrl+S` | Sauvegarder la connexion (ouvre le champ Nom) |
| `Esc` | Annuler et revenir en mode Normal |

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

---

## Vue liste des tables

Après une connexion réussie, Rowdy charge automatiquement la liste des tables (ou des clés pour Redis).

```
 Connected: [postgres] postgres://user@localhost/my_db
┌─ Tables (12) ───────────────────────────────────────┐
│                                                      │
│ > orders                                             │
│   products                                           │
│   users                                             │
│   sessions                                           │
│   ...                                                │
│                                                      │
└──────────────────────────────────────────────────────┘
  j/k: move   Enter: open   e: SQL editor   /: filter   q: disconnect
```

### Navigation

| Touche | Action |
|--------|--------|
| `j` / `↓` | Table suivante |
| `k` / `↑` | Table précédente |
| `Enter` | Ouvrir la table dans le Data Grid |
| `e` | Ouvrir l'éditeur SQL |
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
| `[` | Réduire la largeur de la colonne sélectionnée (−5, min 4) |
| `]` | Agrandir la largeur de la colonne sélectionnée (+5, max 80) |
| `Enter` | Cellule FK → ouvre la sous-grille liée ; cellule normale → édition de la ligne |
| `E` | Ouvrir le prompt d'export (puis `c`=CSV, `j`=JSON, `Esc`=annuler) |
| `q` / `Esc` | Retour à la liste des tables |

### Colonnes

- La **colonne sélectionnée** est indiquée par un en-tête souligné en jaune (`h/l` pour naviguer).
- Les **colonnes filtrées** sont mises en évidence en cyan dans l'en-tête.
- `Space` **collapse** une colonne à 3 caractères pour gagner de la place, ou la **restaure**.
- `[` / `]` ajuste finement la largeur par pas de 5 caractères (min 4, max 80) — la valeur est mémorisée pour la session.
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
| `[` / `]` | Redimensionner la colonne |
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

### Types supportés

| Type | Comportement |
|------|-------------|
| Numériques (`INT`, `FLOAT`, `NUMERIC`…) | SQL généré sans guillemets (`42`, `3.14`) |
| `BOOLEAN` | `Space` pour toggler ; SQL génère `TRUE`/`FALSE` |
| `DATE`, `TIMESTAMP`, `UUID`, `JSON`… | Édition texte libre ; la base de données caste automatiquement |
| `TEXT`, `VARCHAR`… | Édition texte standard |

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

`F4` transfère le résultat du SELECT dans un Data Grid complet (`SQL Result`) avec toutes les fonctionnalités de navigation : `j/k/h/l`, resize `[/]`, panel preview, collapse. Les filtres et l'édition sont désactivés. `q`/`Esc` retourne à l'éditeur SQL avec la requête et le résultat intacts. L'export (`E`) est disponible depuis ce mode.

### Détection automatique SELECT / DML

- Requêtes commençant par `SELECT`, `WITH`, `EXPLAIN`, `SHOW`, `DESCRIBE`, `PRAGMA` → `fetch_all` (résultats tabulaires)
- Autres requêtes (`INSERT`, `UPDATE`, `DELETE`, `CREATE`, …) → `execute` (affiche le nombre de lignes affectées)

> **Note :** l'éditeur SQL est uniquement disponible avec les connecteurs SQL (PostgreSQL, SQLite, MySQL).  
> Redis n'est pas supporté (pas de modèle relationnel).

---

## Export CSV / JSON

Depuis n'importe quelle grille de données (Data Grid, sous-grille FK, SQL Result), appuyez sur `E` pour exporter les données chargées.

```
 Export:  c = CSV   j = JSON   Esc = cancel
```

| Touche | Action |
|--------|--------|
| `c` | Exporter en CSV (RFC 4180 : guillemets si la valeur contient une virgule, un guillemet ou un saut de ligne) |
| `j` | Exporter en JSON (tableau d'objets, valeurs typées : `null`, nombres, chaînes) |
| `Esc` | Annuler |

Le fichier est écrit dans votre répertoire personnel : `~/rowdy_<table>_<timestamp>.csv` ou `.json`.

La status bar confirme le nom du fichier créé : `Saved: ~/rowdy_books_1718453421.csv`

> **Note :** l'export porte sur les données actuellement chargées en mémoire (jusqu'à la page en cours pour le scroll infini). Pour exporter une table complète, chargez toutes les pages avant d'exporter.

---

## Barre de statut

Une ligne permanente est affichée en bas de l'écran depuis tous les écrans. Elle indique :

```
 DATA GRID   ● [postgres] postgres://user@localhost/my_db  [1 247 rows]
```

| Élément | Description |
|---------|-------------|
| Badge mode | Écran actif (`CONNECTION`, `TABLES`, `DATA GRID`, `FK VIEW`, `EDIT`, `SQL EDITOR`, `QUERY RESULT`) |
| `●` vert / `○` rouge | Connecté / déconnecté |
| Info DB | Type de BDD + URL (mot de passe et tokens masqués, ex. `user:***@host`, `authToken=***`) |
| `[N rows]` | Nombre total de lignes (DataGrid / FK View / SQL Result seulement) |
| Message flash | Confirmation (vert) ou erreur (rouge) pendant ~4 secondes après une action |

---

## Raccourcis globaux

| Touche | Action |
|--------|--------|
| `Ctrl-C` | Quitter Rowdy depuis n'importe quel écran |
| `q` | Quitter / reculer (selon le contexte) |

---

## Bases de données supportées

| Moteur | Type | Driver | Statut |
|--------|------|--------|--------|
| PostgreSQL | SQL | `sqlx` | ✅ Supporté |
| SQLite | SQL | `sqlx` | ✅ Supporté |
| libsql / Turso | SQL | `libsql` (HTTP) | ✅ Supporté |
| MySQL / MariaDB | SQL | `sqlx` | ✅ Supporté |
| Redis | Clé-valeur | `redis-rs` | ✅ Supporté (`KEYS *` pour lister) |
| MongoDB | Document | — | 🔲 Prévu |
