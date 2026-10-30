# geohash-relays : plan de l'étape 1 (stratégies A0 à A6)

Plan de réalisation des alternatives A0 à A6 de
[docs/geohash-relays-requirements-and-alternatives.md](docs/geohash-relays-requirements-and-alternatives.md) (§5, §6, §7),
à côté de la stratégie `AS` déjà en place (lot 1 du `PLAN_ET1.md` de bitchat-console).
Créé le 29/10/2026, refondu le même jour selon les consignes de l'utilisateur.

## Principe

### 8 modules

| Module | Contenu |
|---|---|
| `as_` | La stratégie actuelle, **deux implémentations** : iOS (liste revue `online_relays_gps.csv`, règle iOS) et Android (liste georelays `nostr_relays.csv`, règle Android), chacune alimentée par sa propre liste. Reprend `directory`, `select` et `AppStrategy` d'aujourd'hui. |
| `a0` | Liste et règle uniques, écrire large / lire étroit, remplacement des relais en panne, relais partagés. |
| `a1` | Hachage de rendez-vous (HRW) salé. |
| `a2` | Annuaire des moniteurs NIP-66. |
| `a3` | Relais annoncés dans les présences. |
| `a4` | Republication entre ensembles de relais. |
| `a5` | Fédération synchronisée (NIP-77), détection des omissions. |
| `a6` | Qualification des relais (NIP-11 et sondes). |

`as` est un mot réservé de Rust : le module s'appelle `as_` (G1). Les parties communes
(`geohash`, `Strategy`, `Selected`, `Error`) restent à la racine.

**Toutes les constantes** sont regroupées dans un seul module, `params` (G14) : celles des
stratégies (5 relais, w, r, k, p, époque, seuils, limites, âges) et celles de communication
de chaque implémentation (délais de reconnexion, nombre de tentatives, attente de `OK`, fin
d'historique), chacune avec sa source (fichier et fonction de l'app, ou décision G).
`geohash-relays-net` lit les siennes dans ce module et n'en définit aucune.

### Deux crates (G12, validée)

| Crate | Licence | Contenu |
|---|---|---|
| `geohash-relays` | Unlicense OR 0BSD | Les 8 modules ci-dessus : règles pures, sans réseau, sans async, sans E/S (AGENTS.md inchangé sur ce point). Proposable aux apps. |
| `geohash-relays-net` | **GPL-3.0-or-later** | La communication : les 3 features exclusives, les ports, et le code de communication repris de bitchat-console. Dépend de `geohash-relays`, jamais l'inverse. |

bitchat-console et bitchat-geohash dépendent de `geohash-relays-net` (et, à travers elle, de
`geohash-relays`) : plus de cycle, car le code de communication sort de la console.

Les stratégies qui communiquent (republication A4, sondes A6, moniteurs A2, présences A3,
comparaison A5) sont écrites dans `geohash-relays` en **automates sans E/S** (« sans-IO ») :
elles reçoivent des événements (message reçu, `OK`, échec, heure) et rendent des actions
(publier tel événement sur tel relais, ouvrir tel abonnement, lire tel NIP-11).
`geohash-relays-net` exécute ces actions avec les fonctions de communication de la feature
active (G13). Cette forme sert aussi les portages Swift et Kotlin (lot 11) : seul
l'exécutant change.

### 3 features exclusives (dans `geohash-relays-net`)

| Feature | Implémentation | Fonctions de communication |
|---|---|---|
| `bitchat` | app iOS/macOS (`permissionlesstech/bitchat`) | comportement de l'app : `NostrRelayManager.swift` (10 tentatives puis 10 min), `GeohashSubscriptionManager.swift` ; **plus** attente de `OK`, `AUTH` (NIP-42) et `CLOSED` ; Tor au choix, comme l'app |
| `bitchat-android` | app Android (`permissionlesstech/bitchat-android`) | comportement de l'app : `NostrRelayManager.kt` (reconnexion sans limite, `includeDefaults`) ; **plus** attente de `OK`, `AUTH` (NIP-42) et `CLOSED` ; Tor au choix, comme l'app |
| `console` | nouvelle implémentation, bitchat-console et bitchat-geohash | code de la console déplacé : `Connector` (**Tor obligatoire**, une isolation par cellule), client de relais `relay.rs` (`publish`, `set_filters`, `OK`, `AUTH` NIP-42, `CLOSED`), listes téléchargées (`lists.rs`) |

- Une seule feature à la fois : `compile_error!` si deux sont actives.
- L'attente de `OK`, `AUTH` et `CLOSED` **ne sont pas ignorés** dans les features `bitchat`
  et `bitchat-android`, alors que les apps actuelles les ignorent (§1.5 du document). Ces
  features décrivent donc les apps **telles qu'elles seront** après les PR du lot 12, qui
  apporteront une implémentation compatible en Swift et en Kotlin (G13).
- Tor obligatoire est une contrainte de la console seule : les apps ont Tor actif par
  défaut, mais l'utilisateur peut le couper (G4).
- Les features `bitchat` et `bitchat-android` permettent de faire tourner, en Rust et sur le
  réseau réel, un pair qui se comporte comme chaque app : utile au banc et aux mesures
  (lots 1 et 9).

### Portages Swift et Kotlin, PR vers les apps

Des versions Swift et Kotlin seront peut-être nécessaires pour éviter une FFI dans les apps.
Elles ne seront faites **qu'à la fin**, quand les stratégies auront été évaluées (lot 11).
D'ici là, chaque règle garde des vecteurs de test figés réutilisables par ces portages.
Les modifications de bitchat et bitchat-android nécessaires à l'intégration (stratégies
retenues, attente de `OK`, `AUTH`, `CLOSED`) font l'objet de PR au lot 12.

## Tableau de bord

| Lot | Contenu | État |
|---|---|---|
| 0 | Restructuration : 8 modules, crate `geohash-relays-net` (GPL, 3 features exclusives, ports, code de communication de la console), `Strategy` et `Selected` généralisés | ✅ |
| 1 | Banc de comparaison : rendez-vous iOS/Android, charge, stabilité, coût de capture, pannes | ✅ |
| 2 | `a4` : republication entre les ensembles iOS et Android | ✅ |
| 3 | `a0` : liste et règle uniques, écrire large / lire étroit, remplacement, relais partagés | ✅ |
| 4 | `a6` : qualification NIP-11 et sondes | ✅ |
| 5 | `a1` : HRW salé | ✅ |
| 6 | `a2` : annuaire des moniteurs NIP-66 | ✅ |
| 7 | `a3` : relais annoncés dans les présences | ✅ |
| 8 | `a5` : fédération synchronisée, détection des omissions | ✅ (essai réel au lot 9) |
| 9 | Mesures réelles du §7 (avec l'utilisateur) | ⏳ (lecture seule faite) |
| 10 | Évaluation des stratégies et dossier de proposition aux apps | ✅ (à relire) |
| 11 | Portages Swift et Kotlin des stratégies retenues | ⬜ |
| 12 | PR vers bitchat et bitchat-android : intégration, attente de `OK`, `AUTH`, `CLOSED` | ⬜ |
| 13 | Recensement des relais (NIP-65), base SQLite, liste compatible BitChat : `relay-census` | ✅ |

Ordre : 0 → 1, puis chaque stratégie après 1 (mesurée dès sa livraison). Ordre proposé,
celui du §6 : 2 (sans changer les apps), puis 3 et 4 (proposition aux apps), puis 5, 6, 7
(cible), 8 en dernier (infrastructure). 4 précède 6 (les moniteurs appliquent les mêmes
contrôles) ; 5 précède 6 et 7 (A2 donne les candidats de A1 ; A1 ou `AS` amorcent A3).
9 suit 2, 4 et 6 ; 10 suit 9 ; 11 suit 10 ; 12 clôt.

## Décisions

Toutes validées le 29/10/2026, sauf mention contraire.

| Id | Sujet | Proposition |
|---|---|---|
| G1 ✅ | Noms | Modules `as_`, `a0` … `a6` (`geohash-relays`) ; features `bitchat`, `bitchat-android`, `console` (`geohash-relays-net`). |
| G2 ✅ | Ports | Dans `geohash-relays-net` : un trait par besoin (`RelayLink` : publier, s'abonner, recevoir ; `HttpFetch` : NIP-11 ; `Clock`), avec la sémantique de l'implémentation de chaque feature (fichier et fonction cités). |
| G3 ✅ | Async | Feature `console` : ports async (tokio, comme la console) ; features `bitchat` et `bitchat-android` : ports à rappels synchrones, plus proches de Swift et Kotlin et des portages du lot 11. Validée le 29/10/2026. |
| G4 ✅ | AGENTS.md | `geohash-relays` garde « no network, no async, no I/O » ; on y ajoute les automates sans E/S (G13), le module `params` (G14) et la règle de licence (G5). `geohash-relays-net` a son propre AGENTS.md : GPL ; publication réelle seulement sur action de l'utilisateur ; **Tor obligatoire (hors bouclage) pour la feature `console` seulement**, car c'est une contrainte de la console : les implémentations actuelles de bitchat et bitchat-android ne l'imposent pas (Tor au choix). |
| G5 ✅ | Dépendances | `sha2` (A1, empreinte de liste A0) ; `serde_json` (NIP-11, NIP-66) ; `futures` ou `async-trait` seulement dans `geohash-relays-net` ; pas de crypto de signature : l'hôte vérifie et signe (il le fait déjà). **Toute dépendance, directe ou transitive, doit être compatible avec `Unlicense OR 0BSD`** (licences permissives seulement). Vérifié le 29/10/2026 : `sha2`, `serde_json`, `async-trait`, `futures`, `thiserror` sont `MIT OR Apache-2.0`. Contrôle automatique en CI (`cargo deny check licenses`, liste blanche). **Sinon** : le code concerné va dans une crate de communication à part, **GPL-3.0-or-later** (G12). |
| G12 ✅ | Crate de communication | **Retenue d'emblée** (29/10/2026), pas seulement en repli : crate `geohash-relays-net`, GPL-3.0-or-later (`LICENSES/GPL-3.0-or-later.txt`, en-têtes SPDX `GPL-3.0-or-later`, REUSE), dépôt propre `wvmscs/geohash-relays-net` (`main`), propre `[workspace]`. Elle porte les 3 features et reçoit le code de communication de la console (`Connector`, `relay.rs`, `lists.rs`, Arti). bitchat-console et bitchat-geohash en dépendent ; `geohash-relays` n'en dépend jamais, et reste ainsi proposable aux apps. |
| G13 ✅ | Automates sans E/S | Les stratégies qui communiquent sont des automates purs dans `geohash-relays` (événements en entrée, actions en sortie, heure fournie) ; `geohash-relays-net` les exécute. Les features `bitchat` et `bitchat-android` **n'ignorent pas** l'attente de `OK`, `AUTH` et `CLOSED` : ces fonctions en font partie, et une implémentation compatible, qui modifie les codes Swift et Kotlin, est proposée en PR au lot 12. |
| G14 ✅ | Constantes | Toutes les constantes dans un seul module, `params` de `geohash-relays`, source citée pour chacune ; `GEO_RELAY_COUNT` y passe (réexporté à la racine). Demandé avec la validation de G7 à G11. |
| G6 ✅ | `Selected` | Généralisé : rang dans l'ordre de la stratégie, rôle (lecture-écriture, écriture seule), origine (`Ios`, `Android`, `Shared`, `Announced`, …) ; rangs iOS et Android gardés pour `as_`. Version 0.2.0, console et bitchat-geohash adaptées au même lot. |
| G7 ✅ | A0 | Règle iOS sur la liste iOS ; w = 8, r = 5 ; 0 relais partagé par défaut, 1 ou 2 au choix parmi `damus`, `primal`, `offchain` (communs aux deux apps). |
| G8 ✅ | A1 | `SHA-256(sel ‖ préfixe(cellule, p) ‖ url normalisée)`, k = 5, p = 4 ; époque = jour UTC ; sel fourni par l'appelant ; écriture vers les deux époques pendant 1 h autour du changement. |
| G9 ✅ | A3 | Tag `["relay", url]` (pas `r`, pris par le pont maillé), au plus 5 par présence ; k = 2 clés distinctes, preuve de travail ≥ 8 bits ; amorce A1 ou `as_`. |
| G10 ✅ | A4 | Mode optionnel, désactivé par défaut ; publication réelle seulement sur action de l'utilisateur (invariant de la console). |
| G15 | Recensement (lot 13) | Demande de l'utilisateur (30/10/2026) ; choix de l'utilisateur : binaire de `geohash-relays-net` (feature `census`), Tor par défaut et `--direct` en option, géolocalisation hors ligne DB-IP Lite (`maxminddb`), contrôle passif et test d'écriture (`--probe`, A6). Détails par l'agent, *à valider* : automate `census::Crawler` dans `geohash-relays` (G13), liste la plus récente de chaque auteur, 500 listes par relais, 90 s par relais, 16 à la fois, 8 tours au plus ; adresses publiques seulement ; noms résolus par la sortie Tor ; liste BitChat = `wss://` port 443 sans chemin, a répondu, localisé, aucun refus A6 ; lancement A6 partagé avec la console (`qualify::run`). |
| G11 ✅ | A5 | Filtre de synchronisation documenté (`strfry sync`), détection des omissions ; essai entre le relais de test de l’utilisateur (`BITCHAT_TEST_RELAY`) et un second strfry local. |

## Lots

### Lot 0 : restructuration ✅

- [x] Module `as_` : `directory`, `select`, `AppStrategy` déplacés ; deux implémentations
      `as_::Ios` et `as_::Android`, chacune avec sa liste ; `Both` = union (D4 de la console).
      Tests existants inchangés.
- [x] Modules `a0` … `a6` vides, documentés (renvoi au §5 du document).
- [x] Module `params` (G14) : toutes les constantes, chacune avec sa source ;
      `GEO_RELAY_COUNT` déplacé (réexporté à la racine) ; aucune constante ailleurs, dans
      les deux crates.
- [x] `Selected` et `Strategy` généralisés (G6) ; contrat « instantané immuable » : quand ses
      entrées changent, l'hôte reconstruit la stratégie ; forme commune des automates sans
      E/S (G13 : `Input`, `Action`).
- [x] `deny.toml` (licences permises : Unlicense, 0BSD, MIT, Apache-2.0, BSD, ISC,
      Unicode-3.0, Zlib) et `cargo deny check licenses` dans la GitHub Action (G5).
- [x] AGENTS.md (G4), README, CHANGELOG, version 0.2.0, `reuse lint`.

**Crate `geohash-relays-net` (G12)**

- [x] Squelette : `Cargo.toml` (GPL-3.0-or-later, propre `[workspace]`, dépendance git sur
      `geohash-relays`), `LICENSE`, `LICENSES/GPL-3.0-or-later.txt`, `REUSE.toml`, en-têtes
      SPDX, `AGENTS.md` (G4), README, CHANGELOG, GitHub Action (fmt, clippy, tests et
      `reuse lint` pour chaque feature). Dépôt `wvmscs/geohash-relays-net` créé le 29/10/2026,
      à la demande de l'utilisateur.
- [x] Features `bitchat`, `bitchat-android`, `console`, exclusives (`compile_error!`).
- [x] Ports (G2, G3) et port factice pour les tests.
- [x] Feature `console` : `Connector`, `Socks5`, `Loopback`, Arti, `relay.rs`, `lists.rs`
      déplacés de bitchat-console avec leurs tests (proxy SOCKS5 et relais de test).
- [x] Features `bitchat` et `bitchat-android` : comportement de connexion de chaque app
      reproduit (reconnexion, fin d'historique), source citée, **plus** attente de `OK`,
      `AUTH` NIP-42 et `CLOSED` (G13) ; Tor au choix (G4).

**Clients**

- [x] bitchat-console : dépend de `geohash-relays-net` (feature `console`) ; modules déplacés
      retirés ; stratégie remplaçable à chaud ; option `--strategy AS|A0|…` ; tous ses tests
      verts.
- [x] bitchat-geohash : n'a pas besoin de communication partagée (carte en lecture seule, son
      propre écouteur) : reste sur `geohash-relays` seule, au tag `v0.1.0`.

### Lot 1 : banc de comparaison ✅

- [x] Module `bench` (sans feature) : pour un ensemble de cellules et deux pairs (iOS et
      Android de `as_`, ou deux versions d'une liste) :
      - **rendez-vous** : la lecture de l'un recoupe-t-elle l'écriture de l'autre (H1) ;
      - **charge** : serveurs distincts, cellules par serveur au maximum ;
      - **stabilité** : choix inchangés entre deux versions d'une liste ;
      - **pannes** : rendez-vous quand *d* relais choisis tombent (tirage déterministe) ;
      - **coût de capture** : serveurs à placer au centre d'une ville pour prendre les *k*
        places (question 4 du §7).
- [x] Données figées : deux versions datées de chaque liste, annotées dans `REUSE.toml`.
- [x] Exemple `examples/compare.rs` : tableau par stratégie ; résultats `AS` notés ici.

Résultats `AS` (référence, listes figées : iOS du 26/07, Android du 25/10 ; anciennes : iOS du
21/06, Android du 22/10 ; `cargo run --release --example compare`) :

| Mesure | `AS` |
|---|---|
| Rendez-vous pair iOS / pair Android, précision 2 (1 024 cellules) | 93,3 % |
| Idem, 500 cellules de précision 5 | 91,8 % |
| Serveurs utilisés (Android, précision 2) ; le plus chargé | 119 ; `relay.favillakey.io` (361 cellules) |
| Cellules à moins de 5 serveurs (Android, précision 2) | **322** (le chiffre du §4, retrouvé) |
| Stabilité iOS (21/06 → 26/07), Android (22/10 → 25/10), indice de Jaccard moyen | 0,47 ; 0,43 |
| Rendez-vous avec 1, 2, 3 relais choisis morts | 87 %, 80 %, 75 % |
| Coût de capture (serveurs au point de la ville), iOS | 5 à Paris, New York, San Francisco, Londres |
| Idem, Android | > 60 : des serveurs ajoutés au même point perdent les égalités (ordre de la liste) ; il faut être géolocalisé plus près que les serveurs en place |

### Lot 2 : `a4`, republication ✅

- [x] Règle : relais d'une cellule qui n'ont pas un événement signé vu ailleurs ;
      déduplication par id ; limite par minute (seau à jetons) ; kinds 20000, 20001, 1, 5 ;
      âge maximal (1 h ; 15 min avec le tag `r` du pont).
- [x] Mécanisme par les ports : abonnement à l'union des ensembles iOS et Android, envoi
      vers les seuls manquants, compteurs (republiés, écartés, limite atteinte).
- [x] bitchat-console : mode `--republish` (G10).
- [x] Tests : port factice puis relais de test « iOS » et « Android » séparés ; message vu
      de l'autre côté une seule fois, pas de boucle entre deux republieurs.
- [ ] Débit réel des cellules actives (question 5 du §7) pour régler la limite : **reporté au
      lot 9** (mesure sur le réseau réel). Limite provisoire : 60 publications par minute.

Fait : `a4::Republisher` (délai de 3 s avant republication, pour laisser les autres relais
l'envoyer ; une seule passe par id ; seau à jetons), `a4::Plan::of` (union des ensembles de
stratégies) ; `geohash-relays-net::runner` (signatures vérifiées avant l'automate) ; console
`--republish` qui suit la zone. Essais : deux republieurs sur deux relais, un événement
republié une fois par republieur au plus, jamais renvoyé à sa source.

### Lot 3 : `a0`, correction du modèle actuel ✅

- [x] Règle (G7) : ordre partagé ; `write` = *w* premiers, `read` = *r* premiers joignables ;
      remplacement par le suivant ; relais partagés en option (`includeDefaults` d'Android).
- [x] Liste versionnée : numéro et empreinte SHA-256 ; signature vérifiée par l'hôte.
- [x] État des relais (panne, `AUTH`, `CLOSED`, redirection) lu par les ports quand la
      sous les trois features (G13).
- [x] Propriété testée : deux pairs qui ont perdu chacun moins de *r* relais parmi les *w*
      gardent un relais commun ; contre-exemple sans « écrire large ».

Fait : `a0::SharedStrategy` (règle iOS, w = 8, r = 5, `with_health`, `with_shared`,
`with_widths`), `a0::ListVersion` (SHA-256 des lignes normalisées) ; console `--strategy A0`,
`--shared-relays`, état des relais (mort après 3 échecs ou refus d'authentification, oublié
après 10 min), stratégie reconstruite à chaud. Banc (`compare`) :

| Mesure | `AS` | `A0` (copies de liste à un mois d'écart) |
|---|---|---|
| Rendez-vous, précision 2 ; précision 5 | 93,3 % ; 91,8 % | **100 % ; 100 %** |
| Cellules à moins de 5 serveurs (précision 2) | 322 | 0 |
| Rendez-vous avec 1, 2, 3 relais morts | 87, 80, 75 % | **100, 100, 100 %** |
| Coût de capture (iOS), quatre villes | 5 | 5 (A0 ne résiste pas à la capture, comme prévu) |

### Lot 4 : `a6`, qualification ✅

- [x] Règle : verdict NIP-11 par exigence du §3 (R5, R6, R7, R8, R11, R13), raison de chaque
      refus ; résultat de sonde prioritaire sur la déclaration.
- [x] Mécanisme par les ports : lecture NIP-11 (`Accept: application/nostr+json`), sonde
      (écriture kind 20000 sur une cellule de test, relecture après 5 min, 1 h, 24 h ;
      redirection R2 ; certificat R1) ; cache daté chez l'hôte.
- [x] `Qualified<S>` : filtre avant classement, seulement avec « écrire large » (A0) ou un
      filtre partagé (A2) ; sinon refus documenté (mise en garde du §5).
- [x] bitchat-console : commande `--qualify` (rapport sur les deux listes, questions 1 à 3
      du §7).
- [x] Tests : documents NIP-11 figés, **synthétiques** (sur le modèle de `relay.mmwaves.de` et
      `herbstmeister.com` : AUTH ; un relais payant à preuve de travail 16 ; un relais ouvert),
      pour ne pas figer des documents de tiers sans licence ; redirection testée par un
      serveur local qui répond `302` (comme `treuzkas.branruz.com`).

Fait : `a6::{Nip11, Probe, Verdict, Finding, Qualification, Prober}`. La sonde s'abonne
**avant** de publier (R9 : livraison en direct, obligatoire) puis relit (R11 : conservation,
recommandée) : un relais qui ne garde pas les éphémères reste utilisable, avec un
avertissement R11. Une redirection (R2) ou un certificat refusé (R1) arrêtent la connexion
pour de bon (`-net`). Console : `--qualify` (NIP-11 seulement), `--qualify-probe` (écrit un
événement de test par relais, sur action de l'utilisateur), rapport `qualification.md`.

### Lot 5 : `a1`, HRW salé ✅

- [x] Règle (G8) : score, *k* meilleurs, préfixe, sel, époque et chevauchement.
- [x] Vecteurs de score figés (pour le lot 11).
- [x] Tests : répartition uniforme, stabilité à l'ajout d'une entrée, voisins qui partagent
      leurs relais ; banc : capture sans sel et avec sel.
- [x] bitchat-console : `--hrw-salt <hex>`.

Fait : `a1::{HrwStrategy, score, Salts, epoch_of}` ; score
`SHA-256(sel ‖ 0 ‖ préfixe ‖ 0 ‖ url)`, 8 premiers octets gros-boutiens (les octets nuls
séparent les champs) ; vecteurs vérifiés par une implémentation Python indépendante.
Console : `--strategy A1 --hrw-salt <hex>`. Banc :

| Mesure | `AS` | `A0` | `A1` (liste iOS / liste Android) |
|---|---|---|---|
| Rendez-vous p2 ; p5 | 93,3 ; 91,8 % | 100 ; 100 % | 98,4 ; 97,0 % |
| Serveurs utilisés ; le plus chargé (cellules, p2) | 119 ; 361 | 136 ; 326 | **332 ; 30** |
| Stabilité liste iOS ; Android | 0,47 ; 0,43 | 0,47 ; — | **0,62 ; 0,78** |
| 1, 2, 3 relais morts | 87, 80, 75 % | 100, 100, 100 % | 96, 93, 90 % |
| Capture (iOS / Android) | 5 / > 60 | 5 / — | **> 60 / > 60** ; sans sel, 5 noms calculés à l'avance suffisent |

Remarque : avec A1, le serveur le plus chargé est un tunnel temporaire
(`*.trycloudflare.com`) : la qualification (A6) ou les moniteurs (A2) doivent écarter ce genre
d'entrée.

### Lot 6 : `a2`, annuaire NIP-66 ✅

- [x] Vérifier le format actuel de NIP-66 (kinds 30166 et 10166) : fait le 29/10/2026
      (`nostr-protocol/nips`, `66.md`). Écarts avec le document : les exigences sont des tags
      `R` (`auth`, `writes`, `pow`, `payment`, `!` pour faux), sans niveau de preuve de
      travail ; les kinds acceptés ou refusés sont des tags `k` ; `n` donne le réseau (`tor`),
      `g` la position ; le contenu peut porter le document NIP-11 ; la NIP demande de ne pas
      se fier à un seul moniteur et d'écarter un résultat qui retirerait une part
      déraisonnable des relais. La liste des moniteurs existants passe au lot 9 (réseau réel).
- [x] Règle : candidat si *m* moniteurs sur *n* le signalent et s'il passe `a6` ; annuaire
      donné à `a1` ou `a0`.
- [x] Mécanisme par les ports : abonnement aux moniteurs ; repli sur les listes GitHub.
- [x] Tests : rapports figés, seuil, moniteur hostile seul sans effet.

Fait : `a2::{Report, Announcement, Directory, Candidate, Watcher}` ; constat retenu à la
majorité des moniteurs de confiance qui signalent le relais ; un moniteur hostile seul est
sans effet ; garde-fou à 50 % (`A2_MAX_REMOVED_SHARE`). Console : `--monitors`,
`--monitor-relays`, cache `monitors.csv`.

### Lot 7 : `a3`, relais annoncés ✅

- [x] Compatibilité : iOS ignore un tag de plus dans une présence
      (`NostrInboundPipeline.subscribeNostrEvent` ne lit que `n` et `t`) ; Android aussi :
      `GeohashMessageHandler.kt` (commit `20501fdfe0`) ne lit que `g`, `n` et `t`.
- [x] Règle (G9) : tag, registre des annonces (cellule, clé, relais, preuve de travail,
      date), `Announced<S>` = base de `S` + relais annoncés par *k* clés récentes.
- [x] Mécanisme par les ports : tag ajouté aux présences, lecture des présences reçues.
- [x] Tests : convergence d'un pair iOS et d'un pair Android ; Sybil à faible preuve de
      travail sans effet ; annonces périmées oubliées.

Fait : `a3::{announce_tags, PresenceBook, Announced}` ; convergence vérifiée sur les cellules
de précision 2 où les apps se manquent aujourd'hui (rendez-vous 100 % dès que deux clés de
chaque côté annoncent) ; console `--announce-relays` (présences avec preuve de travail de
8 bits, ensemble annoncé suivi aussitôt). Il faut au départ un relais commun (amorce), qui
porte les annonces : relais partagés A0, A1 ou `AS`.

### Lot 8 : `a5`, fédération ✅

- [x] Documentation (G11) : filtre de synchronisation, configuration `strfry sync` / NIP-77.
- [x] Règle : omissions d'un membre pour une cellule et une fenêtre ; `Federation` : lire sur
      1 ou 2 membres, écrire sur un.
- [x] Mécanisme par les ports : comparaison des ids vus par plusieurs relais.
- [x] Essai local : deux relais de test et un membre injoignable ; la note qui manque à un
      membre est trouvée (`--federation-check`).
- [ ] Essai réel : le relais de test de l’utilisateur (`BITCHAT_TEST_RELAY`) et un strfry local synchronisés : **reporté au
      lot 9**, avec l'utilisateur (installer et configurer strfry sur ses machines).

Fait : `a5::{Federation, Comparer, omissions, sync_filter}`, `docs/federation.md`
(`strfry router` pour le direct, éphémères compris ; `strfry sync` NIP-77 pour le stocké ;
durée de vie des éphémères ≥ 1 h, R11) ; console `--strategy A5 --federation`,
`--federation-check`.

### Lot 9 : mesures réelles ⏳ (lecture seule faite ; le reste avec l'utilisateur)

- [x] Question 3 (NIP-11), par Tor, 467 relais des deux listes, deux passages : 323 à 352
      documents ; 26 à 28 relais inutilisables par leur seule déclaration (21–22 R6 écritures
      restreintes, 8–9 R5 NIP-42, 3 R6 paiement, 1 R7) ; 81–93 avertissements R8, 7 R13.
      Deux défauts trouvés et corrigés : 467 circuits Tor d'un coup font échouer le proxy
      (au plus 16 à la fois, `A6_CONCURRENCY`) ; des serveurs ne répondent jamais (délai de
      20 s, `NIP11_TIMEOUT`).
- [x] Question 6 (moniteurs NIP-66) : des moniteurs actifs publient leurs annonces 10166 sur
      `relay.nostr.watch`, `monitorlizard.nostr1.com`, `relaypag.es` et sur de grands relais ;
      quelques-uns testent l'écriture (`c: write`), la plupart seulement `open`, `read`,
      `nip11`.
- [x] Question 5 (débit), échantillon : `u0` sur `nostr.sathoarder.com`, 7 messages et 203
      présences en une heure (≈ 3,5 événements par minute) ; la limite de 60 par minute d'A4
      suffit.
- [x] Question 4 (coût de capture) : banc du lot 1.
- [ ] Questions 1 et 2 (rétention, accès par Tor) : écrivent sur les relais :
      `bitchat-console --qualify-probe`, **sur décision de l'utilisateur**.
- [ ] A5 réel : le relais de test de l’utilisateur (`BITCHAT_TEST_RELAY`) et un strfry local synchronisés (lot 8).
- [ ] Essai avec l'app BitChat de l'utilisateur (téléportation) : A4 relie un pair iOS et un
      pair Android sans relais commun.
- [ ] Texte proposé pour une version 0.3 du document d'exigences
      (`docs/geohash-relays-requirements-and-alternatives.md`, copie de la version 0.2) : à
      rédiger une fois les questions 1 et 2 mesurées.

### Lot 10 : évaluation et proposition ✅ (à relire par l'utilisateur)

- [x] Tableau final du banc et des mesures : `docs/evaluation.md`. Stratégies **proposées**
      pour la suite (à valider) : A4 tout de suite ; A0 + A6 pour les deux apps ; A1 + A2 puis
      A3 en cible ; A5 en option d'infrastructure. Portages proposés au lot 11 : A0, A6, A1
      (vecteurs figés), le tag d'A3.
- [x] README : une section par stratégie, avec ses résultats.

### Lot 11 : portages Swift et Kotlin ⬜

- [ ] Seulement pour les stratégies retenues au lot 10, pour éviter une FFI dans les apps.
- [ ] Mêmes vecteurs de test que la crate ; ports branchés sur `NostrRelayManager.swift` et
      `NostrRelayManager.kt`.

### Lot 12 : PR vers bitchat et bitchat-android ⬜

- [ ] Modifications des codes Swift (`permissionlesstech/bitchat`) et Kotlin
      (`permissionlesstech/bitchat-android`) pour l'intégration : stratégies retenues
      (portages du lot 11), et une implémentation compatible de l'attente de `OK`, de `AUTH`
      (NIP-42, avec la clé de la cellule) et de `CLOSED`, qui sont ignorés aujourd'hui (§1.5).
      Même comportement que les features `bitchat` et `bitchat-android` de
      `geohash-relays-net`.
- [ ] Une PR par sujet et par app, dans l'ordre du §6 (A0 + A6, puis A1 + A2, puis A3),
      avec la source de chaque règle, les vecteurs de test communs et les résultats du banc.
- [ ] Ouverture des PR seulement sur accord de l'utilisateur (publication extérieure).

### Lot 13 : recensement des relais ✅

Demande de l'utilisateur (30/10/2026) : une base SQLite des relais, `--set` (relais donnés,
normalisés), `--refresh` (listes NIP-65 des relais connus, validées, relais nommés ajoutés sans
doublon, caractéristiques dont la localisation geoIP/geohash et les défauts, en boucle tant que
la liste s'enrichit), `--compat bitchat` (CSV des relais compatibles). Décision G15.

- [x] `geohash-relays` : `census::{relay_url, relays_of, Crawler}`, constantes `CENSUS_*`,
      `KIND_RELAY_LIST`, `comm::RESOLVE_TIMEOUT` ; 4 tests.
- [x] `geohash-relays-net` (feature `census`) : `Connector::resolve` (RESOLVE de Tor, Arti,
      système en direct), `census::{store, geo, refresh, bitchat_csv}`, binaire `relay-census` ;
      `qualify::run` repris de la console ; 6 tests unitaires, 2 d'intégration (relais local,
      proxy SOCKS local).
- [x] Essai réel en lecture seule, par Tor, depuis relay.damus.io, nos.lol et
      relay.primal.net : 495 listes, 35 relais nouveaux en un tour ; relay.primal.net qualifié
      (avertissement R8 : 20 abonnements). Constats : 30 s par relais ne suffisaient pas par
      Tor (porté à 90 s) ; nos.lol ne répond pas par Tor (R3) ; relay.damus.io ne renvoie
      rien, même en direct.
- [x] Localisation sur un vrai fichier DB-IP Lite (octobre 2026, gardé hors dépôt) : un tour
      par Tor depuis la base précédente, 38 relais lus, 26 ont répondu, 4 473 listes,
      673 relais nouveaux ; 25 localisés (relay.nostr.watch : nom non résolu par Tor),
      26 qualifiés (3 refusés : R6) ; CSV BitChat de 22 relais, coordonnées cohérentes avec la
      liste iOS (`*.nostr1.com` à New York).
- [x] `--legacy` (demande du 30/10/2026) : diff du recensement avec les listes iOS et
      Android, par serveur, avec les raisons ; listes reprises d'une copie de moins de 24 h,
      sinon téléchargées par Tor. Essai : iOS 326, Android 392 relais ; les relais des deux
      côtés ont la même place (0 km).
- [ ] `--refresh` complet (plusieurs tours, 700 relais et plus) ; `--probe` seulement sur
      accord de l'utilisateur.

## Journal

- 29/10/2026 : plan créé, puis refondu : 8 modules (`as_`, `a0` … `a6`), 3 features
  exclusives (`bitchat`, `bitchat-android`, `console`), portages Swift et Kotlin à la fin ;
  décisions G1 à G11 à valider. G3 validée ; G5 complétée (compatibilité de licence
  obligatoire, contrôlée en CI) ; G12 ajoutée (crate de communication GPL-3.0-or-later en
  repli). G12 retenue d'emblée : crate `geohash-relays-net` (GPL-3.0-or-later) pour la
  communication et les 3 features ; `geohash-relays` reste pure ; G13 ajoutée (automates
  sans E/S). G1, G2, G4, G6 à G11, G13 validées ; G4 : Tor obligatoire pour la seule
  feature `console` ; G13 : `OK`, `AUTH`, `CLOSED` gérés par les features `bitchat` et
  `bitchat-android`, PR Swift et Kotlin au lot 12 (ajouté) ; G14 ajoutée (constantes dans
  le seul module `params`).
- 29/10/2026 : `v0.1.0` posé sur `main` (inchangé depuis) ; travaux sur la branche
  `strategies`. Lot 0 fait : `as_`, `a0` … `a6`, `params`, `sansio`, `Selected` générique
  (26 tests) ; crate `geohash-relays-net` (18 tests par feature, `arti` compilé, `cargo deny`
  propre) ; bitchat-console sur `-net` (61 tests, `set_strategy` à chaud, `--strategy`) ;
  bitchat-geohash reste au tag `v0.1.0`. Les dépendances `strategies` et `main` des crates
  privées passent par HTTPS et la ligne de commande git.
- 29/10/2026 : lot 1 fait : module `bench` (rendez-vous, charge, stabilité, pannes, coût de
  capture), deux versions plus anciennes des listes figées (`tests/data/README.md`), exemple
  `compare`. Les 322 cellules du §4 sont retrouvées.
- 29/10/2026 : lot 2 fait (A4) ; mesure du débit réel reportée au lot 9.
- 29/10/2026 : lot 3 fait (A0).
- 29/10/2026 : lot 4 fait (A6).
- 29/10/2026 : lot 5 fait (A1).
- 29/10/2026 : lot 6 fait (A2).
- 29/10/2026 : lot 7 fait (A3) ; compatibilité Android vérifiée.
- 29/10/2026 : lot 8 fait (A5) ; essai réel avec strfry reporté au lot 9.
- 29/10/2026 : lot 9, mesures en lecture seule (NIP-11 par Tor, moniteurs NIP-66, débit) ; deux
  défauts corrigés (concurrence des circuits Tor, serveurs muets). Lot 10 : évaluation et
  proposition dans `docs/evaluation.md`, README par stratégie. Restent avec l'utilisateur :
  sondes en écriture, strfry réel, essai avec l'app, choix des stratégies à porter.
- 30/10/2026 : lot 13, recensement des relais (G15) : automate `census` ici, binaire
  `relay-census` dans `geohash-relays-net`, lancement A6 partagé avec la console. Essai réel
  par Tor en lecture seule (un tour) ; localisation à essayer avec un fichier DB-IP.
- 30/10/2026 : lot 13, localisation DB-IP essayée (un tour par Tor : 711 relais connus, CSV
  BitChat de 22 relais).
- 30/10/2026 : lot 13, `relay-census --legacy` (diff avec les listes des apps).
