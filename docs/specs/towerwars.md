# TowerWars

Statut : implémenté (non déployé : TowerWars reste désactivé par défaut dans le chart)
Date : 2026-11-23
Id : `tw`
Valeurs : [`../gdd.md`](../gdd.md)

## Goal

Un mini-jeu TowerWars jouable de bout en bout sur le réseau : noyau de simulation pur et testé, plugin rspigot, intégration orchestrateur, Helm et Docker, sur le modèle de SheepWars.

## Context

- SheepWars est le seul mini-jeu : `crates/hypolia-sheepwars`, `hypolia-orchestrator` (file, flotte, CRD `SheepWarsServer`), Helm, Docker, NPC du lobby, progression (`MatchReport`).
- `ServerKind` ne connaît que `Lobby` et `SheepWars`. L'orchestrateur est écrit pour un seul jeu (file unique, un réconciliateur, un CRD).
- Le SDK rspigot est épinglé par `rspigot.ref` (ADR 0010).

## Glossary

| Terme | Définition | Synonyme rejeté |
|---|---|---|
| voie | Chemin d'un défenseur, du portail d'entrée au portail de sortie. 6 par partie. | lane (code seulement : `Lane`) |
| défenseur | Joueur ou équipe qui possède une voie et la défend avec des tours. | cible |
| attaquant | Joueur qui a acheté et envoyé un mob. | envoyeur |
| envoi | Achat d'un mob : l'or est dépensé, l'income monte, un mob apparaît sur chaque voie adverse vivante. | spawn |
| income | Or crédité à chaque cycle de 120 ticks. Ne monte que par les envois. | revenu |
| cycle | 120 ticks entre deux versements d'income. | tick d'income |
| charge | Unité de stock d'un type de mob. Max 10, recharge 30 ticks. | stack |
| progress | Distance en blocs parcourue par un mob depuis le spawn. | avancement |
| mort subite | Phase qui démarre à 15:00 : mobs plus rapides et plus gros, plus aucun soin ni vol de vie. | sudden death |
| simulation | Le noyau pur `Match` : aucune dépendance au SDK, avance par `tick()`. | moteur |

## Arbitrages sur le document de design

| Sujet | Décision |
|---|---|
| Cible d'un envoi | Tous les adversaires vivants (pas de voie suivante). |
| Vol de vie | Aucun mob n'a *Leech*. Seul le Wither donne +2 PV à l'attaquant, plafonné aux PV de départ. Coupé en mort subite. |
| Kill bounty | 12 % du coût d'envoi, arrondi vers le bas, sans income. Milieu de la fourchette 10 à 15 %. |
| Mort subite | À partir de 18 000 ticks, `n = 1 + (elapsed - 18000) / 1200`. Vitesse x(1 + 0,25 n) calculée à chaque tick. PV x(1 + 0,5 n) figés à l'apparition. Soin de la Witch et régénération du Wither coupés. |
| Égalité | Si les derniers défenseurs tombent au même tick, la partie est nulle (`Won { lane: None }`, `MatchOutcome::Draw`). |
| Joueurs absents | Une voie sans propriétaire ne reçoit aucun mob et ne compte pas. En Solo il faut au moins 4 joueurs. |
| Teams | 2 propriétaires par voie, or individuel, income individuel, limite de 80 tours partagée, 25 PV partagés. Un envoi coûte l'or de celui qui l'achète. |
| Armure | `dégâts reçus = dégâts x (1 - armure)`. L'armure du mob et la réduction d'armure des tours se cumulent en additionnant les pourcentages, plafonnée à 90 %. Les dégâts magiques (Mage, brûlure) ignorent l'armure. |
| Stock | Une charge se recharge toutes les 30 ticks par type, jusqu'à 10. Stock initial : 10. |
| Aléa | Tout hasard (critique, esquive, gel) vient d'un générateur `SplitMix64` initialisé par la graine de la partie. Même graine, mêmes commandes, même résultat. |
| Unités | Positions en blocs (`f32`). Une vitesse en b/s vaut `v / 20` bloc par tick. Une cadence en secondes vaut `round(s x 20)` ticks. |

## Invariants

1. Une voie compte entre 0 et 80 tours. [type : `Lane::place` renvoie `Result`, la grille est privée]
2. Un `LaneId` vaut 0 à 5. [type : `TryFrom<u8>`]
3. L'or ne devient jamais négatif. [type : `Gold::checked_sub`, `Match::send` et `Match::place` passent par lui]
4. Un T4 a une branche, un T1 à T3 n'en a pas. [type : `Level::{T1, T2, T3, T4(Branch)}`]
5. Une voie éliminée ne reçoit plus de mob et ses tours ne tirent plus. [test]
6. Un mob avance de `vitesse / 20` bloc par tick, jamais à reculons. [test]
7. Une même graine et les mêmes commandes donnent les mêmes événements. [test : simulation déterministe]
8. Le total remboursé à la revente vaut `floor(0,8 x total investi)`. [test]
9. Aucun mob ne s'envoie hors du tier débloqué ni sans charge. [test]
10. Les PV d'une voie sont plafonnés aux PV de départ. [type : `Hp::heal_to_cap`]

## Out of scope

- Équilibrage final (valeurs du GDD prises telles quelles).
- Cosmétiques TowerWars, progression d'XP spécifique (le `MatchReport` est envoyé avec les mêmes règles que SheepWars, sans cosmétiques nouveaux).
- Auto-send du slot 9.
- Parties de moins de 4 joueurs en Solo.

## Design

### Crate `hypolia-towerwars`

```
src/
  lib.rs main.rs config.rs orchestrator.rs plugin.rs
  domain/   ids money mob tower status error      (types purs, catalogues)
  map/      layout path generate                  (map.toml, waypoints, carte de test)
  sim/      rng lane mobs towers combat economy match event
  game/     engine session view                   (adaptateur SDK : commandes, affichage)
  hud/      scoreboard actionbar bossbar
```

`sim` et `domain` et `map` ne dépendent pas de `rspigot-plugin`. Le plugin traduit les `Event` de `Match` en appels SDK.

### Tick (ordre fixe dans `Match::tick`)

1. Versement d'income si `elapsed % 120 == 0` (hors tick 0).
2. Recharge des charges de mobs.
3. Mort subite : met à jour le multiplicateur.
4. Apparition des mobs en file d'envoi (décalage de 2 ticks).
5. Statuts : décompte, dégâts de brûlure et d'aura, soins, régénération.
6. Tours : décrément du cooldown, ciblage, tir, application des dégâts et statuts.
7. Capacités des mobs (blink) puis déplacement (vitesse sous seuil, ralentissements).
8. Morts : bounty, divisions, effets de mort, jusqu'à ce qu'il n'y ait plus de nouvelle mort dans le tick.
9. Arrivées au portail : dégâts à la voie, vol de vie. Un mob mort ne fuit jamais.
10. Éliminations, victoire ou match nul.

### Ports et adaptateurs

| Port | Propriétaire | Dispatch | Adaptateur |
|---|---|---|---|
| `Match` (struct concrète, pas de trait) | `sim` | — | `game::Game` l'appelle |
| `Map::from_json` + `Map::test_map()` | `map` | fonctions | fichier `map.json` (`TOWERWARS_MAP_PATH`), carte de test générée par défaut |
| `OrchestratorClient` | `orchestrator.rs` | struct concrète (copie du modèle SheepWars) | HTTP |

### Erreurs

`SimError` (commande refusée : `NotEnoughGold`, `LockedTier`, `NoCharge`, `LaneFull`, `OccupiedCell`, `OutOfBuildBounds`, `NotOwner`, `MaxLevel`, `UnknownTower`, `LaneEliminated`), `MapError` (parse, validation), `OrchestratorClientError`.

### Intégration (W6 à W9)

Constat : l'orchestrateur est écrit pour un seul jeu. Décision à prendre dans l'ADR 0011 avant W6, après lecture de `engine.rs` (file, flotte, k8s, CRD) :

- `ServerKind::TowerWars` dans `hypolia-protocol`.
- Une file et une flotte par jeu, plutôt que des conditions `if kind` éparpillées.
- CRD `TowerWarsServer` calqué sur `SheepWarsServer`, ou CRD générique `GameServer` portant un `game`. Alternative écartée à ce stade : réutiliser `SheepWarsServer` avec un champ `game`, car le nom du CRD ment.
- Helm : template de Pod, CRD, RBAC, valeurs. Docker : cible `towerwars` dans `Dockerfile`, `docker-bake.hcl`, `build-images.sh`, `images.yml`.
- Lobby : NPC et menu d'entrée en file TowerWars, progression par jeu.

## Acceptance (Gherkin)

Chaque ligne devient un test dans `crates/hypolia-towerwars/tests/` (scénarios sur `Match`) ou dans le module concerné.

@spec-tw-1 Scénario : départ. Étant donné 6 joueurs en Solo, quand la partie démarre, chacun a 100 or, 10 d'income, 20 PV et 10 charges de chaque mob de tier 1.
@spec-tw-2 Scénario : income. Quand 120 ticks passent sans envoi, chaque joueur gagne 10 or.
@spec-tw-3 Scénario : envoi. Quand A envoie un Silverfish à 100 or, A a 85 or, 12 d'income, et chaque voie adverse vivante reçoit un Silverfish ; la voie de A n'en reçoit pas.
@spec-tw-4 Scénario : stock. Quand A envoie 11 Silverfish, le onzième est refusé `NoCharge`. 30 ticks plus tard, une charge est revenue.
@spec-tw-5 Scénario : déblocage. Avant 3000 ticks un Spider est refusé `LockedTier` ; après il est accepté.
@spec-tw-6 Scénario : déplacement. Un Silverfish (3,2 b/s) a `progress = 3,2` après 20 ticks et suit les waypoints dans les virages.
@spec-tw-7 Scénario : base. Un mob de 1 dégât atteignant le portail retire 1 PV au défenseur, ne rapporte pas d'or, et disparaît.
@spec-tw-8 Scénario : placement. Une tour sur une case hors `build_bounds` ou occupée est refusée ; la 81e tour est refusée `LaneFull`.
@spec-tw-9 Scénario : revente. Une Archer T3 (400 investis) revendue rend 320 or.
@spec-tw-10 Scénario : amélioration. T3 → T4-A coûte 600 ; une T4 ne s'améliore plus ; choisir A interdit B.
@spec-tw-11 Scénario : ciblage. Avec deux mobs à portée, FIRST vise le plus avancé, LAST le moins avancé, STRONGEST le plus de PV, WEAKEST le moins.
@spec-tw-12 Scénario : tir. Une Archer T1 inflige 12 dégâts toutes les 20 ticks à un mob sans armure et tue un Silverfish (35 PV) en 3 tirs ; le kill rapporte `floor(15 x 0,12) = 1` or.
@spec-tw-13 Scénario : armure. Un Zombie (10 %) reçoit 10,8 d'un tir d'Archer T1. Un tir de Mage ignore l'armure.
@spec-tw-14 Scénario : glace. Une Ice T1 ralentit de 25 % pendant 50 ticks ; deux ralentissements ne se cumulent pas (le plus fort gagne) ; un Spider n'en subit que 50 %.
@spec-tw-15 Scénario : brûlure. Un Mage T1 applique 5/s pendant 60 ticks ; un Blaze y est immunisé.
@spec-tw-16 Scénario : Tesla. Une Tesla T1 touche 3 mobs différents dans un rayon de rebond de 3,5.
@spec-tw-17 Scénario : beacon. Un Beacon T1 à portée donne +10 % de vitesse d'attaque ; un troisième beacon sur la même tour est ignoré.
@spec-tw-18 Scénario : capacités. Slime géant → 2 slimes moyens à la mort ; Enderman → blink de 4 blocs une seule fois sous 50 % PV ; Witch → 30 PV/s aux alliés à 3 blocs ; Creeper → étourdit les tours à 2,5 blocs pendant 20 ticks à sa mort.
@spec-tw-19 Scénario : mort subite. À 18 000 ticks, un mob apparu a ses PV x1,5 et sa vitesse x1,25 ; une minute plus tard x2 et x1,5 ; la Witch ne soigne plus.
@spec-tw-20 Scénario : élimination. Une voie à 0 PV est éliminée, ne reçoit plus d'envoi ; le dernier défenseur vivant gagne ; la partie se termine.
@spec-tw-21 Scénario : Wither. Un Wither qui passe donne +2 PV à son attaquant, plafonné à 20 ; en mort subite, 0.
@spec-tw-22 Scénario : Teams. En Teams, deux coéquipiers partagent PV (25) et limite de 80 tours, gardent chacun leur or et leur income.
@spec-tw-23 Scénario : déterminisme. Deux parties avec la même graine et les mêmes commandes produisent la même suite d'événements.
@spec-tw-24 Scénario : carte. La carte de test a 6 voies de 75 à 95 blocs avec 4 à 6 virages ; un `map.json` invalide est refusé avec une erreur précise.
@spec-tw-25 Scénario : plugin. Les commandes `/towerwars join|leave|hub|version|forcestart|stop`, le heartbeat et le `MatchReport` suivent le contrat SheepWars (test de bout en bout avec le harnais rspigot).
@spec-tw-26 Scénario : orchestration. Un joueur qui rejoint la file TowerWars est envoyé à un serveur TowerWars ; la file SheepWars n'est pas affectée.

## Simulation

Oui, légère : la propriété « même graine, mêmes commandes, mêmes événements » (@spec-tw-23) est testée par une partie scriptée de 20 minutes de jeu (24 000 ticks), exécutée deux fois, avec comparaison des journaux d'événements. Pas de simulation déterministe distribuée : le jeu est mono-processus.

## Workstreams

Règles : W2 à W4 s'exécutent dans le même checkout, donc les sous-agents n'exécutent que des tests ciblés (`cargo test -p hypolia-towerwars <module>::`). Seul l'orchestrateur lance la suite complète.

| Id | Mission | Dépend de | Fichiers possédés |
|---|---|---|---|
| W0 | Contrats gelés : types du domaine, structs `Mob`, `Lane`, `Event`, signatures de `Match` | — | orchestrateur : `domain/*` (types), `sim/{event,rng,match}.rs`, `lib.rs`, `Cargo.toml` |
| W1 | Carte : `map.toml`, validation, échantillonnage du chemin, carte de test | W0 | `map/*`, `tests/map.rs` |
| W2 | Économie et mobs : income, stocks, déblocage, envoi, déplacement, arrivée, mort subite, bounty | W0, W1 | `sim/{economy,mobs}.rs`, `domain/mob.rs` (corps du catalogue), `tests/economy.rs` |
| W3 | Tours : catalogue, grille, placement, amélioration, revente, ciblage | W0 | `sim/{lane,towers}.rs`, `domain/tower.rs` (corps du catalogue), `tests/towers.rs` |
| W4 | Combat et statuts : tirs par type de tour, statuts, capacités des mobs | W2, W3 | `sim/combat.rs`, `domain/status.rs`, `tests/combat.rs` |
| W5 | Plugin : `game/*`, `hud/*`, `plugin.rs`, `config.rs`, `orchestrator.rs`, `main.rs`, affichage Display, particules, GUI | W4 | idem |
| W6 | ADR 0011 et protocole : `ServerKind::TowerWars`, file et flotte par jeu dans l'orchestrateur | W0 | `hypolia-protocol`, `hypolia-orchestrator` |
| W7 | CRD, réconciliateur, pod builder TowerWars | W6 | `crd.rs`, `k8s.rs`, `config.rs` de l'orchestrateur |
| W8 | Helm, Dockerfile, bake, CI | W7 | `deploy/**`, `Dockerfile`, `docker-bake.hcl`, `.github/workflows/images.yml` |
| W9 | Lobby : NPC et menu TowerWars ; progression par jeu | W6 | `hypolia-lobby`, `hypolia-progression*` |

Points de convergence, écrits par l'orchestrateur seul : `Cargo.toml` racine et du crate, `Cargo.lock` (régénéré par `cargo`), `lib.rs`, tous les `mod.rs`, `README.md`.

## Frozen contracts

Dans le code, écrits par W0 : `crates/hypolia-towerwars/src/domain/*` (types, signatures des catalogues), `sim/event.rs` (`Event`), `sim/match.rs` (signatures publiques de `Match`). Les sous-agents ne modifient pas ces signatures ; ils signalent le besoin.

## Risks / open questions

| Risque | Responsable |
|---|---|
| Le chemin local du SDK (`/opt/minecraft/dev/rspigot`) n'existe pas : build local via un worktree du commit épinglé et `Cargo.toml` modifié en `skip-worktree`. À défaire avant tout commit de `Cargo.toml`. | orchestrateur |
| Les `BlockDisplay` / `TextDisplay` / particules sont-ils exposés par le SDK au commit épinglé ? À vérifier avant W5 ; sinon, repli sur `ArmorStand` et `Block` et amendement du spec. | orchestrateur (avant W5) |
| Forme multi-jeu de l'orchestrateur (file, CRD) : ADR 0011. | orchestrateur (avant W6) |
| Carte réelle : la carte de test est générée en code ; une vraie map se charge via `map.toml`. | Baptiste |

## Verify (exit)

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
helm lint deploy/charts/hypolia
helm template deploy/charts/hypolia | kubectl apply --dry-run=client -f -
```

(Le test Postgres de l'orchestrateur ne doit pas être sauté silencieusement : voir `cicd`.)

## Amendments

- 2026-11-23 : la carte est en JSON (`map.json`), pas en TOML : pas de nouvelle dépendance, même choix que `arena.json` de SheepWars.
- 2026-11-23 : égalité de dernière élimination = match nul (l'ancienne règle « dernier traité gagne » favorisait la voie au plus grand indice).
- 2026-11-23 : un joueur parti (`forfeit`) ne peut plus agir (`SimError::PlayerLeft`) ; le bounty ne va jamais à un propriétaire parti.
- 2026-11-23 : valeurs absentes du GDD, choisies dans `domain/tower_catalog.rs` : les T4 héritent du T3 (dégâts, cadence, portée) sauf valeur donnée ; Ice : portée = rayon d'effet ; Pyromancer brûlure 80 ticks ; Necromancer rayon d'explosion 3,0 ; Cluster Bomb : impact principal du T3 puis 4 sous-munitions ; Arc Reactor = 10 cibles (9 rebonds), Overload = 4 cibles (3 rebonds) ; Gold Mine rayon 5,5.
- 2026-11-23 : règles de combat choisies : le Ghast subit +25 % de dégâts de l'Archer et de la Tesla ; l'artillerie (Cannon, Cluster) et les flaques ne touchent pas un mob volant ; le Ravager ignore les flaques ; Blizzard tente un gel par mob toutes les 20 ticks ; une esquive de Skeleton annule le tir entier (y compris brûlure de zone et flaque).
- 2026-11-23 : Teams : minimum 4 joueurs (2 équipes).
- 2026-11-23 : plugin : un `/towerwars leave` ou `hub` déclare forfait ; en Solo la voie est éliminée, en Teams le coéquipier continue. Le SDK au commit épinglé n'a pas de visibilité d'entité par joueur : seules les voies observées par un joueur connecté sont rendues, mouvements toutes les 2 ticks avec interpolation client.
- 2026-11-23 : le test de déterminisme (24 000 ticks, mort subite et tier 4 atteints) vit dans `tests/spec_gaps.rs`.

## Amendement 2026-11-23 — taille de partie choisie au lobby

Remplace « min. 4 joueurs / flag dev solo ». Décision utilisateur.

- Le PNJ du lobby ouvre un menu d'une ligne (9 slots) : 4 laines blanches aux slots 1, 3, 5, 7 (index 0..8), quantité = nombre de participants : 1 Solo, 2 Duo, 3 Trio, 4 Quatuor. Clic = rejoindre la file avec `match_size` = quantité, puis fermer le menu. Un joueur déjà en file TowerWars qui clique le PNJ quitte la file (bascule existante), sans menu.
- Chaque participant a sa propre voie (mode `Solo` du simulateur). Les voies non occupées jusqu'à 4 voies sont tenues par des bots (déjà implémentés, ids `u128::MAX - i`, exclus des rapports). Un bot envoie toutes les 5 s le mob le plus cher qu'il peut acheter, pour que le joueur ait des vagues à affronter. Le mode `Teams` n'est pas exposé par le menu.
- Contrat protocole (gelé, déjà dans `hypolia-protocol`) : `JoinQueueRequest.match_size: u8` (0 = défaut ; borné 1..=4 par l'orchestrateur) ; `HeartbeatResponse { expected_players: Option<u32> }`.
- Orchestrateur, jeu TowerWars uniquement : la file est groupée par `match_size`. Un groupe de taille s part quand il compte ≥ s joueurs en attente : les s plus anciens de ce groupe sont envoyés ensemble sur un serveur prêt, VIDE (aucun joueur, aucun transfert en cours, `expected_players` non posé). Pas de backfill. Le serveur reçoit `expected_players = s` dans la réponse de ses heartbeats (200 + JSON `HeartbeatResponse` ; sinon 204 comme aujourd'hui). `match_size` 0 ou hors 1..=4 : traité comme 1. La file persistée (Postgres) garde `match_size` (colonne `smallint NOT NULL DEFAULT 0`, migration `0004_queue_match_size.sql`, idempotente).
- Plugin : `min_players` par défaut 1 (plus de flag `TOWERWARS_DEV_SOLO` ni de `soloDev` Helm : supprimés). Avec orchestrateur configuré, la partie ne démarre qu'une fois `expected_players` connu ET effectif (joueurs présents) ≥ `expected_players`. Sans orchestrateur : seuil `min_players`. Si `expected_players == 1` : démarrage immédiat, sans décompte ; sinon décompte habituel. Le décompte s'annule si l'effectif repasse sous `expected_players`. Les bots complètent jusqu'à 4 voies, toujours (pas seulement en dev).
- Hors périmètre : compteurs de file par taille dans le lobby (le total reste affiché).

## Amendement 2026-11-24 — équilibrage, bots et stade

Décisions prises après des parties de test en solo contre les bots. Mesures : `cargo run --release -p hypolia-towerwars --example balance`.

- Bots (`game/bot.rs`) : toutes les 10 s, un bot envoie le mob le plus cher payable avec 30 % de son or, puis construit et améliore ses tours avec le reste. Au plus 40 tours, niveau T3 maximum. Avant : tout l'or toutes les 5 s, aucune tour ; un joueur seul était éliminé vers 1:07.
- Plafond d'income : `MAX_INCOME = 10 000`. Au-delà, un envoi coûte toujours son or mais n'augmente plus l'income. Avant : un joueur qui spamme dépassait 170 000 d'income.
- Plafond de mobs : `MAX_MOBS_PER_LANE = 80` (mobs vivants + apparitions en attente). Une voie pleine ne reçoit plus de mob ; si toutes les voies adverses sont pleines, l'envoi est refusé (`SimError::LanesSaturated`) sans rien débiter. Avant : plus de 300 mobs simultanés sur une voie, le serveur ralentissait.
- Résultat simulé (solo, 3 bots) : un joueur qui ne fait que défendre perd vers 13:00 ; un joueur qui attaque gagne vers 13:55, avant la mort subite.
- Wither : affiché comme un Wither Skeleton (échelle 1,1), car l'entité Wither affiche une barre de boss par mob. Les règles du mob ne changent pas.
- Carte : stade commun, 6 voies en 2 rangées de 3, sans mur entre elles ; toutes les voies sont construites, même sans joueur. Un panneau flottant au-dessus de chaque voie affiche nom, PV, income, tours et mobs.

## Amendement 2026-11-24 — carte « Impérial » pour la production

- Thème original d'inspiration impériale (pas une reproduction d'une carte existante), généré par le code dans `game/imperial.rs` : palais à deux toits (colonnes de manguier écorcé, murs de grès, toits en tuiles de deepslate, bordures en terre cuite jaune, faîtage en or), une pagode tous les trois modules, terrasse avec statues et lanternes, douves et canaux d'eau entre les voies, galerie couverte surélevée au-dessus de la place centrale.
- Voies : cases constructibles en damier terre cuite rouge / verte, chemin des mobs en andésite polie bordé d'eau, rebord en pierre taillée, lanternes aux coins.
- Toute l'eau est fermée (test `every_water_block_is_enclosed_so_it_never_flows`).
- Pas de fichier de monde ni de carte S3 : la carte est construite une seule fois au démarrage du serveur. Tous les chunks de l'arène sont chargés de force puis remplis une fois chacun (`Arena::prepare`, `Arena::place_once`). Mesure locale : 71 084 blocs sur 84 chunks en environ 16 s, aucun bloc perdu. Le plugin n'envoie son premier heartbeat « prêt » qu'après la construction.
- `cargo run -p hypolia-towerwars --example arena_stats` affiche le nombre de blocs et de chunks.

## Amendement 2026-11-24 — murs de 3 cases et carte plus grandiose

- Voies : les cases constructibles forment des murs de 3 cases de large (cases à 2 à 4 blocs du chemin), 2 blocs au-dessus du chemin des mobs (1 de plus qu'avant). Les rangées du serpentin sont espacées de 6 blocs, pour qu'un seul mur de 3 cases les sépare ; le chemin mesure 87 blocs. La portée des tours se mesure au sol (x, z), la hauteur des murs ne change donc pas le jeu. Équilibrage re-simulé : défenseur pur éliminé vers 13:15, joueur actif vainqueur vers 14:00.
- Décor : palais à trois niveaux de toits (faîte à 26 blocs du sol), une pagode à trois étages tous les deux modules, quatre tours d'angle à cinq étages (sommet à plus de 100 blocs de hauteur absolue), galerie surélevée de 7 blocs de large avec un pavillon central à trois toits.
- Plus de mur de barrière : un joueur qui sort du stade (ou de la plateforme d'attente) est ramené, comme pour une chute dans le vide.
- `TOWERWARS_DECOR=false` construit la carte sans décor (sol, voies, canaux). Utilisé par les tests de bout en bout ; la production garde le décor (valeur par défaut).

## Amendement 2026-11-24 — carte précuite sur S3 / RustFS

- `deploy/scripts/bake-map.sh <nom>` lance un serveur rspigot temporaire avec le plugin, attend « arène construite … pending_chunks=0 », sauvegarde (`save-all flush`, `stop`) puis produit `dist/towerwars-maps/<nom>/{world.zip,arena.json}`. `arena.json` est la carte TowerWars (`export_map`), le monde exclut `playerdata`, `stats`, `advancements`.
- Téléversement : `upload-map.sh towerwars <nom> <dossier>` de hypolia/platform. Les maps de chaque jeu vivent sous leur propre préfixe (`towerwars/<nom>/`, `sheepwars/<nom>/`) : la rotation SheepWars ne voit jamais les cartes TowerWars.
- Chart : si `s3.enabled` et `orchestrator.towerwars.map` sont définis, le Pod TowerWars reçoit l'`initContainer` `map-sync` (préfixe `towerwars/`), et le plugin reçoit `TOWERWARS_MAP_PATH=/data/arena.json` et `TOWERWARS_WORLD_PREBUILT=true` (volume `/data` en lecture seule). Sans carte, rien ne change.
- `map-sync` lit aussi `TOWERWARS_MAP_NAME` (déjà injecté par l'orchestrateur).
- Plugin : avec `TOWERWARS_WORLD_PREBUILT=true`, les chunks de l'arène sont chargés de force mais aucun bloc n'est posé. Mesure locale : prêt en 0,1 s au lieu de 22 s.
- Le bake doit être refait à chaque changement de carte ou de décor dans le code.

## Amendement 2026-11-24 — SDK rspigot 0.1.0 et lobby

- `rspigot.ref` pointe sur le commit du tag `0.1.0` de rspigot (`103f2d34`), selon l'ADR 0010. Tout le workspace compile et passe clippy contre cette version.
- Lobby : la sidebar et la tab list affichent la file TowerWars à côté de SheepWars, le pied de la tab list propose `/towerwars`, la description du menu Mini-Jeux annonce 1 à 4 joueurs, et le menu staff Serveurs indique le jeu de chaque serveur.

## Amendement 2026-11-25 — Solo et Duo sans bots, arrivée, mobs rapides, équilibrage

Remplace « les bots complètent jusqu'à 4 voies, toujours » et le menu Solo/Duo/Trio/Quatuor. Décision utilisateur.

- Modes : `match_size` 1 = Solo (une voie par joueur, 2 à 6 joueurs), 2 = Duo (2 joueurs par voie, 2 à 6 équipes). Trio et Quatuor retirés (lobby et orchestrateur).
- Orchestrateur : un groupe de mode part quand il atteint le maximum, ou quand il atteint le minimum et que le plus ancien attend depuis `TOWERWARS_GATHER_SECS` (20 s). Un groupe d'amis n'est jamais séparé ; en Duo, un groupe de 2 forme une équipe.
- Protocole : `HeartbeatResponse.teams: Vec<Vec<Uuid>>` (une liste par voie, champ optionnel). Le plugin en déduit le mode de la partie (une équipe de 2 = Duo) et place chaque équipe sur sa voie. Les joueurs présents absents de la liste sont placés après.
- Plugin : plus de bots par défaut. `TOWERWARS_BOTS=true` les remet (tests locaux). Une partie demande au moins 2 voies occupées (`MIN_LANES`), sinon elle ne démarre pas. Sans orchestrateur, le seuil ne descend jamais sous le minimum du mode.
- Arrivée : titre et 3 lignes de règles à l'entrée, titres 5-4-3-2-1 avec son à la fin du décompte, titre « Défends ta voie ! », nom du coéquipier et rappel des mobs rapides au départ.
- Préparation : pendant `TOWERWARS_PREPARATION_SECS` (20 s), le plugin refuse les envois (`SimError::Preparing`) ; la BossBar affiche le temps restant.
- Mobs rapides : Lapin (T1, 6,2 b/s), Renard (T2, x1,8 tant qu'il n'a pas été touché), Cheval squelette (T3, 5,0 b/s, résiste à 60 % des ralentissements). Valeurs dans le GDD.
- Équilibrage, mesuré par `cargo test --release -p hypolia-towerwars --lib bench:: -- --ignored --nocapture` (débit de mobs tenu sans fuite, à budget égal) :
  - Tesla : chaque rebond fait -30 % (`CHAIN_FALLOFF`). Avant : 2 Tesla T3 tenaient 100 % d'une vague mixte, contre 47 % pour 6 Archers T3 au même prix.
  - Mage : plus de dégâts et de brûlure, T2/T3/T4 moins chers. Avant : moitié moins rentable que l'Archer.
  - Archer T1 : 50 → 60 or (le spam de T1 dépassait tout le reste à petit budget).
  - Glace et Slime moins chers (tours de soutien qui ne remboursaient pas leur coût).
  - Résultat : à budget égal, Archer, Mage et Tesla tiennent des débits proches ; le Canon et la Glace dominent les vagues denses au sol mais s'effondrent contre les mobs rapides et volants.
