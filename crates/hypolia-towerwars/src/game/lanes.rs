use crate::domain::{GameMode, LaneId, PlayerId};
use crate::sim::Seat;

pub fn assign_seats(mode: GameMode, players: &[PlayerId]) -> Vec<Seat> {
    let per_lane = mode.owners_per_lane();
    players
        .iter()
        .enumerate()
        .filter_map(|(index, player)| {
            let lane = u8::try_from(index / per_lane).ok()?;
            Some(Seat {
                player: *player,
                lane: LaneId::try_from(lane).ok()?,
            })
        })
        .collect()
}

pub fn seat_lineup(mode: GameMode, lineup: &[Vec<PlayerId>], roster: &[PlayerId]) -> Vec<Seat> {
    let per_lane = mode.owners_per_lane();
    let mut teams: Vec<Vec<PlayerId>> = lineup
        .iter()
        .map(|team| {
            team.iter()
                .filter(|player| roster.contains(player))
                .take(per_lane)
                .copied()
                .collect::<Vec<_>>()
        })
        .filter(|team| !team.is_empty())
        .collect();
    let unlisted: Vec<PlayerId> = roster
        .iter()
        .filter(|player| !teams.iter().flatten().any(|seated| seated == *player))
        .copied()
        .collect();
    teams.extend(unlisted.chunks(per_lane).map(<[PlayerId]>::to_vec));
    teams
        .iter()
        .zip(LaneId::ALL)
        .flat_map(|(team, lane)| {
            team.iter().map(move |player| Seat {
                player: *player,
                lane,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn players(count: u128) -> Vec<PlayerId> {
        (0..count)
            .map(|i| PlayerId(Uuid::from_u128(i + 1)))
            .collect()
    }

    fn lanes(seats: &[Seat]) -> Vec<u8> {
        seats.iter().map(|seat| seat.lane.get()).collect()
    }

    #[test]
    fn solo_gives_each_player_the_next_lane_in_join_order() {
        let seats = assign_seats(GameMode::Solo, &players(5));
        assert_eq!(lanes(&seats), vec![0, 1, 2, 3, 4]);
        assert_eq!(seats[2].player, players(5)[2]);
    }

    #[test]
    fn teams_pair_players_on_a_lane() {
        let seats = assign_seats(GameMode::Teams, &players(5));
        assert_eq!(lanes(&seats), vec![0, 0, 1, 1, 2]);
    }

    #[test]
    fn players_beyond_the_six_lanes_are_left_out() {
        assert_eq!(assign_seats(GameMode::Solo, &players(8)).len(), 6);
        assert_eq!(assign_seats(GameMode::Teams, &players(14)).len(), 12);
    }

    #[test]
    fn every_assignment_for_a_legal_roster_builds_a_match() {
        use crate::map::Map;
        use crate::sim::Match;
        for count in 2..=6 {
            let seats = assign_seats(GameMode::Solo, &players(count));
            assert!(Match::new(GameMode::Solo, 1, &Map::test_map(), &seats).is_ok());
        }
        for count in 4..=12 {
            let seats = assign_seats(GameMode::Teams, &players(count));
            assert!(Match::new(GameMode::Teams, 1, &Map::test_map(), &seats).is_ok());
        }
    }

    #[test]
    fn a_lineup_keeps_each_team_on_its_own_lane() {
        let all = players(4);
        let lineup = vec![vec![all[0], all[3]], vec![all[1], all[2]]];
        let seats = seat_lineup(GameMode::Teams, &lineup, &all);
        let lane_of = |player: PlayerId| {
            seats
                .iter()
                .find(|seat| seat.player == player)
                .map(|seat| seat.lane.get())
        };
        assert_eq!(lane_of(all[0]), Some(0));
        assert_eq!(lane_of(all[3]), Some(0));
        assert_eq!(lane_of(all[1]), Some(1));
        assert_eq!(lane_of(all[2]), Some(1));
    }

    #[test]
    fn a_lineup_skips_absent_players_and_seats_unlisted_ones_after() {
        let all = players(5);
        let lineup = vec![
            vec![all[0]],
            vec![all[4]],
            vec![PlayerId(Uuid::from_u128(99))],
        ];
        let roster = vec![all[0], all[4], all[2]];
        let seats = seat_lineup(GameMode::Solo, &lineup, &roster);
        assert_eq!(lanes(&seats), vec![0, 1, 2]);
        assert_eq!(seats[1].player, all[4]);
        assert_eq!(seats[2].player, all[2]);
    }

    #[test]
    fn without_a_lineup_seating_follows_join_order() {
        let all = players(5);
        assert_eq!(
            seat_lineup(GameMode::Teams, &[], &all),
            assign_seats(GameMode::Teams, &all)
        );
    }
}
