---
layout: home

hero:
  name: Quicktys
  text: Le code que n'importe qui peut publier est celui qui peut faire le moins.
  tagline: L'automatisation de stream pour OBS, Twitch et tout ce qui gravite autour — rebâtie en Rust, où chaque intégration est une extension WebAssembly isolée plutôt qu'un script qui a les clés de la machine.
  image:
    src: /logo.svg
    alt: Quicktys
  actions:
    - theme: brand
      text: Démarrer
      link: /fr/guide/what-is-quicktys
    - theme: alt
      text: Écrire une extension
      link: /fr/dev/overview
    - theme: alt
      text: GitHub
      link: https://github.com/Ascol57/Quicktys
---

<div class="qt-status">
  <div class="qt-status-inner">
    <strong>Où en est le projet.</strong>
    Quicktys est une réécriture clean-room de
    <a href="https://github.com/LordTocs/CastMate">CastMate</a>, développée au
    grand jour et encore pré-1.0. Le noyau host, les deux ABI d'extension, le
    runner d'automatisation, le système d'overlays, la marketplace et la chaîne de
    signature sont implémentés et couverts par des tests d'intégration ; le chat
    Twitch en direct, EventSub, le pilotage d'OBS et l'audio fonctionnent de bout
    en bout. Ce qui reste : l'empaquetage de release, le portage de masse des
    intégrations restantes, et le service d'activation en ligne. La
    <a href="/Quicktys/fr/contributing/roadmap">roadmap</a> donne l'état honnête
    de chaque jalon.
  </div>
</div>

<div class="qt-section">
  <span class="qt-path">Le modèle d'extension</span>
  <h2>Trois types d'extension, trois niveaux de confiance</h2>
  <p class="qt-lead">
    La plupart des outils d'automatisation n'ont qu'un type d'extension et un seul
    niveau de privilège : ce que l'app peut faire, un add-on le peut aussi.
    Quicktys découpe ça en trois, pour que le code que n'importe qui peut publier
    soit celui qui peut faire le moins de choses.
  </p>
  <div class="qt-grid">
    <div class="qt-card qt-tier qt-tier-plugin">
      <span class="qt-tier-who">Écrit par n'importe qui</span>
      <h3>Plugin</h3>
      <p>
        La partie avec laquelle vous interagissez : actions, triggers, valeurs en
        direct, réglages. Tourne dans un bac à sable WebAssembly <strong>sans
        réseau, sans disque et sans accès à votre système</strong> — ces
        interfaces ne sont pas dans son world, il ne peut même pas les demander.
      </p>
      <p>
        Pour atteindre l'extérieur, il appelle un module ; le host vérifie que la
        dépendance est déclarée et que les données ont la forme publiée.
      </p>
    </div>
    <div class="qt-card qt-tier qt-tier-module">
      <span class="qt-tier-who">Développeurs certifiés, signés</span>
      <h3>Module</h3>
      <p>
        Le driver : comment parler à OBS, à Twitch, à votre carte son, à votre
        clavier. Le <strong>seul</strong> étage qui touche le monde réel.
      </p>
      <p>
        Il n'y accède que si sa signature est validée contre une autorité
        épinglée, uniquement pour ce qu'il a déclaré, et uniquement pour ce que
        vous avez accordé. Révoquez une permission : elle cesse d'agir aussitôt.
      </p>
    </div>
    <div class="qt-card qt-tier qt-tier-widget">
      <span class="qt-tier-who">Écrit par n'importe qui</span>
      <h3>Widget</h3>
      <p>
        Les pixels : boîtes du tableau de bord et overlays de stream. Aucun
        WebAssembly — du frontend classique dans un cadre isolé, alimenté par un
        canal de messages étroit.
      </p>
      <p>
        Il peut afficher ce qu'on lui donne et demander à l'app de lancer une
        action. Il ne peut pas mettre la machine en danger.
      </p>
    </div>
  </div>

  <p class="qt-wall-note">
    Le cœur ne livre <strong>aucun</strong> module ni widget. Installer un plugin
    détermine les drivers nécessaires, télécharge exactement ceux-là, et vous
    montre ce pour quoi ils demandent la permission. Désinstallez le plugin : ils
    repartent avec lui.
  </p>
</div>

<div class="qt-section">
  <span class="qt-path">Le moteur d'automatisation</span>
  <h2>Des automatisations qui branchent, bouclent et attendent</h2>
  <div class="qt-split">
  <div>

```yaml
# Démarre deux branches, attend LES DEUX, puis continue.
- kind: parallel
  join: all
  branches:
    - kind: block
      body:
        - { kind: action, plugin: obs, action: start-record }
        - { kind: wait-for, wait: delay, ms: "3000" }
    - kind: action
      plugin: sound
      action: play
      config: { file: intro.mp3, volume: 80 }

- kind: if
  cond: 'obs.scene == "Intro"'
  then:
    kind: action
    plugin: twitch
    action: chat
    config: { message: "En live avec @pear.title" }
```

  </div>
  <div>

<p>
  Une automatisation est un arbre de nœuds typés exécuté par un interpréteur
  tree-walking côté host : <code>action</code>, <code>block</code>,
  <code>parallel</code>, <code>timeline</code>, <code>if</code>,
  <code>switch</code>, <code>loop</code>, <code>let</code>/<code>set</code>,
  <code>try</code>, <code>wait-for</code>, <code>return</code>.
</p>
<p>
  La concurrence est <strong>structurée</strong>. La politique de jointure d'un
  nœud parallèle décide s'il faut attendre toutes les branches ou prendre la
  première et annuler les perdantes via un jeton d'annulation propagé. Les
  boucles sont bornées, les attentes ont un timeout, rien ne tourne sans borne.
</p>
<p>
  Conditions et valeurs sont des expressions <strong>Rhai</strong> sandboxées,
  évaluées côté host sur un contexte déjà matérialisé — synchrones,
  déterministes, et sous forme d'arbre syntaxique plutôt que de JavaScript
  opaque. C'est précisément ce qui rend possibles le suivi de dépendances, la
  validation à la saisie et l'autocomplétion.
</p>
<p>
  <a href="/Quicktys/fr/reference/nodes">Référence des nœuds →</a>
</p>

  </div>
  </div>
</div>

<div class="qt-section">
  <span class="qt-path">Décisions de conception</span>
  <h2>Trois autres décisions à connaître</h2>
  <div class="qt-grid">
    <div class="qt-card qt-card-mint">
      <h3>Les traductions sont des données, pas du code</h3>
      <p>
        Le cœur, chaque plugin, chaque module et chaque widget porte son propre
        dossier <code>lang/</code> de JSON par locale. Les manifestes émettent des
        <em>clés</em>, jamais des chaînes traduites, et le host découvre et
        namespace chaque dossier automatiquement.
      </p>
      <p>
        Comme les locales voyagent à côté du <code>.wasm</code> au lieu d'y être
        compilées, ajouter une langue revient à ajouter un fichier — pas de build,
        pas de Rust, pas de PR dans la config d'un bundler.
      </p>
    </div>
    <div class="qt-card qt-card-clay">
      <h3>Les overlays sont vraiment WYSIWYG</h3>
      <p>
        Placez des widgets sur un canvas à ratio fixe, avec guides d'alignement et
        géométrie en pourcentage, puis pointez une source navigateur OBS sur le
        serveur local. Le rendu est identique dans l'app et à l'antenne.
      </p>
      <p>
        Les aperçus de l'éditeur ne sont pas des maquettes : chacun est le vrai
        widget, rendu dans son propre document isolé, alimenté par le même message
        de config que l'overlay en direct — rendu inerte pour ne jamais avaler un
        glisser-déposer.
      </p>
    </div>
    <div class="qt-card qt-card-violet">
      <h3>Le payant a été prévu dès le premier jour</h3>
      <p>
        Un registre statique signé et auto-hébergeable, des licences vérifiables
        hors-ligne, un consentement par capacité qui prend effet immédiatement, et
        des chaînes de signature hybrides post-quantiques.
      </p>
      <p>
        Une extension tierce peut être fermée et commerciale — c'est exactement ce
        qu'achète l'exception de licence. Le côté vérification, lui, reste ouvert :
        vous pouvez toujours auditer ce que votre app décide de croire.
      </p>
    </div>
  </div>
</div>

<div class="qt-section">
  <span class="qt-path">En pratique</span>
  <h2>À quoi ça ressemble</h2>
  <p class="qt-lead">
    Une app de bureau à onglets : un tableau de bord de widgets personnalisable,
    des profils qui lient triggers et séquences, un éditeur de nœuds qui surligne
    l'étape en cours d'exécution, un navigateur de toutes les variables réactives,
    et un éditeur d'overlays dont les aperçus sont les vrais widgets.
  </p>
  <Screenshot src="/screens/hero.png" caption="Le tableau de bord, avec ses widgets en direct" />
</div>

<div class="qt-section">
  <span class="qt-path">Par où commencer</span>
  <h2>Choisissez votre chemin</h2>
  <div class="qt-grid">
    <div class="qt-card">
      <h3><a href="/Quicktys/fr/guide/installation">Je veux l'utiliser</a></h3>
      <p>
        Installation par OS, construction des bundles, lancement de l'app, et
        votre premier trigger lié à une séquence en une dizaine de minutes.
      </p>
    </div>
    <div class="qt-card">
      <h3><a href="/Quicktys/fr/dev/overview">Je veux l'étendre</a></h3>
      <p>
        Écrire un plugin en Rust contre un world WIT, le compiler en composant
        WebAssembly, l'empaqueter avec ses locales, le signer, le publier.
      </p>
    </div>
    <div class="qt-card">
      <h3><a href="/Quicktys/fr/contributing/architecture">Je veux bidouiller le cœur</a></h3>
      <p>
        Seize crates host, un runtime wasmtime, un frontend Vue 3, et un document
        d'architecture qui consigne chaque décision structurante.
      </p>
    </div>
  </div>
</div>
