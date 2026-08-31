# DRM & commerce intégré

> Doc de conception. Prose en français, identifiants/formats en anglais. Étend
> **[marketplace.md §8](marketplace.md) (Licences & entitlements)** et
> **ARCHITECTURE.md §11.1 (Économie & licence)** / **§15 (Cloud & comptes,
> local-first)**. Cible **ROADMAP M6**, étape « Entitlements & payant ».
>
> Le marketplace décide *quoi* vendre et *comment le distribuer* ; ce doc décide
> *comment protéger* ce qui est vendu et *comment encaisser* — l'activation, le
> chiffrement du payload, le binding machine, le traçage des fuites, la révocation.

## 0. Honnêteté d'ingénierie (à lire d'abord)

**Un DRM client parfait n'existe pas**, et c'est encore plus vrai ici :

1. **Le cœur est AGPL** → son code, y compris tout contrôle de licence, est
   **public et recompilable**. Un fork peut retirer n'importe quel `if entitled`.
2. **L'utilisateur contrôle sa machine** → toute clé/plaintext présent en mémoire
   est, en principe, extractible par un attaquant déterminé.

Conséquence : **un simple booléen `if paid { … }` dans le host open-source ne
protège rien.** La seule protection qui a des dents côté client repose sur la
**cryptographie** (l'attaquant sans clé n'a que du chiffré), pas sur le secret du
code (principe de Kerckhoffs — le décrypteur peut être open source). Le reste
(binding, watermark, révocation) **augmente le coût** de la piraterie et **protège
la majorité honnête**, sans jamais prétendre à l'inviolabilité.

**Objectifs réalistes, par ordre de force :**

| Objectif | Force | Moyen |
| --- | --- | --- |
| Empêcher la copie triviale (copier le dossier `installed/`) | forte | **payload chiffré**, clé non présente sur disque |
| Lier un achat à un compte/appareil | forte | activation en ligne + binding machine |
| Tracer une fuite jusqu'à l'acheteur | forte (dissuasive) | **watermark forensique** par acheteur |
| Couper l'accès après remboursement/abus | forte (en ligne) | révocation + CRL revérifiée au load |
| Résister à un attaquant expert avec host modifié | **faible** | (impossible côté client) → **server-gated** §4 tier 2 |

**Non-objectif assumé** : rendre impossible qu'un acheteur légitime extraie les
octets déchiffrés et les republie. On rend ça **traçable et révocable**, pas
impossible. La vraie inviolabilité n'existe que pour ce qui **s'exécute côté
serveur** (§4, tier 2).

## 1. Contrainte légale AGPL (décision structurante)

L'AGPLv3 (§3) **prive le cœur de protection anti-contournement légale** : on ne
peut pas transformer le **cœur** lui-même en « mesure technique de protection »
opposable. **Donc le DRM ne protège JAMAIS le cœur** — il protège les **objets
vendus** (extensions/contenus/tiers), qui sont **séparément licenciés** sous
l'exception d'extension (§0bis) et peuvent être fermés/payants.

- Le **décrypteur** et la logique d'activation vivent dans le cœur AGPL → **open,
  c'est assumé** (Kerckhoffs).
- La **clé** et le **payload chiffré** appartiennent à l'objet payant (séparément
  licencié) et au service de licence → **c'est là qu'est le secret**.
- Un fork qui retire l'activation obtient un host qui… ne possède toujours pas les
  clés de déchiffrement des payloads payants. Il peut lire du contenu **gratuit**,
  pas déchiffrer du **payant**.

## 2. Ancrages de confiance & clés

Trois autorités **distinctes** (clés séparées, compromissions cloisonnées) :

| Autorité | Clé (hybride, cf. §2.1) | Épinglée où | Rôle |
| --- | --- | --- | --- |
| **CA de certification** | **Ed25519 + SLH-DSA** racine | dans le host (keyring, cf. marketplace §2) | signer les modules (provenance) |
| **Service de licence (store)** | **Ed25519 + ML-DSA** | clé publique dans le host | signer entitlements & clés de licence |
| **Service d'activation** | **KEM hybride X25519+ML-KEM-1024** + **Ed25519 + ML-DSA-87** | clé publique dans le host | livrer les **content keys** (déchiffrement) liées à l'appareil |

Séparer *certification* (qui a écrit le code) de *licence* (qui a le droit de
l'utiliser) de *activation* (livraison de clé de déchiffrement) est essentiel :
un module peut être **certifié** (sûr) **et gratuit**, ou **certifié et payant**.

### 2.1 Post-quantique (décision structurante)

Toute la crypto asymétrique est **hybride classique + post-quantique dès le
départ** — pas une migration ultérieure, pour deux raisons :

1. **« Harvest now, decrypt later ».** La menace la plus urgente est la
   **confidentialité** : un adversaire enregistre aujourd'hui un `CK` scellé et le
   déchiffre le jour où un ordinateur quantique casse X25519 (Shor). Un contenu
   payant doit rester protégé **des années** → on ne peut pas se permettre du
   classique seul sur le canal d'activation.
2. **Signatures moins urgentes mais ancrages longs.** Une signature ne se forge
   pas *rétroactivement* ; le risque n'existe qu'une fois un ordinateur quantique
   disponible **au moment** de vérifier. Mais les **racines** (CA, store) vivent
   des décennies → on les rend PQ pour éviter une migration douloureuse plus tard.

**Hybride, pas PQ pur (décision).** On combine toujours un algo **classique** et un
algo **PQ** ; la sécurité tient si **l'un des deux** résiste. Motif : les schémas
PQ (ML-KEM/ML-DSA, standardisés FIPS 203/204/205 en 2024) sont récents et moins
éprouvés que X25519/Ed25519 ; l'hybride couvre le cas d'une faille dans l'un.

On vise **NIST niveau 5** (marge maximale, contenu protégé des décennies) —
cohérent sur tous les rôles :

| Fonction | Choix (suite `hyb-mlkem1024-mldsa87-v1`) | Notes |
| --- | --- | --- |
| **KEM** (confidentialité, activation) | **hybride X25519 + ML-KEM-1024** (FIPS 203, niv. 5) | combineur HKDF (voir plus bas) ; **pas** X-Wing (figé à 768) |
| **Signatures** (releases, entitlements, records) | **Ed25519 + ML-DSA-87** (FIPS 204, niv. 5) | concaténées, **les deux** doivent vérifier |
| **Racine CA** (rare, longévité max) | **Ed25519 + SLH-DSA-256s** (FIPS 205, hash-based) | hypothèse minimale (hash) ; sigs volumineuses tolérées (signe peu) |
| **AEAD** (payload + HPKE) | AES-256-GCM / XChaCha20-Poly1305 | **déjà PQ** (Grover ⇒ 256→128 bits effectifs) |
| **KDF / hash** | HKDF-SHA-384 (ancrages) / SHA-256 (content-hash) | niveau 5 côté KDF |

> **Implémenté** (M6.4) dans la crate **`quicktys-crypto`** : signatures, KEM
> hybride (combineur HKDF lié au transcript), seal/open type HPKE, AEAD, avec
> sérialisation octet+`SUITE_ID`. Crates pures Rust `fips203`/`fips204` (FIPS
> finaux) + `x25519-dalek`/`ed25519-dalek` + `chacha20poly1305` + `hkdf`.

**Pourquoi ML-KEM-1024 et pas X-Wing.** X-Wing est un excellent combineur hybride
**mais figé à ML-KEM-768** (niveau 3). Pour rester **niveau 5** partout, on utilise
un **hybride générique** X25519 + ML-KEM-1024 avec un **combineur explicite** :

```
ss = HKDF-SHA-384( ss_X25519 ‖ ss_MLKEM ‖ ct_MLKEM ‖ pk_X25519 , info = suite_id ‖ contexte )
```

(concaténation des deux secrets partagés + liaison au ciphertext/clé publique, façon
NIST SP 800-56C / draft IETF hybrid-KEM). Sécurité **IND-CCA2 si l'un des deux
KEM** tient. On garde **X-Wing (768)** disponible comme **profil allégé**
sélectionnable par `suite_id` (`hyb-xwing-mldsa65-v1`) si un objet préfère des
sceaux plus petits.

**Crypto-agilité.** Chaque signature/sceau porte un **`suite_id` versionné** ; le
keyring du host accepte plusieurs suites en parallèle → on peut faire tourner un
algorithme sans invalider les paquets existants. Crates Rust pressenties :
`libcrux` / `ml-kem` + `ml-dsa` (RustCrypto) ou `fips203`/`fips204`/`fips205`
(**formellement vérifiés / pure-Rust**) ; côté classique `x25519-dalek` /
`ed25519-dalek` ; `hkdf`, `sha2`.

> **Coût de taille (à budgéter).** ML-KEM-1024 : pk ~1568 o, ciphertext ~1568 o.
> ML-DSA-87 : sig ~4.6 Ko. SLH-DSA-256s : sig ~29 Ko (racine, signe rarement).
> Impact réel : l'*activation record* et `signature.json` grossissent de quelques
> Ko — négligeable pour des blobs installés, à noter pour le format.

## 3. Cycle de vie d'un droit (entitlement lifecycle)

```
Achat ──► Entitlement émis ──► Activation ──► Usage ──► Re-validation ──► (Révocation)
 (store)   (jeton signé)       (binding +      (payload    (périodique,     (remboursement,
                                content key)    déchiffré)   grâce offline)   abus, fuite)
```

1. **Achat** : le store crée un **entitlement** — jeton signé
   `{ account_id, sku, kind, id, version_range, model, not_after?, seats }`
   (`model ∈ {perpetual, subscription}`, `seats` = nb d'appareils). Vérifiable
   **hors-ligne** contre la clé publique du store (déjà esquissé marketplace §8).
2. **Activation** (par appareil) : le client envoie `{ entitlement, device_fp }` ;
   le service d'activation vérifie le droit + un **seat** libre, puis renvoie la
   **content key** du payload **chiffrée pour cet appareil** (ECDH vers une clé
   d'appareil dérivée du fingerprint) + un **activation record** signé
   `{ device_fp, sku, not_after, activation_id }`. La clé n'est jamais stockée en
   clair sur disque : elle est reconstituée à la volée à partir du record + secret
   d'appareil (voir §6 binding).
3. **Usage** : au chargement, le host déchiffre le payload **juste-à-temps** en
   mémoire, avant de le passer à wasmtime (module) ou au renderer (widget/contenu).
4. **Re-validation** : à intervalle (ex. abonnement : tous les N jours), reconfirmer
   en ligne. **Fenêtre de grâce offline** obligatoire (ex. 14 j) → on ne *bricke*
   jamais un utilisateur honnête hors-ligne.
5. **Révocation** : remboursement/chargeback/fuite → l'`activation_id` et/ou
   l'`account_id` entrent dans la **CRL** signée (partagée avec celle du
   marketplace §2), revérifiée **au chargement**.

## 4. Tiers de protection du payload (décision)

Chaque objet vendable déclare un `protection` dans son entrée de registre. Le
vendeur choisit le compromis force/friction.

| Tier | Payload | Force | Friction | Pour |
| --- | --- | --- | --- | --- |
| **T0 — token** | en clair, gardé par entitlement signé | faible (open host peut sauter le check) | nulle, marche hors-ligne | gratuit-mais-tracé, dons, « pro cosmétique » |
| **T1 — chiffré** | **AES-256-GCM**, `content_key` livrée par activation | forte contre la copie triviale | activation en ligne 1×/appareil + grâce offline | **le défaut du payant** (modules/widgets/contenus) |
| **T2 — server-gated** | la partie critique **s'exécute/réside côté serveur** | maximale (le client n'a jamais les octets) | requiert connexion pour la feature | très haute valeur (ex. modèle propriétaire, service cloud) |

**T1 en détail** (le cœur du système) :

- Le `.qtpkg` payant contient un **payload chiffré** (`obs.wasm.enc`) + un
  `content_key_id`. La `content_key` **n'est pas** dans le paquet.
- `manifest.json` gagne `"protection": { "tier": "encrypted", "cipher":
  "xchacha20-poly1305", "content_key_id": "…", "enc": ["obs.wasm"] }`
  (défaut XChaCha20-Poly1305, AES-256-GCM possible). **Chiffrement partiel** :
  seuls les fichiers de `enc` sont chiffrés — format octet précis en **Annexe B**.
- À l'install : signature/provenance vérifiées comme d'habitude (la **couche de
  provenance est indépendante** du chiffrement — on sait *qui* a publié même sans
  pouvoir déchiffrer).
- Au **load** : activation obtenue → `content_key` reconstituée en mémoire →
  déchiffrement → hash du plaintext vérifié (`hashes` du manifest, marketplace §3)
  → passage à wasmtime. Le plaintext n'est **jamais** réécrit sur disque.

> Décision : **provenance et confidentialité sont deux couches distinctes.** Un
> module T1 est *signé en clair sur son enveloppe* (donc `certified` calculable
> sans clé) **et** son `.wasm` est *chiffré*. On peut ainsi cataloguer, vérifier
> la provenance et faire le consentement des caps **avant** de posséder la clé.

## 5. Watermarking forensique (décision)

Puisqu'un acheteur T1 peut techniquement extraire le plaintext, on **trace** :

- À l'activation, le service **personnalise** le payload livré (ou une couche de
  métadonnées associée) avec un **identifiant d'acheteur** dérivé de
  `activation_id` — idéalement de façon **robuste** (répartie, pas un simple champ
  effaçable) : pour un widget/contenu, watermark dans les assets ; pour un module
  WASM, table de constantes/ordre de fonctions spécifiques au build de l'acheteur.
- Une fuite publique → on extrait le watermark → on identifie l'`account_id` →
  **révocation + action commerciale**. C'est **dissuasif** : republier, c'est se
  dénoncer.
- **Vie privée** : le watermark encode un identifiant **opaque** (pas de PII) ;
  la correspondance identifiant→compte reste côté serveur. À documenter dans la
  politique de confidentialité (§15 local-first : l'utilisateur doit savoir qu'un
  achat T1 est individualisé).

## 6. Binding machine & activation (décision)

- **Device fingerprint** : dérivé de signaux stables **mais non-PII** (ex. hash de
  quelques identifiants matériels + un secret d'appareil généré au premier lancement
  et stocké via la cap `storage`/secrets OS). Tolérant aux petits changements
  (seuil), sinon un upgrade matériel bricke l'utilisateur.
- **Seats** : un entitlement porte `seats` (ex. 3 appareils). Activation refusée si
  tous pris → l'utilisateur **désactive** un ancien appareil (`deactivate` libère un
  seat côté serveur). UX explicite « Gérer mes appareils ».
- **Content key jamais en clair au repos** : sur disque on ne garde que
  l'`activation record` signé, qui embarque la `content_key` **scellée HPKE vers la
  clé publique de l'appareil**. À chaque boot le host la **rouvre localement**
  (ECDH avec la clé privée d'appareil + l'éphémère serveur inclus dans le sceau) —
  **hors-ligne**, tant que la grâce n'a pas expiré. Voler le dossier `installed/`
  d'un autre PC ne donne rien sans la **clé privée d'appareil**, qui est dans le
  **keystore OS**, jamais dans `installed/`. Protocole complet en **Annexe A**.
- **Grâce offline** : l'`activation record` a un `not_after` de grâce ; hors-ligne,
  l'usage continue jusqu'à expiration, puis dégrade proprement (message, pas de
  crash). L'abonnement expiré désactive la feature au boot suivant.

## 7. Révocation & anti-abus

- **CRL unifiée** (marketplace §2) : `revoked_activations`, `revoked_accounts`,
  `revoked_hashes`. Téléchargée avec l'index, **revérifiée au chargement** (un
  objet révoqué déjà installé est refusé au boot suivant).
- **Chargebacks/remboursements** → révocation automatique de l'entitlement.
- **Partage de compte** : détection côté serveur (activations géo-improbables,
  churn de seats) → plafonds + friction, pas de blocage aveugle.
- **Rate limiting** de l'activation ; un pic d'activations sur un même entitlement
  = signal.

## 8. Rester local-first & ne jamais bricker (décision, cf. §15)

Contraintes **non négociables** — le DRM ne doit pas dégrader l'expérience honnête :

1. **Le gratuit ne requiert jamais de compte ni de connexion.** T0 gratuit et tout
   le cœur marchent 100 % hors-ligne, pour toujours.
2. **Clés de licence hors-ligne** (variante sans compte) : pour un client qui ne
   veut pas de compte, une **clé de licence** (statement Ed25519-signé
   `{ sku, id, seat_hint, not_after }`) débloque un T1 **sans service d'activation**
   — au prix d'un binding plus faible (pas de content key par-appareil ; le payload
   est alors chiffré avec une clé dérivée de la licence). Compromis assumé : moins
   fort, mais respecte le local-first.
3. **Grâce offline généreuse** pour les abonnements activés.
4. **Dégradation propre** : jamais de crash ; message clair « abonnement expiré /
   réactiver » et le reste de l'app continue.
5. **Portabilité** : l'utilisateur peut **exporter/désactiver** ses activations
   (droit de déplacer ses achats).

## 9. Gestion des clés (résumé)

- **Content keys** (AES-256) par objet/version, **wrappées** par le service
  d'activation vers chaque appareil (KEM hybride X25519+ML-KEM-1024). Rotation par version.
- **Clés serveur** (store/activation/CA) : idéalement HSM ; rotation via keyrings
  épinglés multi-clés dans le host (comme la CA marketplace §2).
- **Secret d'appareil** : généré localement, dans le **keystore OS** (Keychain /
  Credential Manager / Secret Service), jamais dans `installed/`.
- **Séparation stricte** : compromettre le store (licences) ne donne pas les
  content keys (activation) ni la CA (provenance).

## 10. Intégration dans le code (où ça vit)

Aligné sur l'existant (le host déchiffre **avant** wasmtime ; réutilise le lockfile
et la CRL du marketplace) :

- **Nouveau sous-système `drm`/`licensing`** dans `quicktys-host` : store des
  entitlements + activation records, vérif signatures, dérivation content key,
  décrypteur. Distinct du linker de caps.
- **Point d'insertion au load** : dans `WasmHost::load_module`/`load_plugin`
  (`lib.rs`), **avant** de décoder le composant, un hook `resolve_payload(path,
  manifest) -> Vec<u8>` : passe-plat pour T0 ; déchiffre pour T1 ; refuse si
  entitlement/activation manquant/révoqué. Le reste du chargement est inchangé.
- **Machine à états d'install** (marketplace §6) : l'étape *Entitle* devient
  *Entitle + Activate* pour T1 (obtenir la content key). *Verify* (provenance)
  reste **avant** et indépendant.
- **Lockfile** (`installed.json`) : champ `entitlement` déjà prévu ; ajouter
  `activation` (`activation_id`, `not_after` de grâce, `protection.tier`).
- **Store de secrets** : réutiliser le sous-dir `secrets` de `DataDir` /
  keystore OS pour le secret d'appareil et les records.
- **IPC** (en plus de marketplace §10) :

| Commande / Event | Rôle |
| --- | --- |
| `activate_entitlement { id }` | activer sur cet appareil (obtient la content key) |
| `deactivate_device { id, activation_id? }` | libérer un seat |
| `list_activations` | appareils/seats de l'utilisateur |
| `enter_license_key { key }` | clé hors-ligne (déjà marketplace §10) |
| `licensing_status { id }` | état : actif / grâce / expiré / révoqué |
| Event `entitlement-changed` | l'UI rafraîchit (expiration, révocation, réactivation) |

## 11. Modèle de menace (synthèse DRM)

| Menace | Mitigation | Résiduel |
| --- | --- | --- |
| Copier `installed/` vers un autre PC | payload T1 chiffré ; content key liée au secret d'appareil (keystore OS) | — |
| Fork du host qui saute `if entitled` | inutile pour T1 : pas de clé ⇒ pas de plaintext | T0 non protégé (assumé) |
| Rejouer un vieux entitlement révoqué | CRL revérifiée au load ; `not_after` | fenêtre de grâce (voulue) |
| Extraire le plaintext après déchiffrement mémoire | watermark forensique → traçable + révocable | possible pour un acheteur (assumé) |
| Partage massif de compte | seats + détection serveur + rate limit | petit partage familial (toléré) |
| Chargeback puis usage | révocation auto sur remboursement | délai de propagation offline |
| Attaque sur la vie privée via watermark | identifiant **opaque**, mapping serveur-only, divulgué à l'utilisateur | — |
| Valeur maximale à protéger absolument | **T2 server-gated** (le client n'a jamais les octets) | requiert connexion |

## 12. Mise en œuvre incrémentale (dans M6, après le marketplace)

Chaque étape apporte de la valeur seule ; on ne construit le lourd que si le
payant décolle.

1. **T0 + entitlements signés** (déjà esquissé marketplace §8) : vendre du
   « gratuit-mais-tracé » / features cosmétiques, jetons vérifiés hors-ligne. Zéro
   friction. Valide la boucle commerce sans crypto de payload.
2. **Clés de licence hors-ligne** (§8.2) : premier vrai payant, sans compte, binding
   faible. Débloque la vente sans infra d'activation.
3. **T1 chiffré + service d'activation** : content keys, binding appareil, grâce
   offline, décrypteur au load. Le cœur du DRM.
4. **Seats & gestion d'appareils** : multi-appareils, désactivation, UX.
5. **Révocation & CRL DRM** : remboursements, fuites, revérif au load.
6. **Watermarking forensique** : traçage des fuites (widgets/contenus d'abord, plus
   simples à watermarker que du WASM).
7. **T2 server-gated** : pour la très haute valeur, quand un objet le justifie.

## 13. Décisions à acter / questions ouvertes

- **Modèle sans compte vs avec compte** : jusqu'où pousser le T1 hors-ligne (clé
  de licence) avant d'exiger l'activation en ligne ? (impacte fort le local-first).
- **Fingerprint** : quels signaux, quel seuil de tolérance, quelle vie privée
  (jamais de PII qui sorte de l'appareil).
- **Grâce offline** : durée par défaut (perpetual vs subscription).
- **Watermark WASM** : faisabilité/robustesse d'individualiser un `.wasm` par
  acheteur sans exploser les coûts de build/CDN (peut-être limiter le watermark
  fort aux assets, faible pour le code).
- **Opérations** : HSM, rotation, SLA du service d'activation (s'il tombe, la grâce
  offline protège, mais quel objectif de dispo ?).
- **Éthique/UX** : communiquer clairement (achats individualisés, activations,
  expiration) ; **ne jamais** faire du DRM qui pénalise l'acheteur plus que le
  pirate (leçon classique du secteur).
- **Interaction exception AGPL** : confirmer juridiquement que le décrypteur dans
  le cœur AGPL protégeant des extensions *séparément licenciées* est conforme
  (le cœur n'est pas la « chose » protégée).

---

## Annexe A — Protocole d'activation (ECDH / HPKE)

Objectif : livrer la **content key** `CK` d'un objet payant **uniquement** à un
appareil autorisé, de façon **rejouable hors-ligne** ensuite (jusqu'à expiration
de la grâce), sans jamais écrire `CK` en clair sur le disque.

On n'invente pas d'ECDH maison : on utilise **HPKE (RFC 9180)**, la construction
standard « sceller vers une clé publique ». HPKE encapsule déjà l'ECDH éphémère +
le KDF + l'AEAD, correctement.

### A.1 Suite cryptographique (décision — hybride PQ, cf. §2.1)

| Rôle | Primitive (suite `hyb-mlkem1024-mldsa87-v1`) |
| --- | --- |
| Accord de clés (KEM) | **HPKE avec KEM hybride générique = X25519 + ML-KEM-1024** (combineur HKDF, §2.1) |
| Dérivation (KDF) | **HKDF-SHA-384** |
| AEAD (HPKE + payload) | **XChaCha20-Poly1305** (défaut) / AES-256-GCM (option) — déjà PQ |
| Signatures | **Ed25519 + ML-DSA-87** (entitlement, activation record) ; **+ SLH-DSA-256s** pour la racine CA |
| Hash / intégrité | **SHA-256** (content-hash) / **SHA-384** (ancrages) |

Le KEM hybride est le point **critique** (harvest-now-decrypt-later, §2.1) : la
`content key` est scellée sous **X25519 ET ML-KEM-1024** à la fois — il faut casser
**les deux** pour la récupérer. Crates : `ml-kem`+`ml-dsa` (RustCrypto) ou
`fips203/204/205` + `x25519-dalek`/`ed25519-dalek` ; `hpke-rs` (KEM custom) pour le
schedule ; `chacha20poly1305`/`aes-gcm`, `hkdf`, `sha2`, `rand_core` ; keystore via
`keyring` (Secret Service / Keychain / Credential Manager). Chaque sceau/signature
porte un `suite_id` versionné (crypto-agilité) — le profil allégé
`hyb-xwing-mldsa65-v1` (X-Wing/768) reste sélectionnable.

### A.2 Clés & identités (bien distinguer)

- **Clé d'appareil** `(d_dev, Q_dev)` — paire **KEM hybride** (une composante
  X25519 **et** une composante ML-KEM-1024) générée au **premier lancement**.
  `d_dev` (privée, les deux composantes) vit dans le **keystore OS**, jamais dans
  `installed/`. `Q_dev` (publique, ~1600 o) est envoyée au serveur. **C'est le seul
  secret qui commande le déchiffrement** — le casser exige de casser X25519 **et**
  ML-KEM-1024.
- **Fingerprint appareil** `device_fp` — identifiant matériel **flou et non-PII**
  (hash de quelques signaux stables + un salt local). **Sert au comptage de seats
  et à l'anti-abus, PAS à la crypto.** On sépare volontairement : le *binding
  cryptographique* est la paire `d_dev` (fort, exact) ; le *binding de gestion* est
  `device_fp` (tolérant aux upgrades matériels, pour ne pas bricker).
- **Clé de service d'activation** `(sk_act, pk_act)` **Ed25519 + ML-DSA-87** — signe
  l'*activation record* (métadonnées : `not_after`, seat…) ; **les deux** signatures
  doivent vérifier. `pk_act` **épinglée** dans le host (keyring, cf. marketplace §2).
  Le KEM, lui, utilise une **encapsulation éphémère hybride** générée par le serveur
  à chaque scellage (le `enc` inclus dans le sceau contient l'éphémère X25519 **et**
  le ciphertext ML-KEM-1024).
- **Content key** `CK` — clé AEAD 256 bits **par objet+version** (`content_key_id`).
  Détenue/dérivée par le serveur ; **jamais** transmise en clair.

### A.3 Requête d'activation (device → serveur)

```
POST /v1/activate
Authorization: Bearer <session compte>            # ou l'entitlement lui-même
Content-Type: application/json
{
  "entitlement": { … , "sig": "<Ed25519 store>" },# {account_id, sku, kind, id,
                                                   #  version_range, seats, model,
                                                   #  not_after?}
  "device_pub":  "<base64url(Q_dev)>",            # clé publique hybride (~1600 o)
  "suite_id":    "hyb-mlkem1024-mldsa87-v1",       # crypto-agilité (§2.1)
  "device_fp":   "<opaque>",                       # pour le seat, non-crypto
  "object":      "module/obs", "version": "1.4.0",
  "nonce":       "<16 o aléatoires>",              # anti-rejeu
  "ts":          "2026-08-03T12:00:00Z"
}
```

Le serveur : vérifie la **signature** de l'entitlement (`pk_store`), le compte, la
**disponibilité d'un seat** (indexé par `device_fp`/`device_pub`), la **CRL**, la
fenêtre `ts`/`nonce`. Puis il **scelle** `CK` vers `Q_dev` (A.5) et renvoie
l'*activation record*.

### A.4 Réponse — l'*activation record* (stocké sur disque)

```jsonc
{
  "activation_id": "act_…",
  "object": "module/obs", "version": "1.4.0",
  "content_key_id": "<16 o>",
  "seat": 2,
  "not_after": "2026-09-03T00:00:00Z",   // grâce (perpetual: lointain ; sub: court)
  "suite_id": "hyb-mlkem1024-mldsa87-v1",
  "hpke": "<base64url(enc || ct)>",       // enc = encapsulation hybride (éphémère X25519
                                          //       ‖ ciphertext ML-KEM-1024, ~1600 o)
                                          // ct  = Seal(CK || key_meta)
  "sig": "<Ed25519 ‖ ML-DSA-87 (sk_act) sur le header canonique>"  // couvre TOUT sauf `sig`
}
```

Deux protections **orthogonales** : `sig` (**Ed25519 + ML-DSA**, les deux vérifiées)
**authentifie les métadonnées** (un appareil ne peut pas s'auto-prolonger
`not_after`) ; `hpke` (**KEM hybride X25519+ML-KEM-1024**) **protège la
confidentialité** de `CK` (seul `d_dev` l'ouvre, et il faut casser X25519 **et**
ML-KEM-1024).

Réponses d'erreur : `409 seat_limit` (→ l'UI propose `deactivate_device`),
`403 revoked` / `403 no_entitlement`, `429 rate_limited`.

### A.5 Scellage HPKE (serveur) et ouverture (appareil)

**Contexte commun** (liaison de domaine, empêche tout mélange de contextes) :

```
info = "qtpkg-activation/v1" ‖ object ‖ "@" ‖ version ‖ content_key_id
aad  = SHA-256( header canonique de l'activation record sans `hpke`/`sig` )
```

**Serveur — sceller** (HPKE Base mode, KEM hybride générique) :
```
(enc, ctx) = HPKE.SetupBaseS(pk = Q_dev, info)      # encap hybride : enc = epk_X25519 ‖ ct_ML-KEM-1024
                                                    # ss = HKDF-384(ss_X25519 ‖ ss_MLKEM ‖ … , suite_id ‖ info)
ct         = ctx.Seal(aad, plaintext = CK ‖ key_meta)
hpke       = enc ‖ ct                                # stocké dans l'activation record
```

**Appareil — ouvrir (à chaque boot, hors-ligne) :**
```
1. vérifier sig (Ed25519 ET ML-DSA-87, pk_act épinglée) ; not_after vs horloge + grâce ; CRL
2. ctx = HPKE.SetupBaseR(enc, sk = d_dev, info)      # decap hybride : DH(X25519) + decap ML-KEM-1024
3. CK ‖ key_meta = ctx.Open(aad, ct)                 # échoue si sceau/appareil/aad ne collent pas
4. déchiffrer les fichiers `.enc` du bundle avec CK  (Annexe B)
```

C'est l'« accord de clés(secret d'appareil, éphémère serveur) » de §6 — mais
**hybride** (X25519 + ML-KEM-1024) et rejoué **localement** contre l'encapsulation
stockée (`enc`) → **pas d'appel réseau au boot**. La grâce offline découle de
`not_after` ; le déchiffrement échoue net si le dossier a été copié sur une machine
sans `d_dev`. Le secret capté aujourd'hui reste illisible même pour un futur
ordinateur quantique (il faudrait casser ML-KEM-1024 **et** X25519).

### A.6 Séquence

```
Appareil                      Service d'activation            Store (compte)
   │  achat ──────────────────────────────────────────────────► entitlement signé
   │◄──────────────────────────── entitlement (Ed25519) ───────────────┘
   │  POST /activate {entitlement, Q_dev, device_fp, object} ─►│
   │                                     vérifie sig/compte/seat/CRL
   │                                     CK ⇒ HPKE.Seal(→Q_dev) ; sig(record)
   │◄──── activation_record {hpke, sig, not_after, seat} ──────┤
   │  (stocke le record ; d_dev reste dans le keystore OS)
   │  ── boot N (hors-ligne) ──: verify sig+CRL → HPKE.Open(d_dev) → CK → déchiffre
```

### A.7 Revalidation, seats, révocation

- **Revalidation** (abonnements) : `POST /v1/revalidate {activation_id}` → nouveau
  `not_after`. Hors-ligne : usage maintenu jusqu'à la fin de grâce, puis
  dégradation propre (§8, jamais de crash).
- **Seats** : `POST /v1/deactivate {activation_id}` libère un seat côté serveur ;
  l'appareil supprime son record local. `list_activations` liste les appareils.
- **Révocation** : `activation_id`/`account_id` → **CRL signée** (unifiée avec le
  marketplace) ; **revérifiée au boot**. Un remboursement/chargeback révoque
  automatiquement.

### A.8 Variante hors-ligne **sans compte** (clé de licence, T1 faible)

Pour respecter le local-first (§8) quand l'utilisateur refuse tout compte : pas de
service d'activation, donc pas de sceau HPKE par-appareil. La `CK` est alors
**dérivée d'un secret imprimé dans la clé de licence** :

```
Clé de licence = "QTLIC-" base32(statement) "-" base32(k_lic)
  statement = {sku, id, version_range, not_after} + sig (Ed25519 ‖ ML-DSA store)  # public, vérifiable
  k_lic     = 256 bits secrets (la partie « à ne pas partager »)
Le paquet embarque  wrap = AEAD_Seal(KEK, CK)   avec   KEK = HKDF-SHA256(k_lic, info)
Appareil : vérifie statement (pk_store, hybride) → KEK = HKDF(k_lic) → CK = AEAD_Open(KEK, wrap) → déchiffre
```

Cette variante est **naturellement PQ** côté confidentialité : `k_lic` → `KEK` →
`CK` est **purement symétrique** (HKDF + AEAD), donc pas de clé publique à casser
par Shor. Seule la **signature** du `statement` est hybride (Ed25519 + ML-DSA).
Compromis **assumé** par ailleurs : binding à la **clé** (partageable) et non à
l'appareil, donc plus faible ; révocation seulement quand l'appareil se reconnecte
(CRL). C'est le prix du 100 % hors-ligne sans compte — proposé en option, pas par
défaut.

## Annexe B — Format `.qtpkg` chiffré (chiffrement partiel)

**Décision : on ne chiffre qu'une partie du paquet.** Catalogue, provenance et
consentement des caps doivent fonctionner **avant** de posséder `CK`. Donc :

| Fichier | Chiffré ? | Pourquoi |
| --- | --- | --- |
| `manifest.json` | **non** | id/kind/version/caps/deps/hashes → catalogue + consentement + AAD |
| `signature.json` | **non** | provenance (CA→éditeur→release) vérifiable sans `CK` |
| `lang/…` | **non** | i18n = données publiques |
| `index.html` (widget non-premium) | non | rendu même sans droit |
| **`<id>.wasm`, assets premium** | **oui** | c'est la valeur payante à protéger |

`manifest.protection.enc` liste **exactement** les fichiers chiffrés. Chacun devient
`<nom>.enc` (ex. `obs.wasm` → `obs.wasm.enc`).

### B.1 Enveloppe d'un fichier chiffré (octet par octet)

```
offset  taille  champ
0       4       magic  = "QTEN"
4       1       format_version = 1
5       1       cipher : 1 = AES-256-GCM (nonce 12 o) | 2 = XChaCha20-Poly1305 (nonce 24 o)
6       16      content_key_id           (= manifest.protection.content_key_id)
22      12|24   nonce                     (aléatoire par fichier)
34/46   …       ciphertext ‖ tag Poly1305/GCM (16 o)   (l'AEAD produit ct+tag)
```

### B.2 Données associées (AAD) — liaison anti-substitution

L'AEAD **authentifie** (sans chiffrer) une AAD qui **cloue** le ciphertext à son
contexte, pour qu'on ne puisse ni déplacer un `.enc` vers un autre paquet, ni le
renommer :

```
aad = "qtpkg/v1\0"
    ‖ content_key_id                    (16 o)
    ‖ manifest_hash                     (SHA-256 du manifest.json canonique, 32 o)
    ‖ utf8(relative_path)               (ex. "obs.wasm")
```

`manifest_hash` lie le fichier à **cette** version exacte (le manifest contient
`hashes[path]` = SHA-256 du **plaintext**) ; donc après déchiffrement on revérifie
`sha256(plaintext) == manifest.hashes["obs.wasm"]` → intégrité du clair, en plus du
tag AEAD.

### B.3 Deux couches indépendantes : provenance vs confidentialité

```
provenance (SANS CK)                 confidentialité (AVEC CK)
─────────────────────                ─────────────────────────
content_hash = sha256(.qtpkg)        au load : HPKE.Open → CK
signature.json signe content_hash    → AEAD.Open(.enc, aad) → plaintext
(couvre le .enc CHIFFRÉ + le clair)  → sha256(plaintext) == manifest.hashes[path]
⇒ on sait QUI a publié sans la clé   ⇒ seul un appareil activé obtient le code
```

L'archive `.qtpkg` (tar déterministe + zstd, marketplace §3) contient les `.enc`
**déjà chiffrés**, donc `content_hash` et la signature de release se calculent et se
vérifient **sans** `CK`. On peut ainsi installer, vérifier la provenance et faire le
**consentement des caps** d'un module, puis n'obtenir `CK` (activation) qu'au
**premier chargement**.

### B.4 Nonces & clés — règles

- **Un nonce unique par fichier chiffré** (aléatoire). XChaCha20-Poly1305 (nonce
  192 bits) est le **défaut** — marge anti-collision confortable ; AES-256-GCM
  (nonce 96 bits) reste permis (peu de fichiers par paquet). `CK` **partagée** par
  tous les `.enc` d'un même objet+version, **distincte** par version
  (`content_key_id` change → rotation à chaque release).
- `CK` n'existe **qu'en mémoire**, le temps du déchiffrement au load ; le plaintext
  n'est **jamais** réécrit dans `installed/`.

### B.5 Point d'insertion (rappel §10)

Le hook `resolve_payload(path, manifest) -> Vec<u8>` de `WasmHost::load_module`/
`load_plugin` : **T0** → lit le fichier tel quel ; **T1** → lit `<name>.enc`, obtient
`CK` (Annexe A : ouverture HPKE du record, ou dérivation clé de licence A.8),
déchiffre en mémoire, vérifie `hashes`, renvoie le plaintext à wasmtime. Refus net
(erreur claire, pas de crash) si le record manque, est expiré hors grâce, ou révoqué.
