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

✅ **Tous résolus en v0.5.7** — zéro warning `dead_code` à la compilation.

| Élément | Résolution |
|---------|-----------|
| `AppState::Quit` | Supprimé (quit via `should_quit`) |
| `DbEvent::SchemaLoadFailed(String)` | Payload `String` retiré |
| `ConnectorType` / `from_str` | Supprimés (reliquat) |
| `SqlClient::disconnect` | `#[allow(dead_code)]` (API trait future) |
| `KvClient::{disconnect,get,set,del}` | `#[allow(dead_code)]` (API trait future) |
| `Column::type_name`, `rows_affected`, `is_nullable` | `#[allow(dead_code)]` (champs API future) |
| `Modal`, `StatusBar`, `AppEvent`, `handle_event` | `#![allow(dead_code)]` (stubs roadmap) |

Il reste uniquement le warning externe `sqlx-postgres v0.7.4 future-incompat` (dépendance, hors contrôle).

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
- ✅ **Validation de format** — corrigé : `validate_field()` sur sortie du mode édition, valeur invalide en rouge, hint format en cyan, `Ctrl+S` bloqué si erreurs.
- 🟡 **Champ BOOLEAN éditable en texte libre** — `Space` toggle entre `true`/`false`, mais `Enter`/`i` ouvre quand même l'éditeur texte, permettant de saisir une valeur invalide (ex. `maybe`). À envisager : bloquer l'édition texte sur les champs booléens.
- 🟡 **Valeurs NULL** — il n'est pas possible de remettre un champ à NULL depuis l'édition (la valeur `"NULL"` serait insérée comme texte `'NULL'`). Pourrait nécessiter un raccourci dédié (`Ctrl+Del` ou similaire).
- ⚪ **Troncature du nom de table dans le titre** — si `table_name` est très long, le titre déborde sans troncature.

---

## FK / Sous-grille

- 🟡 **Pas d'indication visuelle** quand `Enter` sur une cellule FK avec valeur NULL ne fait rien (le badge magenta est visible mais aucune feedback).
- 🟡 **FK récursive profonde (navigation)** — la pile `fk_history` est illimitée en mémoire ; pas de garde contre les cycles FK (ex. A→B→A) dans la navigation interactive. *(Note : l'export JSON FK résout ce problème avec une détection de cycles et une profondeur max de 3.)*
- ⚪ **Badge FK tronqué** si le nom de la table liée est très long (> largeur de colonne - longueur de valeur).

---

## SQL Editor

- 🟡 **Largeur de colonne fixe à 30** dans le panneau résultats (`col_display_width` → `.min(30)`) — pas de resize `[/]` contrairement au Data Grid.
- 🟡 **Résultat SQL perdu** si on quitte l'éditeur et qu'on le rouvre (`SqlEditorScreen::new` recrée un écran vide).
- 🟡 **`Alt+↑/↓` (historique)** peut être intercepté par certains émulateurs de terminal avant d'atteindre l'app — en cas de problème, vérifier les raccourcis du terminal.
- ⚪ **Pas de numérotation de ligne** dans l'éditeur textarea.

---

## Connexion / Config

- 🟡 **Pas de validation du DSN** avant de lancer la connexion — une URL malformée provoque une erreur async dont le message peut être cryptique.
- ⚪ **Ordre des profils dans `config.toml`** non garanti après une mise à jour (TOML reécrit entièrement).
- 🟡 **Script `pre_connect` sans timeout** — si le script ne se termine pas (ex. SSH bloqué), l'UI reste sur "Running pre-connect script…" indéfiniment sans possibilité d'annuler. Contournement futur : `tokio::time::timeout` autour de l'exécution du script.
- 🟡 **Script `post_disconnect` bloque la fermeture** — à la sortie de l'app (`Ctrl-C`), le script est attendu (`await`). Si le script est long ou bloquant, rowdy ne quitte pas. Contournement futur : timeout de ~5 s.
- ⚪ **Champ URL tronqué visuellement** dans le panneau d'édition — les URLs longues dépassent la largeur du champ sans scroll horizontal. La valeur complète est utilisée en interne.

---

## Redis

- ✅ **Vue clé-détail Redis** — corrigé : `Enter` sur une clé ouvre son contenu (string/hash/list/set/zset) dans un Data Grid read-only avec TTL affiché.

---

## MongoDB

- 🟡 **Nouveaux items d'array créés avec type `string`** — quand on ajoute un item via `a`, il est créé avec `type_name = "string"`. Si l'utilisateur saisit un entier ou un float, la valeur sera sérialisée en JSON comme une chaîne (`"42"`) au lieu d'un nombre (`42`). Contournement futur : détecter automatiquement le type à la sortie de l'édition (parse int → float → string).
- ✅ **`insert_one` depuis l'UI** — touche `a` depuis le DataGrid MongoDB ouvre un `EditRecordScreen` vide ; `Ctrl+S` confirme l'insertion.
- ✅ **`delete_one` depuis l'UI** — touche `D` depuis le DataGrid MongoDB affiche une confirmation modale avant suppression.
- 🟡 **Tous les champs de l'écran d'insertion sont de type `string`** — lors d'un `insert_one`, le schéma est inféré depuis les colonnes de la collection courante avec `type_name = "string"`. Si la collection est vide, les champs ne sont pas proposés. Contournement : insérer au moins un document via l'éditeur MQL (F5), puis utiliser `a` pour les suivants.
- 🟡 **Validation de type limitée dans EditRecord MongoDB** — les champs `object`/`array` sont validés comme JSON, mais les champs `string`/`int`/`float` ne font pas l'objet d'une validation de format (pas de schéma MongoDB côté serveur). Saisir `"abc"` dans un champ `int` est accepté et sera sérialisé en string dans le document.
- ⚪ **`ObjectId` affiché comme string hex** — l'`_id` ObjectId est converti en string à l'affichage (`bson_to_value`). La reconstruction `id_to_bson()` re-parse correctement les 24-char hex, mais les `_id` personnalisés (entiers, UUID strings) sont traités comme des strings BSON — ce qui est correct dans la majorité des cas.
