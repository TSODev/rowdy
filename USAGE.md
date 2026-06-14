# Rowdy — Guide d'utilisation

## Installation

### Depuis les sources

```bash
git clone https://github.com/TSODev/rowdy.git
cd rowdy
cargo build --release
./target/release/rowdy-db
```

### Depuis crates.io _(à venir)_

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
| `q` | Quitter Rowdy |
| `Ctrl-C` | Quitter Rowdy (toujours disponible) |

### Mode Saisie (`n`)

| Touche | Action |
|--------|--------|
| `Tab` | Changer le type de BDD (`postgres` → `sqlite` → `mysql` → `redis`) |
| _(frappe)_ | Saisir l'URL de connexion |
| `Backspace` | Effacer un caractère |
| `Enter` | Se connecter à l'URL saisie |
| `Esc` | Annuler et revenir en mode Normal |

**Formats d'URL :**

| Type | Format |
|------|--------|
| PostgreSQL | `postgres://user:password@host:5432/dbname` |
| SQLite | `sqlite:///chemin/vers/fichier.db` ou `sqlite::memory:` |
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
 users │ row 2/1000 │ col 3/8  (LIMIT 1000)
┌─────┬──────────────────┬─────────────────────┬ cr… ┬──────────────────────┐
│ id  │ name             │ email               │  …  │ created_at           │
├─────┼──────────────────┼─────────────────────┼─────┼──────────────────────┤
│   1 │ Alice            │ alice@example.com   │  …  │ 2024-01-15 08:30:00  │
│ > 2 │ Bob              │ bob@example.com     │  …  │ 2024-02-20 14:10:00  │
│   3 │ Charlie          │ charlie@example.com │  …  │ 2024-03-05 09:45:00  │
└─────┴──────────────────┴─────────────────────┴─────┴──────────────────────┘
  j/k: rows   h/l: cols   g/G: first/last   PgUp/Dn: page   Space: collapse   q: back
```

### Navigation

| Touche | Action |
|--------|--------|
| `j` / `↓` | Ligne suivante |
| `k` / `↑` | Ligne précédente |
| `h` / `←` | Colonne précédente |
| `l` / `→` | Colonne suivante |
| `g` | Première ligne |
| `G` | Dernière ligne |
| `PgDown` | +10 lignes |
| `PgUp` | -10 lignes |
| `Space` | Réduire / agrandir la colonne sélectionnée |
| `q` / `Esc` | Retour à la liste des tables |

### Colonnes

- La **colonne sélectionnée** est indiquée par un en-tête souligné en jaune (`h/l` pour naviguer entre colonnes).
- `Space` **réduit** une colonne à 3 caractères pour gagner de la place, ou la **restaure** à sa largeur naturelle.
- Les colonnes défilent automatiquement pour garder la colonne sélectionnée toujours visible.
- La largeur naturelle est calculée automatiquement d'après le contenu (max 25 caractères).
- Les valeurs longues sont tronquées avec `…`.

> **Note :** les données sont limitées à 1000 lignes (`LIMIT 1000`).  
> Redis n'est pas supporté dans le Data Grid (utiliser la vue liste des clés).

### Roadmap — expansion par clé étrangère _(à venir)_

Lorsqu'une colonne contient une clé étrangère (FK), il sera possible d'afficher
sur les lignes du dessous les enregistrements de la table liée, à la manière
d'un expandable row. Cette fonctionnalité nécessite l'introspection du schéma
(`information_schema` / `PRAGMA foreign_key_list`) et est prévue dans une version future.

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
| `Tab` | Basculer vers le panneau Résultats (si résultat disponible) |
| `Ctrl+Q` | Retour à la liste des tables |

Toutes les touches d'édition standard sont supportées : flèches, `Backspace`, `Delete`, `Home`, `End`, `Ctrl+A` (tout sélectionner), `Ctrl+Z` (annuler), copier/coller selon le terminal.

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
| `Tab` / `Esc` | Retour dans l'éditeur |

### Détection automatique SELECT / DML

- Requêtes commençant par `SELECT`, `WITH`, `EXPLAIN`, `SHOW`, `DESCRIBE`, `PRAGMA` → `fetch_all` (résultats tabulaires)
- Autres requêtes (`INSERT`, `UPDATE`, `DELETE`, `CREATE`, …) → `execute` (affiche le nombre de lignes affectées)

> **Note :** l'éditeur SQL est uniquement disponible avec les connecteurs SQL (PostgreSQL, SQLite, MySQL).  
> Redis n'est pas supporté (pas de modèle relationnel).

---

## Raccourcis globaux

| Touche | Action |
|--------|--------|
| `Ctrl-C` | Quitter Rowdy depuis n'importe quel écran |
| `q` | Quitter / reculer (selon le contexte) |

---

## Bases de données supportées

| Moteur | Type | Statut |
|--------|------|--------|
| PostgreSQL | SQL | ✅ Supporté |
| SQLite | SQL | ✅ Supporté |
| MySQL / MariaDB | SQL | ✅ Supporté |
| Redis | Clé-valeur | ✅ Supporté (`KEYS *` pour lister) |
| MongoDB | Document | 🔲 Prévu |
