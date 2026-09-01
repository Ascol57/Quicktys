# Captures d'écran attendues

Chaque page du site référence ses captures avec le composant `<Screenshot>`.
Tant qu'un fichier manque, un cadre pointillé légendé s'affiche à la place — le
site reste présentable. Pour ajouter une capture : déposer le PNG **ici**, sous
exactement le nom indiqué, et elle apparaît automatiquement.

## Conseils de prise de vue

- **Fenêtre en 1600×1000** environ (l'export est affiché à 100 % de largeur de
  colonne, ~700 px : au-delà de 1600 px de large, le texte devient illisible).
- **Thème sombre**, celui par défaut de l'app — c'est ce que montre le reste du
  site.
- **Pas de données personnelles** : pas de vrai pseudo Twitch, pas de token, pas
  de chemin `\Users\<prénom nom>`. Un profil de démo suffit.
- **PNG**, recadré sur la fenêtre de l'app (pas tout l'écran), sans ombre du WM.
- Pour les gros plans (un formulaire, un nœud), recadrer serré : une capture
  plein écran d'un détail ne se lit pas.

## Liste

| Fichier | Page | Ce qu'on veut voir |
| --- | --- | --- |
| `hero.png` | landing | La fenêtre principale, onglet **Tableau de bord** rempli (quelques widgets : chat Twitch, now playing, compte à rebours, boutons). C'est l'image d'accroche : la plus soignée. |
| `dashboard.png` | guide/interface, guide/dashboard | Le tableau de bord en mode normal. |
| `dashboard-edit.png` | guide/dashboard | Le tableau de bord en **mode Édition** : poignées de redimensionnement + palette « Ajouter un widget » ouverte. |
| `widget-config.png` | guide/dashboard | Le panneau ⚙ de configuration d'un widget (celui du chat Twitch est le plus riche). |
| `sidebar.png` | guide/interface | La barre latérale gauche, sections dépliées (Profils ouvert avec 2–3 profils). |
| `profile.png` | guide/profiles | Un profil ouvert : condition d'activation + une liaison trigger → séquence. |
| `trigger-picker.png` | guide/profiles | Le sélecteur de trigger ouvert (recherche + groupes par plugin). |
| `automation-editor.png` | guide/automations | L'éditeur d'automatisation avec une séquence de 4–5 nœuds, dont un `if` et un `parallel` imbriqués. |
| `node-palette.png` | guide/automations | La palette de nœuds ouverte (catégories Core / Structure / Flow / Vars). |
| `template-input.png` | guide/variables | Un champ template avec l'autocomplétion `@` ouverte, montrant `pear.title`, `twitch.chat`… |
| `variables.png` | guide/variables | La vue **Variables** listant l'état réactif des plugins chargés. |
| `overlay-editor.png` | guide/overlays | L'éditeur d'overlays : canvas 16:9, 2–3 widgets placés, guides d'alignement visibles pendant un drag. |
| `overlay-widget-config.png` | guide/overlays | Le panneau ⚙ d'un widget dans l'éditeur d'overlay. |
| `obs-browser-source.png` | guide/overlays | La fenêtre **Propriétés de la source navigateur** d'OBS remplie avec `http://localhost:8787/overlay/<id>`. |
| `marketplace.png` | guide/marketplace | La vue Marketplace : liste de plugins/widgets installables. |
| `marketplace-consent.png` | guide/marketplace | L'écran de consentement des capacités lors de l'installation d'un plugin qui tire un module certifié. |
| `settings-accounts.png` | guide/accounts | Réglages → la carte Twitch avec le bouton **Connecter**. |
| `twitch-device-code.png` | guide/accounts | La boîte de dialogue du flux device code (code utilisateur + URL). |
| `twitch-app-registration.png` | guide/accounts | Le formulaire dev.twitch.tv/console/apps rempli (Client ID **masqué**). |
| `settings.png` | guide/interface | La vue Réglages, section langue/apparence. |
| `getting-started-final.png` | guide/getting-started | Le résultat du tutoriel : profil + toast déclenché. |
