# Hypolia TowerWars

Mini-jeu TowerWars : carte impériale, modes Solo (une voie par joueur) et Duo (équipes de 2).

Images publiées :

- `ghcr.io/hypolia/towerwars` : le plugin, lancé en sidecar à côté du serveur.
- `ghcr.io/hypolia/towerwars-server` : rspigot avec le template de `server/`.

Dépend de [`hypolia/sdk`](https://github.com/hypolia/sdk), cloné dans `../sdk` et figé par `sdk.ref`, et de rspigot, attendu dans `/opt/minecraft/dev/rspigot` et figé par `rspigot.ref`.

## Lancer en local

Serveur rspigot (`:25568`, gRPC `:25582`) :

```bash
unzip -o server/world.zip -d server
/opt/minecraft/dev/rspigot/target/release/rspigot --world-dir ./server
```

Plugin :

```bash
RSPIGOT_URL=http://127.0.0.1:25582 RSPIGOT_PLUGIN_NAME=hypolia-towerwars cargo run --release -p hypolia-towerwars
```

Le proxy Velocity se lance depuis [`hypolia/platform`](https://github.com/hypolia/platform).

Tester seul en local : `TOWERWARS_BOTS=true TOWERWARS_MIN_PLAYERS=1` complète la partie avec 3 bots.

## Construire les images

```bash
docker buildx bake
```

Optionnel : `ORCHESTRATOR_URL=http://127.0.0.1:8080` et `SERVER_ID=towerwars-1` si l'orchestrateur local tourne.

En jeu : `/towerwars join|leave|hub`, et pour `op` ou la permission `towerwars.admin` : `/towerwars forcestart|stop`.

Variables : `TOWERWARS_MODE` (`solo` ou `teams`), `TOWERWARS_MIN_PLAYERS` (1), `TOWERWARS_MAX_PLAYERS` (12), `TOWERWARS_PREPARATION_SECS` (20), `TOWERWARS_BOTS` (`false`, tests locaux seulement), `TOWERWARS_COUNTDOWN_SECS`, `TOWERWARS_EPHEMERAL`, `TOWERWARS_SEED`, `TOWERWARS_MAP_PATH`. Sans `TOWERWARS_MAP_PATH`, le plugin construit sa carte intégrée au démarrage.

## Carte précuite

Sans carte S3, chaque pod construit sa carte au démarrage (~22 s). Avec une carte précuite, le monde arrive déjà construit par `map-sync`.

```bash
./deploy/scripts/bake-map.sh imperial
../platform/deploy/scripts/upload-map.sh towerwars imperial dist/towerwars-maps/imperial
```

Refaire le bake à chaque changement de `map/generate.rs`, `game/arena.rs` ou `game/imperial.rs`.

Contrat du sidecar : [`docs/PLUGIN_CONTRACT.md`](./docs/PLUGIN_CONTRACT.md). Architecture de flotte : [ADR 0011 dans hypolia/platform](https://github.com/hypolia/platform/blob/main/docs/adr/0011-un-moteur-de-flotte-par-jeu.md).
