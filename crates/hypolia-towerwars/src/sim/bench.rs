use uuid::Uuid;

use crate::domain::{Branch, GameMode, LaneHp, LaneId, Level, MobKind, PlayerId, TowerKind};
use crate::map::Map;
use crate::sim::{Event, Match, PendingSpawn, Seat};

const WAVE_TICKS: u64 = 2 * 60 * 20;
const MAX_LEAK_RATIO: f32 = 0.02;
const BUDGETS: [u32; 2] = [2_500, 6_000];
const KINDS: [TowerKind; 6] = [
    TowerKind::Archer,
    TowerKind::Ice,
    TowerKind::Mage,
    TowerKind::Cannon,
    TowerKind::Tesla,
    TowerKind::Slime,
];

fn pid(index: u8) -> PlayerId {
    PlayerId(Uuid::from_u128(u128::from(index) + 1))
}

fn lane(index: u8) -> LaneId {
    LaneId::try_from(index).expect("lane")
}

fn levels() -> [Level; 5] {
    [
        Level::T1,
        Level::T2,
        Level::T3,
        Level::T4(Branch::A),
        Level::T4(Branch::B),
    ]
}

fn path_to(target: Level) -> Vec<Level> {
    match target {
        Level::T1 => vec![Level::T1],
        Level::T2 => vec![Level::T1, Level::T2],
        Level::T3 => vec![Level::T1, Level::T2, Level::T3],
        Level::T4(branch) => vec![Level::T1, Level::T2, Level::T3, Level::T4(branch)],
    }
}

fn total_cost(kind: TowerKind, target: Level) -> u32 {
    path_to(target)
        .into_iter()
        .map(|level| kind.stats(level).cost)
        .sum()
}

fn best_cells(game: &Match) -> Vec<(i32, i32)> {
    let lane_ref = game.lane(lane(0));
    let path = lane_ref.path();
    let mut samples = Vec::new();
    let mut progress = 0.0;
    while progress < path.length() {
        samples.push(path.sample(progress).position);
        progress += 0.5;
    }
    let mut cells: Vec<((i32, i32), usize)> = Vec::new();
    for x in -50..200 {
        for z in -50..200 {
            if lane_ref.is_buildable((x, z)) {
                let (cx, cz) = (x as f32 + 0.5, z as f32 + 0.5);
                let covered = samples
                    .iter()
                    .filter(|p| (p.x - cx).hypot(p.z - cz) <= 8.0)
                    .count();
                cells.push(((x, z), covered));
            }
        }
    }
    cells.sort_by_key(|cell| (std::cmp::Reverse(cell.1), cell.0));
    cells.into_iter().map(|(cell, _)| cell).collect()
}

pub(crate) struct Score {
    pub sent: u32,
    pub leaks: u32,
}

pub(crate) fn defend(towers: &[(TowerKind, Level)], wave: &[MobKind], every: u64) -> Score {
    let seats = [
        Seat {
            player: pid(0),
            lane: lane(0),
        },
        Seat {
            player: pid(1),
            lane: lane(1),
        },
    ];
    let mut game = Match::new(GameMode::Solo, 7, &Map::test_map(), &seats).expect("match");
    let cells = best_cells(&game);
    {
        let world = game.world_mut();
        world.player_mut(pid(0)).expect("me").gold = crate::domain::Gold::new(1_000_000);
        world.lane_mut(lane(0)).hp = LaneHp::full(10_000);
    }
    for (&(kind, target), cell) in towers.iter().zip(&cells) {
        let id = game.place(pid(0), kind, *cell).expect("place");
        for level in path_to(target).into_iter().skip(1) {
            let branch = match level {
                Level::T4(branch) => Some(branch),
                _ => None,
            };
            game.upgrade(pid(0), id, branch).expect("upgrade");
        }
    }
    let mut sent = 0;
    {
        let world = game.world_mut();
        let start = world.elapsed + 1;
        let mut due = start;
        let mut index = 0;
        while due < start + WAVE_TICKS {
            let kind = wave[index % wave.len()];
            world.pending.push(PendingSpawn {
                due_tick: due,
                lane: lane(0),
                kind,
                sender: pid(1),
            });
            index += 1;
            sent += 1;
            due += every;
        }
    }
    let mut score = Score { sent, leaks: 0 };
    for _ in 0..WAVE_TICKS + 60 * 20 {
        game.tick();
        for event in game.drain_events() {
            match event {
                Event::MobLeaked { lane: at, .. } if at == lane(0) => score.leaks += 1,
                _ => {}
            }
        }
    }
    score
}

pub(crate) fn waves() -> Vec<(&'static str, Vec<MobKind>)> {
    vec![
        (
            "mixte",
            vec![
                MobKind::Zombie,
                MobKind::Skeleton,
                MobKind::Spider,
                MobKind::Creeper,
                MobKind::Blaze,
                MobKind::Witch,
            ],
        ),
        (
            "rapides",
            vec![
                MobKind::Rabbit,
                MobKind::Endermite,
                MobKind::Fox,
                MobKind::Spider,
                MobKind::Rabbit,
                MobKind::SkeletonHorse,
            ],
        ),
        (
            "lourds",
            vec![
                MobKind::GiantSlime,
                MobKind::PiglinBrute,
                MobKind::Ghast,
                MobKind::Enderman,
            ],
        ),
    ]
}

pub(crate) fn held_per_minute(towers: &[(TowerKind, Level)], wave: &[MobKind]) -> f32 {
    let holds = |every: u64| {
        let score = defend(towers, wave, every);
        (score.leaks as f32) <= MAX_LEAK_RATIO * score.sent as f32
    };
    let (mut dense, mut sparse) = (1u64, 240u64);
    if !holds(sparse) {
        return 0.0;
    }
    while sparse - dense > 1 {
        let mid = (dense + sparse) / 2;
        if holds(mid) {
            sparse = mid;
        } else {
            dense = mid;
        }
    }
    60.0 * 20.0 / sparse as f32
}

#[test]
#[ignore = "banc d'équilibrage, lancer avec --ignored --nocapture"]
fn tower_value_at_equal_budget() {
    let waves = waves();
    for budget in BUDGETS {
        println!("\n== budget {budget} or : mobs par minute tenus sans fuite (plus = mieux)");
        print!("{:<22}", "tour");
        for (name, _) in &waves {
            print!("{name:>10}");
        }
        println!();
        for kind in KINDS {
            for target in levels() {
                let cost = total_cost(kind, target);
                let count = ((budget / cost) as usize).min(crate::domain::TOWER_LIMIT);
                if count == 0 {
                    continue;
                }
                let towers = vec![(kind, target); count];
                print!("{:<22}", format!("{kind:?} {target:?} x{count}"));
                for (_, wave) in &waves {
                    print!("{:>10.1}", held_per_minute(&towers, wave));
                }
                println!();
            }
        }
    }
}

#[test]
#[ignore = "banc d'équilibrage, lancer avec --ignored --nocapture"]
fn support_towers_pay_for_themselves() {
    let waves = waves();
    let archers = vec![(TowerKind::Archer, Level::T3); 15];
    let mut with_ice = vec![(TowerKind::Archer, Level::T3); 11];
    with_ice.extend([(TowerKind::Ice, Level::T3); 3]);
    let mut with_slime = vec![(TowerKind::Archer, Level::T3); 11];
    with_slime.extend([(TowerKind::Slime, Level::T3); 2]);
    let mut with_beacon = vec![(TowerKind::Archer, Level::T3); 11];
    with_beacon.extend([(TowerKind::Beacon, Level::T2); 2]);
    let mut mixed = vec![(TowerKind::Archer, Level::T3); 5];
    mixed.extend([(TowerKind::Cannon, Level::T3); 2]);
    mixed.extend([(TowerKind::Mage, Level::T3); 2]);
    mixed.extend([(TowerKind::Ice, Level::T3); 2]);
    println!("\n== ~6000 or : mobs par minute tenus sans fuite");
    for (name, towers) in [
        ("Archer T3 x15", archers),
        ("Archer x11 + Glace x3", with_ice),
        ("Archer x11 + Slime x2", with_slime),
        ("Archer x11 + Beacon x2", with_beacon),
        ("Mélange 4 tours", mixed),
    ] {
        let cost: u32 = towers
            .iter()
            .map(|(kind, level)| total_cost(*kind, *level))
            .sum();
        print!("{:<26}{:>6}", name, cost);
        for (_, wave) in &waves {
            print!("{:>10.1}", held_per_minute(&towers, wave));
        }
        println!();
    }
}
