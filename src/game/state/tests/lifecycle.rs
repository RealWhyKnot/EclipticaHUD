use super::*;

#[test]
fn run_lifecycle() {
    let mut gs = GameState::default();
    feed_at(
        &mut gs,
        "08:18:01",
        "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer",
    );
    assert_eq!(gs.runs.len(), 1);
    assert!(gs.runs[0].end_ts.is_none());
    feed_at(
        &mut gs,
        "08:19:54",
        "ECLIPTICA - now fighting boss: DarkMouth(Clone) on phase: 0",
    );
    feed_at(&mut gs, "08:19:55", "Dealing 100 STRIKE damage");
    feed_at(
        &mut gs,
        "08:20:10",
        "Boss DarkMouth dead, personal damage dealt: ",
    );
    feed_at(&mut gs, "08:20:10", "STRIKE DMG: 4100");
    feed_at(&mut gs, "08:20:10", "NON-STRIKE DMG: 55");
    feed_at(&mut gs, "08:20:49", "ECLIPTICA - now in lobby");
    let run = &gs.runs[0];
    assert!(run.end_ts.is_some());
    assert!(run.fights.last().filter(|f| f.lost).is_none());
    assert_eq!(run.stage, "Hall of Beginnings");
    assert_eq!(run.class, "Spellhammer");
    assert_eq!(run.fights.len(), 1);
    let f = &run.fights[0];
    assert_eq!(f.dmg, 100);
    assert_eq!(f.kill, Some((4100, 55)));
    assert!(f.end_ts.is_some());
    feed_at(
        &mut gs,
        "08:21:25",
        "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Nekomancer",
    );
    assert_eq!(gs.runs.len(), 2);
    assert_eq!(gs.runs[1].class, "Nekomancer");
}

#[test]
fn room_left_ends_run() {
    let mut gs = GameState::default();
    feed_at(
        &mut gs,
        "08:29:14",
        "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Twinmage",
    );
    feed_at(
        &mut gs,
        "08:30:00",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0",
    );
    feed_at(&mut gs, "08:30:52", "[Behaviour] OnLeftRoom");
    assert_eq!(gs.mode, Mode::Idle);
    assert!(gs.boss.is_none());
    let run = &gs.runs[0];
    assert!(run.end_ts.is_some());
    assert!(run.fights[0].end_ts.is_some());
    assert_eq!(run.fights[0].kill, None);
}

#[test]
fn rotation_ends_run() {
    let mut gs = GameState::default();
    feed_at(
        &mut gs,
        "08:29:14",
        "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Twinmage",
    );
    feed_at(
        &mut gs,
        "08:29:20",
        "damage has been taken: 5, from source: x",
    );
    let last = gs.feed(
        "2026.09.08 08:30:00 Debug      -  ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0",
    ).unwrap();
    gs.log_rotated();
    assert_eq!(gs.mode, Mode::Idle);
    assert!(gs.boss.is_none());
    assert!(gs.taken.is_empty());
    let run = &gs.runs[0];
    assert_eq!(run.end_ts, Some(last));
    assert_eq!(run.fights[0].end_ts, Some(last));
}

#[test]
fn location_follows_room_lines() {
    let mut gs = GameState::default();
    feed_at(
        &mut gs,
        "03:59:35",
        "[Behaviour] Joining wrld_0fb88df3-2057-4c2f-8e06-e948864378fd:87887~hidden(usr_4e64b21b-fbd0-4c12-8b55-c8c500b517b1)~region(use)",
    );
    assert!(gs
        .location
        .as_deref()
        .is_some_and(|l| l.ends_with("~region(use)")));
    feed_at(&mut gs, "04:40:00", "[Behaviour] OnLeftRoom");
    assert_eq!(gs.location, None);
}

#[test]
fn end_run_clears_live_state() {
    let mut gs = GameState::default();
    spawn_level(&mut gs, "04:00:01", HALL);
    feed_at(&mut gs, "04:00:20", SAVE);
    feed_at(
        &mut gs,
        "04:05:00",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
    );
    feed_at(&mut gs, "04:05:01", "ownership of Nan transferred to Alice");
    feed_at(&mut gs, "04:05:02", "Dealing 100 STRIKE damage");
    feed_at(
        &mut gs,
        "04:05:03",
        "damage has been taken: 40, from source: (Nan) slam",
    );
    feed_at(&mut gs, "04:05:04", DEAD);
    kill_at(&mut gs, "04:08:00", "Nan", 1000);
    assert!(gs.last_kill.is_some());
    feed_at(&mut gs, "04:08:30", LOBBY);
    assert_eq!(gs.mode, Mode::Lobby);
    assert!(gs.boss.is_none() && gs.target.is_none());
    assert_eq!(gs.fight_dmg, 0);
    assert_eq!(gs.fight_taken, 0);
    assert_eq!(gs.fight_hits, 0);
    assert_eq!(gs.fight_max_hit, 0);
    assert!(gs.fight_attacks.is_empty());
    assert!(gs.taken.is_empty() && gs.history.is_empty() && gs.deaths_log.is_empty());
    assert!(gs.stage.is_empty() && gs.class.is_empty());
    assert_eq!(gs.progress, 0.0);
    assert!(gs.stage_no.is_none());
    assert!(gs.level_tokens.is_empty());
    assert_eq!(gs.tokens_got, 0);
    assert!(!gs.is_dead(gs.last_ts));
    assert_eq!(gs.rolling_dps(gs.last_ts), 0);
    let run = &gs.runs[0];
    assert_eq!(run.hits.len(), 1);
    assert_eq!(run.targets.len(), 1);
    assert_eq!(run.death_log.len(), 1);
    assert_eq!(run.stage_no, Some(1));
    feed_at(&mut gs, "04:09:00", "ownership of Nan transferred to Bob");
    assert!(gs.target.is_none());
    assert!(gs.history.is_empty());
}

#[test]
fn other_world_and_quit_end_the_run() {
    let mut gs = GameState::default();
    feed_at(
        &mut gs,
        "13:24:21",
        "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
    );
    assert!(gs.in_ecliptica());
    feed_at(&mut gs, "13:25:04", HALL);
    feed_at(&mut gs, "13:26:04", "[Behaviour] OnLeftRoom");
    assert!(!gs.in_ecliptica());
    assert!(gs.runs[0].end_ts.is_some());
    assert!(!gs.runs[0].lost);
    feed_at(&mut gs, "13:26:05", "[Behaviour] Entering Room: Sky Dream");
    assert!(!gs.in_ecliptica());
    assert_eq!(gs.mode, Mode::Idle);
    feed_at(
        &mut gs,
        "13:30:00",
        "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
    );
    feed_at(&mut gs, "13:31:00", HALL);
    feed_at(&mut gs, "13:31:10", "[Behaviour] Entering Room: Sky Dream");
    assert_eq!(gs.runs.len(), 2);
    assert!(gs.runs[1].end_ts.is_some());
    assert_eq!(gs.mode, Mode::Idle);
    feed_at(
        &mut gs,
        "13:40:00",
        "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
    );
    feed_at(&mut gs, "13:41:00", HALL);
    feed_at(
        &mut gs,
        "13:42:00",
        "VRCApplication: HandleApplicationQuit at 6480.239",
    );
    assert_eq!(gs.runs.len(), 3);
    assert!(gs.runs[2].end_ts.is_some());
    assert!(gs.world.is_none());
    assert_eq!(gs.mode, Mode::Idle);
}
