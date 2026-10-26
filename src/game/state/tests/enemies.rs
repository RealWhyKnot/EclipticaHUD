use super::*;

fn stage_with_pool(gs: &mut GameState) {
    feed_at(gs, "13:39:50", RESET);
    feed_at(gs, "13:40:02", HALL);
    for id in 0..20 {
        feed_at(gs, "13:40:02", &format!("Retiring Enemy POOL ID{id}"));
    }
}

fn spawn(gs: &mut GameState, t: &str, slot: u32, kind: u32, name: &str) {
    feed_at(
        gs,
        t,
        &format!("Initializing Enemy POOL ID{slot} as ENEMY ID {kind}"),
    );
    feed_at(
        gs,
        t,
        &format!("[Behaviour] No targets to encode on {name}(Clone)"),
    );
}

#[test]
fn count_follows_spawns_and_retires_to_zero() {
    let mut gs = GameState::default();
    stage_with_pool(&mut gs);
    assert_eq!(gs.enemies_left(), None);
    spawn(&mut gs, "13:40:12", 0, 3, "FrostSac");
    assert_eq!(gs.enemies_left(), Some(1));
    spawn(&mut gs, "13:40:15", 1, 1, "Fly");
    spawn(&mut gs, "13:40:18", 2, 1, "Fly");
    assert_eq!(gs.enemies_left(), Some(3));
    feed_at(&mut gs, "13:40:30", "Retiring Enemy POOL ID1");
    feed_at(&mut gs, "13:40:31", "Retiring Enemy POOL ID1");
    assert_eq!(gs.enemies_left(), Some(2));
    feed_at(&mut gs, "13:40:40", "Retiring Enemy POOL ID0");
    let clear = feed_at(&mut gs, "13:40:50", "Retiring Enemy POOL ID2");
    assert_eq!(gs.enemies_left(), Some(0));
    assert_eq!(gs.stage_clear(), Some(clear));
}

#[test]
fn hidden_outside_the_clearing_phase() {
    let mut gs = GameState::default();
    assert_eq!(gs.enemies_left(), None);
    stage_with_pool(&mut gs);
    spawn(&mut gs, "13:40:12", 0, 3, "FrostSac");
    feed_at(&mut gs, "13:40:40", "Retiring Enemy POOL ID0");
    feed_at(
        &mut gs,
        "13:41:00",
        "Initializing Enemy POOL ID0 as ENEMY ID 36",
    );
    feed_at(
        &mut gs,
        "13:41:00",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0",
    );
    assert_eq!(gs.enemies_left(), None);
    feed_at(&mut gs, "13:43:00", "ECLIPTICA - now in intermission");
    assert_eq!(gs.enemies_left(), None);
    feed_at(&mut gs, "13:44:00", LOBBY);
    assert_eq!(gs.enemies_left(), None);
}

#[test]
fn hidden_when_the_pool_was_not_seen_resetting() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "13:40:02", HALL);
    spawn(&mut gs, "13:40:12", 0, 3, "FrostSac");
    assert_eq!(gs.enemies_left(), None);
}

#[test]
fn names_match_spawns_in_the_order_they_came() {
    let mut gs = GameState::default();
    stage_with_pool(&mut gs);
    feed_at(
        &mut gs,
        "13:40:12",
        "Initializing Enemy POOL ID17 as ENEMY ID 90",
    );
    feed_at(
        &mut gs,
        "13:40:12",
        "Initializing Enemy POOL ID2 as ENEMY ID 4",
    );
    feed_at(
        &mut gs,
        "13:40:12",
        "[Behaviour] No targets to encode on Neko4(Clone)",
    );
    feed_at(
        &mut gs,
        "13:40:12",
        "[Behaviour] No targets to encode on Ice Squid(Clone)",
    );
    assert_eq!(gs.enemies_left(), Some(1));
    feed_at(
        &mut gs,
        "13:40:20",
        "Initializing Enemy POOL ID5 as ENEMY ID 90",
    );
    assert_eq!(gs.enemies_left(), Some(1));
    spawn(&mut gs, "13:40:25", 6, 4, "Ice Squid");
    assert_eq!(gs.enemies_left(), Some(2));
}

#[test]
fn a_known_summon_keeps_its_place_in_the_name_queue() {
    let mut gs = GameState::default();
    stage_with_pool(&mut gs);
    spawn(&mut gs, "13:40:10", 16, 87, "Neko1");
    feed_at(
        &mut gs,
        "13:40:14",
        "Initializing Enemy POOL ID15 as ENEMY ID 87",
    );
    feed_at(
        &mut gs,
        "13:40:14",
        "Initializing Enemy POOL ID0 as ENEMY ID 26",
    );
    feed_at(
        &mut gs,
        "13:40:14",
        "[Behaviour] No targets to encode on Neko1(Clone)",
    );
    feed_at(
        &mut gs,
        "13:40:14",
        "[Behaviour] No targets to encode on Crab(Clone)",
    );
    assert_eq!(gs.enemies_left(), Some(1));
    spawn(&mut gs, "13:40:20", 1, 26, "Crab");
    assert_eq!(gs.enemies_left(), Some(2));
}

#[test]
fn a_late_name_still_finds_its_spawn() {
    let mut gs = GameState::default();
    stage_with_pool(&mut gs);
    spawn(&mut gs, "13:40:10", 0, 2, "Roboorb");
    feed_at(
        &mut gs,
        "13:40:20",
        "Initializing Enemy POOL ID1 as ENEMY ID 2",
    );
    feed_at(&mut gs, "13:40:20", "Retiring Enemy POOL ID0");
    feed_at(
        &mut gs,
        "13:40:21",
        "Initializing Enemy POOL ID3 as ENEMY ID 88",
    );
    feed_at(
        &mut gs,
        "13:40:21",
        "[Behaviour] No targets to encode on Roboorb(Clone)",
    );
    feed_at(
        &mut gs,
        "13:40:21",
        "[Behaviour] No targets to encode on Neko2(Clone)",
    );
    assert_eq!(gs.enemies_left(), Some(1));
}

#[test]
fn a_spawn_with_no_name_counts_and_does_not_take_a_later_name() {
    let mut gs = GameState::default();
    stage_with_pool(&mut gs);
    feed_at(
        &mut gs,
        "13:40:10",
        "Initializing Enemy POOL ID0 as ENEMY ID 2",
    );
    feed_at(&mut gs, "13:40:15", "unrelated line");
    spawn(&mut gs, "13:40:20", 1, 87, "Neko1");
    assert_eq!(gs.enemies_left(), Some(1));
    feed_at(&mut gs, "13:40:30", "Retiring Enemy POOL ID0");
    assert_eq!(gs.enemies_left(), Some(0));
}

#[test]
fn a_summon_kind_from_one_stage_is_still_a_summon_in_the_next() {
    let mut gs = GameState::default();
    stage_with_pool(&mut gs);
    spawn(&mut gs, "13:40:10", 4, 87, "Neko1");
    assert_eq!(gs.enemies_left(), None);
    spawn(&mut gs, "13:40:12", 5, 3, "FrostSac");
    assert_eq!(gs.enemies_left(), Some(1));
    feed_at(
        &mut gs,
        "13:41:00",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0",
    );
    feed_at(
        &mut gs,
        "13:43:00",
        "Boss Kakarot dead, personal damage dealt: ",
    );
    feed_at(&mut gs, "13:43:05", "ECLIPTICA - now in intermission");
    for id in 0..20 {
        feed_at(&mut gs, "13:43:10", &format!("Retiring Enemy POOL ID{id}"));
    }
    feed_at(
        &mut gs,
        "13:44:00",
        "ECLIPTICA - now in stage: Stage_OuterHeven on phase: 0.2 as class: Spellhammer",
    );
    feed_at(
        &mut gs,
        "13:44:10",
        "Initializing Enemy POOL ID2 as ENEMY ID 87",
    );
    feed_at(&mut gs, "13:44:20", "unrelated line");
    assert_eq!(gs.enemies_left(), None);
    spawn(&mut gs, "13:44:25", 3, 5, "Roboorb");
    assert_eq!(gs.enemies_left(), Some(1));
}

#[test]
fn a_huge_wave_counts_every_slot() {
    let mut gs = GameState::default();
    stage_with_pool(&mut gs);
    for slot in 0..1200 {
        feed_at(
            &mut gs,
            "13:40:12",
            &format!("Initializing Enemy POOL ID{slot} as ENEMY ID 1"),
        );
    }
    assert_eq!(gs.enemies_left(), Some(1200));
}

#[test]
fn hidden_until_the_first_enemy_of_the_stage() {
    let mut gs = GameState::default();
    stage_with_pool(&mut gs);
    feed_at(&mut gs, "13:40:09", "unrelated line");
    assert_eq!(gs.enemies_left(), None);
    spawn(&mut gs, "13:40:10", 0, 3, "FrostSac");
    feed_at(&mut gs, "13:40:20", "Retiring Enemy POOL ID0");
    assert_eq!(gs.enemies_left(), Some(0));
}

#[test]
fn a_wave_inside_the_wait_still_counts() {
    let mut gs = GameState::default();
    stage_with_pool(&mut gs);
    spawn(&mut gs, "13:40:10", 0, 3, "FrostSac");
    let clear = feed_at(&mut gs, "13:40:20", "Retiring Enemy POOL ID0");
    assert!(!gs.clear_settled(clear + TOKEN_ALERT_SECS));
    assert!(gs.clear_settled(clear + TOKEN_ALERT_SECS + 1));
    spawn(&mut gs, "13:40:28", 1, 1, "Fly");
    assert_eq!(gs.enemies_left(), Some(1));
    assert_eq!(gs.stage_clear(), None);
}

#[test]
fn a_late_spawn_after_a_settled_clear_still_counts() {
    let mut gs = GameState::default();
    stage_with_pool(&mut gs);
    spawn(&mut gs, "13:40:10", 0, 3, "FrostSac");
    let clear = feed_at(&mut gs, "13:40:20", "Retiring Enemy POOL ID0");
    assert!(gs.clear_settled(clear + 29));
    feed_at(
        &mut gs,
        "13:40:49",
        "Initializing Enemy POOL ID0 as ENEMY ID 36",
    );
    assert_eq!(gs.enemies_left(), Some(1));
    assert_eq!(gs.stage_clear(), None);
    assert!(!gs.clear_settled(clear + 29));
}
