use super::*;

#[test]
fn dps_windows() {
    let mut gs = GameState::default();
    let t0 = split_line(&format!("{P}Dealing 100 STRIKE damage"))
        .unwrap()
        .ts;
    gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1"
    ));
    gs.feed(&format!("{P}Dealing 100 STRIKE damage"));
    gs.feed(&format!("{P}Dealing 50 STRIKE damage"));
    assert_eq!(gs.fight_dmg, 150);
    assert_eq!(gs.rolling_dps(t0), 150);
    assert_eq!(gs.rolling_dps(t0 + 2), 50);
    assert_eq!(gs.rolling_dps(t0 + 9), 15);
    assert_eq!(gs.rolling_dps(t0 + 10), 0);
    assert_eq!(gs.rolling_dps(t0 + 60), 0);
    let mut short = GameState {
        window: 3,
        ..Default::default()
    };
    short.feed(&format!("{P}Dealing 90 STRIKE damage"));
    assert_eq!(short.rolling_dps(t0 + 2), 30);
    assert_eq!(short.rolling_dps(t0 + 3), 0);
    assert_eq!(gs.fight_dps(t0 + 10), 15);
    gs.feed(&format!("{P}Boss Kakarot dead, personal damage dealt: "));
    assert!(gs.boss.is_none());
    assert_eq!(gs.fight_dps(t0 + 10), 0);
}

#[test]
fn taken_aggregates_current_fight() {
    let mut gs = GameState::default();
    let t0 = feed_at(
        &mut gs,
        "12:00:00",
        "damage has been taken: 5, from source: attack_Spit",
    );
    assert_eq!(gs.rolling_taken(t0), 5);
    assert_eq!(gs.fight_taken, 0);
    assert_eq!(gs.fight_hits, 0);
    feed_at(
        &mut gs,
        "12:00:10",
        "ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0",
    );
    feed_at(
        &mut gs,
        "12:00:11",
        "damage has been taken: 12, from source: (Yuki) frostBeam",
    );
    feed_at(
        &mut gs,
        "12:00:12",
        "damage has been taken: 8, from source: (Yuki) frostBeam",
    );
    feed_at(
        &mut gs,
        "12:00:13",
        "damage has been taken: 30, from source: attack_Spit (1)",
    );
    let t = feed_at(
        &mut gs,
        "12:00:14",
        "damage has been taken: 10, from source: attack_Spit (2)",
    );
    assert_eq!(gs.rolling_taken(t), 15);
    assert_eq!(gs.fight_taken, 60);
    assert_eq!(gs.fight_hits, 4);
    assert_eq!(gs.fight_max_hit, 30);
    assert_eq!(gs.fight_taken_rate(t + 6), 6);
    let tl = |who: &str, attack: &str, total: u64, hits: u32| Tally {
        who: who.into(),
        attack: attack.into(),
        total,
        hits,
    };
    let expected = vec![tl("Yuki", "Frost Beam", 20, 2), tl("enemy", "Spit", 40, 2)];
    assert_eq!(gs.fight_attacks, expected);
    assert_eq!(gs.runs[0].fights[0].taken, 60);
    assert_eq!(gs.runs[0].fights[0].hits, 4);
    assert_eq!(gs.runs[0].fights[0].attacks, expected);
    feed_at(
        &mut gs,
        "12:01:00",
        "ECLIPTICA - now fighting boss: YukiPhase2(Clone) on phase: 0",
    );
    feed_at(
        &mut gs,
        "12:01:01",
        "damage has been taken: 1, from source: (Yuki) frostBeam",
    );
    assert_eq!(gs.fight_taken, 61);
    assert_eq!(gs.fight_hits, 5);
    assert_eq!(gs.runs[0].fights[1].taken, 1);
    assert_eq!(
        gs.runs[0].fights[1].attacks,
        vec![tl("Yuki", "Frost Beam", 1, 1)]
    );
    let merged = merge_tallies(gs.runs[0].fights.iter().map(|f| f.attacks.as_slice()));
    assert_eq!(
        merged,
        vec![tl("Yuki", "Frost Beam", 21, 3), tl("enemy", "Spit", 40, 2)]
    );
    feed_at(
        &mut gs,
        "12:05:00",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0",
    );
    assert_eq!(gs.fight_taken, 0);
    assert_eq!(gs.fight_hits, 0);
    assert_eq!(gs.fight_max_hit, 0);
    assert!(gs.fight_attacks.is_empty());
    assert_eq!(gs.fight_taken_rate(t + 6), 0);
    assert_eq!(gs.taken.len(), 6);
    let seqs: Vec<u64> = gs.taken.iter().map(|e| e.seq).collect();
    assert!(seqs.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn taken_and_history_cap() {
    let mut gs = GameState::default();
    feed_at(
        &mut gs,
        "12:00:00",
        "ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0",
    );
    for _ in 0..600 {
        feed_at(
            &mut gs,
            "12:00:01",
            "damage has been taken: 1, from source: ",
        );
        feed_at(
            &mut gs,
            "12:00:01",
            "ownership of Yuki transferred to Alice",
        );
    }
    assert_eq!(gs.taken.len(), 500);
    assert_eq!(gs.history.len(), 500);
    assert_eq!(gs.fight_hits, 600);
    gs.log_rotated();
    assert!(gs.taken.is_empty());
    assert!(gs.fight_attacks.is_empty());
}
