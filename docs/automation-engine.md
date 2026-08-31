# Moteur d'automatisation & d'expressions (Quicktys)

> Design des deux sous-systèmes refondus par rapport à CastMate :
> 1. le **moteur d'expressions** (remplace l'eval JS),
> 2. le **modèle d'automatisation** (remplace instant/time/stack/flow).
>
> Décision : un **seul** moteur d'expressions (**Rhai**) alimente **à la fois** le
> templating `{{ }}` et le contrôle de flux des automatisations.

---

## Partie 1 — Le moteur d'expressions

### 1.1 Le problème CastMate

CastMate évalue les templates en compilant du **JS arbitraire** :

```ts
// libs/castmate-core/src/templates/template.ts
const func = new AsyncFunction(...Object.keys(ctx), `return (${template})`)
return await func(...Object.values(ctx))
```

Impossible et indésirable en Rust : non sandboxé, non introspectable, opaque
(impossible de savoir de quelles variables un template dépend, ni de valider/
autocompléter à l'édition).

### 1.2 La solution : Rhai, côté host, sur un contexte matérialisé

- **Un seul moteur** : [Rhai](https://rhai.rs) — langage embarqué pour Rust,
  syntaxe proche JS/Rust (familière aux utilisateurs CastMate).
- **Évalué côté host** (`quicktys-expr`), au moment de l'invoke, sur un contexte
  **déjà matérialisé** : pas d'appel réseau ni d'I/O pendant l'évaluation → **on
  reste synchrone, rapide, déterministe**.
- **Sandbox** : pas d'accès OS/réseau (Rhai n'expose rien par défaut), **limites
  d'opérations et de temps** (anti boucle-infinie), pas d'effets de bord.

### 1.3 Le contexte d'expression

Exposé comme un objet à Rhai, fusion de :

```
{
  ...contexte du trigger,     // ex. user, message, combo
  ...state des plugins,        // valeurs réactives (set-state)
  ...state des modules,        // ex. obs.current_scene
  ...variables locales,        // let/set d'une automatisation (cf. Partie 2)
}
```

> **Décision « mieux que JS » : pas de fonctions injectées par les plugins.**
> Les expressions n'appellent qu'une **bibliothèque standard fournie par le
> cœur** (math, chaînes, dates/durées, listes, objets…). Une fonction fournie par
> un plugin vivrait dans son WASM → l'appeler en cours d'évaluation imposerait un
> aller-retour ABI async à chaque fois (lent, casse le sandbox synchrone). À la
> place, les plugins/modules exposent des **valeurs** via le state. La stdlib est
> versionnée et étendue par les releases du cœur.

### 1.4 Deux usages, une seule grammaire

- **Champ typé** (ex. une `Duration` avec `template: true`) contenant **une seule
  expression** : `{{ timer.remaining + 5 }}` → on évalue → valeur typée, **coercée
  par le schéma** (cf. 1.6).
- **Interpolation de chaîne** : `"Salut {{ user.name }}, {{ points }} pts"` → on
  découpe en régions (texte / `{{ expr }}`), on évalue chaque expression, on
  stringifie et on concatène.

Le découpage en régions est porté par `quicktys-schema` (équivalent du
`parseTemplateString` actuel) ; la compilation/évaluation par `quicktys-expr`.

### 1.5 Analyse statique (le vrai gain vs JS)

Comme une expression est un **AST** et non du JS opaque, on en tire :

- **Suivi de dépendances** : on extrait les variables lues → on sait exactement
  de quel state dépend un template → **réévaluation réactive ciblée** (seulement
  quand ces valeurs changent). CastMate ne peut pas (il enveloppe dans
  `ignoreReactivity`).
- **Validation à l'édition** : référence inconnue, mauvais type, fonction
  inexistante → erreur **dans l'éditeur**, pas à l'exécution.
- **Autocomplétion** : variables/fonctions disponibles proposées dans l'UI.
- **Templating distant** sérialisable proprement (remplace le
  `RemoteTemplateString` ad hoc) : un AST + un contexte se sérialisent.

### 1.6 Coercition par le schéma

Le résultat est converti selon le type du champ (reprend le tableau CastMate) :

| Type | Coercition |
| --- | --- |
| String / Color / FilePath / MediaFile / Directory | → chaîne (option `maxLength`) |
| Number / Duration | → nombre (clamp `min`/`max`) |
| Boolean | `"false"` → `false`, sinon vrai |
| Toggle | `"toggle"` ou booléen |
| Range | templating récursif sur `min`/`max` |
| Dynamic | résout le schéma dynamique puis template |

### 1.7 Résolution asynchrone (cas « viewer → id »)

Certains champs CastMate utilisent un templater **async** (ex. résoudre un pseudo
Twitch en id). Comme l'évaluation reste synchrone, on gère ça par une **passe de
pré-résolution** : avant d'évaluer, le host repère les champs qui opt-in à une
résolution externe, appelle le **service du module** concerné (async), injecte le
résultat dans le contexte, puis évalue en synchrone. Aucune I/O pendant l'eval.

### 1.8 Migration depuis CastMate

La syntaxe Rhai couvre l'essentiel des templates JS existants (accès propriété,
indexation, arithmétique, comparaison, ternaire, concaténation, appels de
fonctions stdlib). Un **codemod** convertira les cas courants ; les templates
exotiques (JS avancé) seront signalés pour réécriture manuelle.

---

## Partie 2 — Le modèle d'automatisation

### 2.1 Limites du modèle CastMate

`Sequence = (InstantAction | TimeAction(offsets) | ActionStack | FlowAction)[]`.
Manques : pas de boucles, pas de variables intermédiaires (juste
`resultMapping`), pas de try/catch, pas de `if` inline (il faut une `FlowAction`
= action de plugin renvoyant un id de branche), parallélisme tout-ou-rien,
offsets déconnectés sans retour.

### 2.2 Nouveau modèle : un arbre de nœuds typés

Une automatisation est un **arbre de nœuds** exécuté par un interpréteur
(tree-walking) dans `quicktys-core`, avec **jeton d'annulation** propagé,
**concurrence structurée**, et **garde-fous** (boucles bornées, budget temps).

Toutes les conditions / valeurs / itérables sont des **expressions** (Partie 1) :
le contrôle de flux et le templating partagent **le même moteur**.

```rust
/// Un nœud d'un flow d'automatisation. Sérialisé en YAML, discriminant `kind`.
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Node {
    /// Invoque une action de plugin ; lie éventuellement le résultat.
    Action(ActionNode),
    /// Enfants ordonnés, partageant un scope de variables locales.
    Block { body: Vec<Node> },
    /// Enfants concurrents, avec politique de jointure.
    Parallel { join: JoinPolicy, branches: Vec<Node> },
    /// La TIMELINE visuelle de CastMate : des pistes décalées dans le temps.
    Timeline { tracks: Vec<Track> },
    /// Condition inline (plus besoin de FlowAction).
    If { cond: Expr, then: Box<Node>, otherwise: Option<Box<Node>> },
    /// Branchement multi-cas sur la valeur d'une expression.
    Switch { value: Expr, cases: Vec<SwitchCase>, default: Option<Box<Node>> },
    /// Boucles bornées (repeat / while / for-each).
    Loop(LoopNode),
    /// Déclare / assigne une variable locale (depuis une expression ou un résultat).
    Let { name: String, value: Expr },
    Set { target: String, value: Expr },
    /// Gestion d'erreur : handler + retry optionnels.
    Try { body: Box<Node>, catch: Option<Box<Node>>, retry: Option<Retry> },
    /// Attente d'un événement / d'une condition, mid-séquence (avec timeout).
    WaitFor(WaitNode),
    Return { value: Option<Expr> },
    Break,
    Continue,
}

pub struct ActionNode {
    pub plugin: String,
    pub action: String,
    pub config: serde_json::Value, // peut contenir des {{ }} résolus au run
    pub bind: Option<String>,      // `let x = action(...)` → remplace resultMapping
}

pub enum JoinPolicy { All, Any, Race } // attendre tout / le premier / course

pub struct Track { pub offset_ms: u64, pub body: Box<Node> } // piste décalée

pub enum LoopNode {
    Repeat  { count: Expr,            body: Box<Node>, max: u32 },
    While   { cond:  Expr,            body: Box<Node>, max: u32 },
    ForEach { var: String, list: Expr, body: Box<Node>, max: u32 },
}

pub enum WaitNode {
    /// Attendre qu'un trigger se déclenche (filtre optionnel) avant timeout.
    Event { trigger: String, filter: Option<Expr>, timeout_ms: Option<u64> },
    /// Attendre qu'une condition devienne vraie (sur le state réactif).
    Until { cond: Expr, timeout_ms: Option<u64> },
    /// Délai simple (durée = expression → permet des délais dynamiques).
    Delay { ms: Expr },
}

pub struct Retry { pub max: u32, pub delay_ms: u64 }
pub struct SwitchCase { pub pattern: Pattern, pub body: Node }
```

`Expr` = `{ source: String, ast: CompiledRhai, deps: Vec<String> }` (source +
AST compilé + dépendances extraites pour la réactivité).

### 2.3 Variables & portée

- Un `Block` ouvre un **scope lexical** ; `let` y introduit une locale, `set`
  réassigne.
- Le **contexte d'expression** d'un nœud = `{ trigger + state + locales en
  portée }`. Templating et automatisation partagent donc le même contexte.
- `Action { bind: "x" }` lie le résultat de l'action à `x` (remplace
  `resultMapping`, en plus lisible et composable).

### 2.4 Sémantique d'exécution

- **Annulation** : un `CancellationToken` (Tokio) est propagé partout ; un abort
  coupe actions, attentes, boucles et branches parallèles.
- **Concurrence structurée** : `Parallel{join}` attend selon la politique
  (`All` = tous, `Any`/`Race` = le premier, annule les autres). La `Timeline`
  reste le cas « pistes décalées » avec jointure en fin de flow.
- **Sécurité** : chaque `Loop` a un `max` d'itérations ; budget temps global ;
  `WaitFor` a un `timeout`. Pas de flow non borné.
- **Erreurs** : une action qui échoue remonte ; `Try` l'intercepte (handler +
  `Retry`). Sans `Try`, l'échec termine le flow (status `failed`).

### 2.5 Démarrer N choses en parallèle et attendre qu'elles finissent

C'est un cas de première classe — et un **vrai manque de CastMate** : son
`ActionStack` ne parallélise que des actions plates (pas des sous-séquences), et
les offsets de `TimeAction` tournent **déconnectés** (le flow ne les attend pas).

`Parallel { join: All }` démarre toutes ses branches **en même temps**, **attend
que toutes soient terminées**, puis l'automatisation **continue**. Chaque branche
est un `Node` complet (donc peut contenir plusieurs actions, des `if`, des
boucles…), pas juste une action.

```yaml
- kind: parallel
  join: all          # all = attendre TOUTES les branches avant de continuer
  branches:
    - kind: block    # branche A : une séquence entière
      body:
        - { kind: action, plugin: obs,   action: start_recording }
        - { kind: action, plugin: time,  action: delay, config: { duration: 3 } }
    - kind: block    # branche B : en parallèle de A
      body:
        - { kind: action, plugin: sound, action: play, config: { file: intro.mp3 } }

# ↓ n'est atteint qu'une fois A ET B terminées
- kind: action
  plugin: twitch
  action: send_chat
  config: { message: "C'est parti !" }
```

Autres politiques de jointure : `any` / `race` (continuer dès la **première**
branche finie, et **annuler** les autres via le jeton d'annulation) — utile pour
un « timeout vs résultat », un « premier qui répond gagne », etc.

### 2.6 On garde la timeline (UX streamer)

Le nœud `Timeline` préserve la signature visuelle de CastMate (pistes d'actions
décalées sur un axe temps, drag-and-drop). Les nouveaux nœuds de contrôle
(if/loop/try/wait) sont structurels et s'imbriquent dans/autour des timelines.

### 2.7 Correspondance CastMate → Quicktys

| CastMate | Quicktys |
| --- | --- |
| `InstantAction` | `Action` |
| `ActionStack` (Promise.all) | `Parallel { join: All }` |
| `TimeAction` + `offsets` | `Timeline { tracks }` |
| `FlowAction` + `subFlows` | `If` / `Switch` (ou `Action` + `Switch` sur son résultat) |
| `resultMapping` | `Action { bind }` + `let` |
| (rien) | `Loop`, `Try/Retry`, `WaitFor`, `Set`, scopes |

Un convertisseur mappera l'ancien modèle sur le nouveau → import des
automatisations CastMate existantes.

### 2.8 Persistance

Sérialisation **YAML** (discriminant `kind`), proche de l'esprit CastMate, dans
`automations/*.yaml`. Les expressions sont stockées en **source** ; l'AST est
compilé au chargement (et invalide → erreur visible avant exécution).

---

## Découpage en crates

| Crate | Rôle |
| --- | --- |
| `quicktys-schema` | schémas + découpage des régions de template + **modèle de nœuds** (types `Node`, sérialisés, partagés avec l'UI) |
| `quicktys-expr` | **moteur Rhai** : sandbox, stdlib, compilation, analyse statique (deps/types), coercition par schéma |
| `quicktys-core` | **runner d'automatisation** (interpréteur sur `Node`, utilise `quicktys-expr` pour conditions/valeurs) + résolution des configs templatées |

> Voir [`ARCHITECTURE.md`](../ARCHITECTURE.md) §5–6 pour l'intégration globale.
