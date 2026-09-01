---
description: "Pourquoi un trigger Quicktys ne se déclenche jamais, une action ne fait rien, une source navigateur reste vide ou une modification d'extension n'a aucun effet — et comment corriger."
---

# Dépannage

## Un trigger ne se déclenche jamais

Descendez cette liste dans l'ordre — les deux premiers points couvrent la plupart
des cas.

**1. Vérifiez le filtre et la condition d'activation.** Rhai convertit
silencieusement toute erreur de compilation ou d'évaluation en `false`. Une faute
de frappe — `=` au lieu de `==`, une variable mal orthographiée — désactive la
liaison sans le moindre message.

**2. Vérifiez le nom de la variable.** Ouvrez la vue **Variables** et lisez la clé
exacte. L'état est namespacé à plat : c'est `obs.scene`, pas `scene`. Si la clé
n'est pas listée du tout, le plugin qui la publie n'est pas chargé.

**3. Vérifiez que le profil est actif.** Une condition d'activation actuellement
fausse éteint toutes les liaisons de ce profil.

**4. Vérifiez que le plugin est installé et chargé.** Le sélecteur de trigger ne
liste que les triggers des plugins chargés.

**5. Pour les follows/subs/raids Twitch**, vérifiez qu'un compte est connecté. Ils
passent par EventSub et exigent un jeton utilisateur ; le chat, non.

**6. Pour le trigger *terminé* d'un compte à rebours**, vérifiez qu'il est affiché
par un widget quelque part. Rien ne tourne côté host, donc un compte à rebours non
affiché ne rapporte jamais le passage à zéro.

## Une action ne fait rien

**Regardez les logs de l'app** — une action qui échoue rapporte un message. Sans
nœud `try`, un échec termine tout le run : ce qui suit n'a donc pas été exécuté
non plus.

**Vérifiez que le module est connecté.** Les actions OBS ne font rien si
obs-websocket n'est pas joignable ; les actions Pear ne font rien si le lecteur ne
tourne pas ou si le consentement a été refusé.

**Vérifiez que la capacité est accordée.** Dans les Réglages, une capacité
révoquée fait échouer immédiatement tout appel qui l'utilise. C'est aussi ce qui
arrive à un **module non signé installé en sideload** : il s'installe, mais ses
capacités système sont refusées au chargement.

## Rien n'apparaît dans une source navigateur

- L'app tourne-t-elle ? Le serveur d'overlays vit à l'intérieur.
- Ouvrez `http://localhost:8787/overlay/<id>` dans un navigateur normal. Si c'est
  vide là aussi, le problème vient de l'overlay, pas d'OBS.
- Le plugin **Overlays** est-il installé ? Sans lui, `/overlay/<id>` n'existe pas.
- Réglez largeur et hauteur de la source sur la résolution de votre canvas. La
  géométrie des widgets est en pourcentages, donc une source mal dimensionnée met
  tout à une échelle étrange.

## Chaque message de chat apparaît deux fois

Le même chat est connecté deux fois — typiquement une boîte de chat du tableau de
bord *et* un overlay qui appellent tous deux la connexion. Fermer l'un ferme
désormais vraiment son socket, donc ça ne devrait pas persister ; si c'est le cas,
relancez l'app et connectez-vous une seule fois.

## Un widget affiche les mauvaises polices

Les iframes de widget n'ont **aucun accès réseau** : une police web retombe
silencieusement sur une autre. Les widgets sont écrits avec des piles de polices
système uniquement. Si vous avez écrit le widget, c'est là qu'est le correctif.

## Un réglage existe d'un côté mais pas de l'autre

Les widgets se configurent dans deux UI distinctes — le tableau de bord et
l'éditeur d'overlays — avec du code séparé. Un réglage ajouté à l'une n'est pas
automatiquement dans l'autre. C'est une aspérité connue.

## L'app ne démarre pas

**Sous Linux**, il faut une vraie session graphique. Elle ne démarrera pas en SSH
sans affichage.

**Partout**, vérifiez que le frontend est construit : `tauri.conf.json` n'a
volontairement pas de `devUrl`, le binaire charge `frontend/dist` embarqué. Après
avoir modifié le frontend, reconstruisez-le.

## Profils et réglages disparaissent au redémarrage (Windows)

Le dossier de données est actuellement construit depuis `$HOME`, que Windows ne
définit pas. Il retombe sur `%TEMP%`, qui est nettoyé. `run-windows.ps1` fixe
`HOME=%USERPROFILE%` pour contourner ça — si vous lancez le binaire autrement,
définissez-le vous-même.

## Une extension ne se construit pas

- Cible `wasm32-wasip2` manquante : `rustup target add wasm32-wasip2`. Rust 1.82+
  est requis.
- Les extensions ne sont **pas** dans le workspace host. Construisez depuis le
  dossier du crate, pas avec `-p` depuis la racine.
- `wit-bindgen` est épinglé à `=0.36.0` volontairement. Les 0.5x plus récentes ont
  besoin d'une fonctionnalité std instable.
- Le nom de l'artefact remplace `-` par `_` : le paquet `counter-client` produit
  `counter_client.wasm`.

## Une modification d'extension n'a aucun effet

Lancez `scripts/build-bundles.sh` et relancez l'app. Le host charge des
**bundles**, pas des `.wasm` bruts depuis `target/`.

Vérifiez aussi que l'extension est dans la liste `EXTENSIONS` ou `WIDGETS` du
script — cette liste est ce qui est livré.

## Toujours bloqué

Ouvrez un ticket sur
[github.com/Ascol57/Quicktys](https://github.com/Ascol57/Quicktys/issues) avec ce
que vous avez fait, ce qui s'est passé, et les lignes de log pertinentes.
