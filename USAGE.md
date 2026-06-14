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
  j/k: move   Enter: open   /: filter   q: disconnect
```

### Navigation

| Touche | Action |
|--------|--------|
| `j` / `↓` | Table suivante |
| `k` / `↑` | Table précédente |
| `Enter` | Ouvrir la table dans le Data Grid _(à venir)_ |
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

## Data Grid _(à venir)_

Affichage paginé des données d'une table avec défilement.

---

## Éditeur SQL _(à venir)_

Éditeur multi-lignes avec coloration syntaxique pour exécuter des requêtes libres.

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
