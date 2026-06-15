# Rowdy — Liste de bugs et améliorations techniques

Format : `[priorité] domaine — description`  
Priorités : 🔴 bloquant · 🟠 important · 🟡 mineur · ⚪ cosmétique

---

## Sécurité

- 🟠 **libsql / Turso — jeton d'authentification stocké en clair dans `config.toml`** — le `?authToken=...` fait actuellement partie de l'URL sauvegardée dans `~/.config/rowdy/config.toml`. Comportement cible :
  - Si l'URL contient déjà `?authToken=` (profil sauvegardé avec token) → connexion directe, pas de saisie supplémentaire
  - Si l'URL ne contient pas de token ET que le type est `libsql` → afficher un second champ de saisie masqué (`*`) pour le jeton avant de lancer la connexion ; le token saisi n'est pas sauvegardé dans le fichier de config
  - Cela laisse le choix à l'utilisateur : stocker le token dans la config (pratique) ou le saisir à chaque connexion (sécurisé)

---

## Fonctionnels

- ✅ **Data Grid — filtre sur colonne BOOLEAN** — corrigé en v0.5.5 : `build_where` utilise le schéma pour générer `= TRUE`/`= FALSE` sur les colonnes booléennes, et `= val` (sans guillemets) sur les colonnes numériques.

---

## Compiler warnings (dead code)

- 🟡 `src/events/app_event.rs` — `enum AppEvent` et `fn handle_event` inutilisés : stub prévu pour un système d'événements applicatifs, à supprimer ou implémenter
- 🟡 `src/app.rs` — `AppState::Quit` jamais construit : le quit passe par `should_quit`, variant à supprimer
- 🟡 `src/app.rs` — `DbEvent::SchemaLoadFailed(String)` : le payload `String` n'est jamais lu (affiché comme `()`)
- 🟡 `src/db/connectors/mod.rs` — `enum ConnectorType` et `fn from_str` inutilisés : reliquat, à supprimer
- 🟡 `src/db/traits/sql_client.rs` — méthode `disconnect` déclarée dans le trait mais jamais appelée
- 🟡 `src/db/traits/kv_client.rs` — méthodes `disconnect`, `get`, `set`, `del` déclarées mais jamais appelées
- 🟡 `src/db/types.rs` — champ `Column::type_name` jamais lu (utilisé en interne mais pas dans le rendu)
- 🟡 `src/db/types.rs` — champ `DbQueryResult::rows_affected` jamais lu
- 🟡 `src/db/types.rs` — champ `ColumnSchema::is_nullable` jamais lu (prévu pour la validation)
- 🟡 `src/ui/components/modal.rs` — `struct Modal` jamais construite : stub roadmap, OK en l'état
- 🟡 `src/ui/components/status_bar.rs` — `struct StatusBar` jamais construite : stub roadmap, OK en l'état

---

## Dépendances

- 🟠 `sqlx-postgres v0.7.4` — code qui sera rejeté par une future version de Rust (`future-incompat` warning). Nécessite soit une mise à jour de `sqlx` vers 0.8, soit attendre que la version stable soit compatible avec l'édition 2024.

---

## Data Grid

- ✅ **Filtre sur colonnes numériques (PostgreSQL)** — corrigé en v0.5.5 : `build_where` génère `= val` (sans guillemets ni LIKE) pour les colonnes INT/FLOAT/NUMERIC/DECIMAL lorsque la valeur saisie est parseable en nombre.
- 🟡 **Filtre sur colonnes DATE / UUID / JSON (PostgreSQL)** — `LIKE '%val%'` échoue toujours sur ces types. Contournement futur : caster en texte (`col::text LIKE ...`) ou générer `= 'val'` pour les types à égalité exacte.
- 🟡 **`col_widths` non réinitialisé sur `reset_data`** — un resize manuel survive à un rechargement par filtre (la méthode `reset_data` ne vide pas `col_widths`, seul `set_result` le fait). Comportement discutable : peut être voulu ou non.
- 🟡 **Scroll horizontal de `col_offset` non réinitialisé sur filtre** — `reset_data` remet `col_offset` à 0, mais pas `selected_col`. Si la colonne sélectionnée est hors de la vue après rechargement, le viewport se recale correctement via `adjust col_offset` dans `draw`, donc impact faible.
- ⚪ **Indicateur de chargement `⏳` reste visible** si une tâche async ne répond jamais (pas de timeout).

---

## EditRecord

- 🟠 **Tables sans clé primaire** — `Ctrl+S` affiche `-- No primary key` et bloque la sauvegarde. Pas de workaround proposé à l'utilisateur (UPDATE avec WHERE sur toutes les colonnes non-NULL pourrait être une alternative future).
- 🟡 **Pas de validation de format** pour les types DATE / TIMESTAMP / UUID / JSON — une valeur invalide est acceptée dans l'UI et provoque une erreur SQL au moment du `Ctrl+S`. L'erreur est bien affichée, mais l'UX pourrait indiquer le format attendu.
- 🟡 **Champ BOOLEAN éditable en texte libre** — `Space` toggle entre `true`/`false`, mais `Enter`/`i` ouvre quand même l'éditeur texte, permettant de saisir une valeur invalide (ex. `maybe`). À envisager : bloquer l'édition texte sur les champs booléens.
- 🟡 **Valeurs NULL** — il n'est pas possible de remettre un champ à NULL depuis l'édition (la valeur `"NULL"` serait insérée comme texte `'NULL'`). Pourrait nécessiter un raccourci dédié (`Ctrl+Del` ou similaire).
- ⚪ **Troncature du nom de table dans le titre** — si `table_name` est très long, le titre déborde sans troncature.

---

## FK / Sous-grille

- 🟡 **Pas d'indication visuelle** quand `Enter` sur une cellule FK avec valeur NULL ne fait rien (le badge magenta est visible mais aucune feedback).
- 🟡 **FK récursive profonde** — la pile `fk_history` est illimitée en mémoire ; pas de garde contre les cycles FK (ex. A→B→A).
- ⚪ **Badge FK tronqué** si le nom de la table liée est très long (> largeur de colonne - longueur de valeur).

---

## SQL Editor

- 🟡 **Largeur de colonne fixe à 30** dans le panneau résultats (`col_display_width` → `.min(30)`) — pas de resize `[/]` contrairement au Data Grid.
- 🟡 **Résultat SQL perdu** si on quitte l'éditeur et qu'on le rouvre (`SqlEditorScreen::new` recrée un écran vide).
- ⚪ **Pas de numérotation de ligne** dans l'éditeur textarea.

---

## Connexion / Config

- 🟡 **Pas de validation du DSN** avant de lancer la connexion — une URL malformée provoque une erreur async dont le message peut être cryptique.
- ⚪ **Ordre des profils dans `config.toml`** non garanti après une mise à jour (TOML reécrit entièrement).

---

## Redis

- 🟠 **Pas de vue clé-détail** — les valeurs Redis (strings, listes, hashes…) ne sont pas affichées dans le Data Grid, seulement les clés dans la table list.
