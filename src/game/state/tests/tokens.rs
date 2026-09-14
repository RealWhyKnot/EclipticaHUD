use super::*;

#[test]
fn token_spawns_per_level() {
    let mut gs = GameState::default();
    for _ in 0..3 {
        feed_at(&mut gs, "08:18:01", "spawn token, False, 0");
    }
    feed_at(
        &mut gs,
        "08:18:01",
        "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer",
    );
    assert_eq!(gs.level_tokens, vec![(false, 0); 3]);
    feed_at(
        &mut gs,
        "08:19:00",
        "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer",
    );
    assert_eq!(gs.level_tokens.len(), 3);
    feed_at(&mut gs, "08:46:50", "spawn token, True, 35");
    feed_at(&mut gs, "08:46:50", "spawn token, True, 55");
    feed_at(
        &mut gs,
        "08:46:50",
        "ECLIPTICA - now in stage: Stage_GMFuncFlat on phase: 0.06 as class: Spellhammer",
    );
    assert_eq!(gs.level_tokens, vec![(true, 35), (true, 55)]);
    feed_at(&mut gs, "08:50:00", "ECLIPTICA - now in lobby");
    assert!(gs.level_tokens.is_empty());
}

#[test]
fn tokens_count_session_saves_until_boss() {
    let mut gs = GameState::default();
    assert_eq!(gs.tokens_shown(), None);
    spawn_level(&mut gs, "04:02:02", HALL);
    assert_eq!(gs.tokens_shown(), Some((0, 3)));
    feed_at(&mut gs, "04:02:26", SAVE);
    assert_eq!(gs.tokens_shown(), Some((1, 3)));
    feed_at(&mut gs, "04:02:30", SAVE);
    feed_at(&mut gs, "04:03:24", SAVE);
    assert_eq!(gs.tokens_shown(), Some((3, 3)));
    feed_at(&mut gs, "04:03:53", SAVE);
    assert_eq!(gs.tokens_shown(), Some((3, 3)));
    feed_at(
        &mut gs,
        "04:03:57",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0",
    );
    assert_eq!(gs.tokens_got, 3);
    feed_at(
        &mut gs,
        "04:06:56",
        "Boss Kakarot dead, personal damage dealt: ",
    );
    feed_at(&mut gs, "04:06:58", SAVE);
    assert_eq!(gs.tokens_shown(), Some((3, 3)));
    feed_at(&mut gs, "04:07:04", "ECLIPTICA - now in intermission");
    feed_at(&mut gs, "04:07:27", SAVE);
    assert_eq!(gs.tokens_got, 3);
    assert_eq!(gs.tokens_shown(), None);
    spawn_level(
        &mut gs,
        "04:07:56",
        "ECLIPTICA - now in stage: Stage_GMFuncFlat on phase: 0.06 as class: Spellhammer",
    );
    assert_eq!(gs.tokens_shown(), Some((0, 3)));
}

#[test]
fn skipped_token_and_boss_save() {
    let mut gs = GameState::default();
    spawn_level(&mut gs, "04:28:53", HALL);
    feed_at(&mut gs, "04:29:16", SAVE);
    feed_at(&mut gs, "04:29:19", SAVE);
    feed_at(&mut gs, "04:31:38", SAVE);
    assert_eq!(gs.tokens_shown(), Some((3, 3)));
    feed_at(
        &mut gs,
        "04:31:42",
        "ECLIPTICA - now fighting boss: FlyLord(Clone) on phase: 0.12",
    );
    assert_eq!(gs.tokens_shown(), Some((2, 3)));
    feed_at(
        &mut gs,
        "04:34:35",
        "ECLIPTICA - now fighting boss: FlyLordPhase2(Clone) on phase: 0.12",
    );
    assert_eq!(gs.tokens_shown(), Some((2, 3)));
}

#[test]
fn token_left_long_before_boss_is_kept() {
    let mut gs = GameState::default();
    spawn_level(&mut gs, "05:00:00", HALL);
    feed_at(&mut gs, "05:00:10", SAVE);
    feed_at(
        &mut gs,
        "05:03:00",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
    );
    assert_eq!(gs.tokens_shown(), Some((1, 3)));
}
