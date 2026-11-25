use crate::domain::{GameMode, TICKS_PER_SECOND};

pub const COUNTDOWN_TITLE_SECONDS: u32 = 5;

pub const WELCOME_TITLE: &str = "§6TowerWars";
pub const WELCOME_SUBTITLE: &str = "§7Défends ta voie, fais tomber les autres";
pub const START_TITLE: &str = "§aDéfends ta voie !";

pub fn welcome_lines(mode: GameMode) -> Vec<String> {
    let team = match mode {
        GameMode::Solo => "§7Mode §fSolo §7: chacun sa voie.",
        GameMode::Teams => "§7Mode §fDuo §7: ton coéquipier partage ta voie et tes PV.",
    };
    vec![
        "§6§lTowerWars §7- comment jouer".to_owned(),
        team.to_owned(),
        "§e1. §fPose des tours à côté du chemin avec le §bbâton de construction§f.".to_owned(),
        "§e2. §fEnvoie des mobs aux autres voies : chaque envoi augmente ton §aincome§f."
            .to_owned(),
        "§e3. §fUn mob qui traverse ta voie te coûte des PV. Le dernier debout gagne.".to_owned(),
    ]
}

pub fn countdown_beat(remaining_ticks: u32) -> Option<u32> {
    if remaining_ticks == 0 || !remaining_ticks.is_multiple_of(TICKS_PER_SECOND) {
        return None;
    }
    let seconds = remaining_ticks / TICKS_PER_SECOND;
    (seconds <= COUNTDOWN_TITLE_SECONDS).then_some(seconds)
}

pub fn countdown_title(seconds: u32) -> String {
    let color = match seconds {
        1 => "§c",
        2 | 3 => "§e",
        _ => "§a",
    };
    format!("{color}{seconds}")
}

pub fn start_subtitle(preparation_secs: u32) -> String {
    if preparation_secs == 0 {
        return "§7Construis et envoie des mobs".to_owned();
    }
    format!("§7Prépare-toi : envois ouverts dans §f{preparation_secs}s")
}

pub fn start_lines(teammate: Option<&str>, preparation_secs: u32) -> Vec<String> {
    let mut lines = vec![if preparation_secs == 0 {
        "§aLa partie commence !".to_owned()
    } else {
        format!(
            "§aLa partie commence ! §7Tu as §f{preparation_secs}s §7pour poser tes premières tours avant les premiers envois."
        )
    }];
    if let Some(name) = teammate {
        lines.push(format!("§7Ton coéquipier : §f{name}§7."));
    }
    lines.push(
        "§7Astuce : mélange les tours. Les mobs rapides (§fLapin§7, §fRenard§7, §fCheval squelette§7) passent là où une défense lente tient."
            .to_owned(),
    );
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_countdown_shows_a_title_on_each_of_the_last_five_seconds() {
        let beats: Vec<u32> = (0..=15 * TICKS_PER_SECOND)
            .filter_map(countdown_beat)
            .collect();
        assert_eq!(beats, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn the_last_second_is_red() {
        assert_eq!(countdown_title(1), "§c1");
        assert_eq!(countdown_title(5), "§a5");
    }

    #[test]
    fn the_welcome_explains_the_mode_and_the_three_actions() {
        let solo = welcome_lines(GameMode::Solo);
        let duo = welcome_lines(GameMode::Teams);
        assert_eq!(solo.len(), 5);
        assert!(solo[1].contains("Solo"));
        assert!(duo[1].contains("Duo"));
    }

    #[test]
    fn a_teammate_is_named_at_the_start() {
        assert!(
            start_lines(Some("Alice"), 20)
                .iter()
                .any(|line| line.contains("Alice"))
        );
        assert!(
            !start_lines(None, 20)
                .iter()
                .any(|line| line.contains("coéquipier"))
        );
    }
}
