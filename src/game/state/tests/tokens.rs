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
fn last_save_before_the_boss_is_the_summon() {
    let mut gs = GameState::default();
    spawn_level(&mut gs, "05:00:00", HALL);
    feed_at(&mut gs, "05:00:10", SAVE);
    feed_at(&mut gs, "05:02:56", SAVE);
    assert_eq!(gs.tokens_shown(), Some((2, 3)));
    feed_at(
        &mut gs,
        "05:03:00",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
    );
    assert_eq!(gs.tokens_shown(), Some((1, 3)));
}

#[test]
fn alert_when_enemies_clear_with_tokens_missing() {
    let mut gs = GameState::default();
    for id in 0..20 {
        feed_at(&mut gs, "01:09:09", &format!("Retiring Enemy POOL ID{id}"));
    }
    spawn_level(&mut gs, "01:09:15", HALL);
    for id in 0..20 {
        feed_at(&mut gs, "01:09:15", &format!("Retiring Enemy POOL ID{id}"));
    }
    let pool = [
        ("01:09:24", "Initializing Enemy POOL ID0 as ENEMY ID 1"),
        ("01:09:47", "Initializing Enemy POOL ID1 as ENEMY ID 0"),
        ("01:09:51", "Retiring Enemy POOL ID0"),
        ("01:09:51", "Initializing Enemy POOL ID0 as ENEMY ID 0"),
        ("01:09:59", "Initializing Enemy POOL ID2 as ENEMY ID 2"),
        ("01:10:05", "Retiring Enemy POOL ID0"),
        ("01:10:11", "Initializing Enemy POOL ID0 as ENEMY ID 0"),
        ("01:10:13", "Retiring Enemy POOL ID1"),
        ("01:10:29", "Retiring Enemy POOL ID0"),
    ];
    for (t, line) in pool {
        feed_at(&mut gs, t, line);
    }
    assert_eq!(gs.token_alerts, 0);
    assert_eq!(gs.stage_clear(), None);
    let clear = feed_at(&mut gs, "01:10:34", "Retiring Enemy POOL ID2");
    assert_eq!(gs.stage_clear(), Some(clear));
    assert_eq!(gs.stage_secs(clear + 600), 79);
    assert_eq!(gs.token_alerts, 1);
    for t in ["01:10:35", "01:10:40", "01:10:52"] {
        feed_at(&mut gs, t, SAVE);
    }
    assert_eq!(gs.tokens_missing(), None);
    feed_at(
        &mut gs,
        "01:11:00",
        "Initializing Enemy POOL ID5 as ENEMY ID 0",
    );
    feed_at(&mut gs, "01:11:05", "Retiring Enemy POOL ID5");
    assert_eq!(gs.token_alerts, 1);
}
