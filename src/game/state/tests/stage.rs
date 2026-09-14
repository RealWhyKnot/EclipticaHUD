use super::*;

#[test]
fn stage_counter() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:10:01", HALL);
    assert_eq!(gs.stage_no, Some(1));
    assert_eq!(gs.runs.len(), 1);
    assert_eq!(gs.runs[0].stage_no, Some(1));
    feed_at(
        &mut gs,
        "09:12:00",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
    );
    kill_at(&mut gs, "09:15:00", "Nan", 500);
    feed_at(&mut gs, "09:15:10", "ECLIPTICA - now in intermission");
    assert_eq!(gs.stage_no, Some(1));
    const HALLOW: &str =
        "ECLIPTICA - now in stage: Stage_Hallow on phase: 0.1 as class: Spellhammer";
    feed_at(&mut gs, "09:20:00", HALLOW);
    assert_eq!(gs.stage_no, Some(2));
    assert_eq!(gs.runs[0].stage_no, Some(2));
    feed_at(&mut gs, "09:20:30", HALLOW);
    assert_eq!(gs.runs[0].stage_no, Some(2));
    feed_at(&mut gs, "09:30:00", "ECLIPTICA - now in lobby");
    assert!(gs.stage_no.is_none());
    assert_eq!(gs.runs[0].stage_no, Some(2));
    gs.log_rotated();
    assert!(gs.stage_no.is_none());
}

#[test]
fn wave_idle_after_twenty_quiet_seconds() {
    let mut gs = GameState::default();
    spawn_level(&mut gs, "14:13:32", HALL);
    let t = feed_at(
        &mut gs,
        "14:13:40",
        "Initializing Enemy POOL ID1 as ENEMY ID 8",
    );
    assert!(!gs.wave_idle(t + 19));
    assert!(gs.wave_idle(t + 20));
    feed_at(&mut gs, "14:13:45", SAVE);
    assert_eq!(gs.tokens_missing(), Some((1, 3)));
    let t = feed_at(&mut gs, "14:14:10", "Dealing 40 NON-STRIKE damage");
    assert_eq!(gs.stage_stats.dmg, 40);
    assert!(!gs.wave_idle(t + 19));
    assert!(gs.wave_idle(t + 20));
    let t = feed_at(
        &mut gs,
        "14:14:40",
        "damage has been taken: 7, from source: machinegunShooter1",
    );
    assert!(!gs.wave_idle(t + 19));
    let t = feed_at(
        &mut gs,
        "14:14:50",
        "ownership of Crab transferred to WhyKnot",
    );
    assert!(!gs.wave_idle(t + 19));
    assert!(gs.wave_idle(t + 20));
    feed_at(&mut gs, "14:15:20", SAVE);
    feed_at(&mut gs, "14:15:21", SAVE);
    assert_eq!(gs.tokens_missing(), None);
    assert_eq!(gs.tokens_shown(), Some((3, 3)));
    let t = feed_at(
        &mut gs,
        "14:16:26",
        "ECLIPTICA - now fighting boss: NX-Obsidian(Clone) on phase: 0.34",
    );
    assert!(!gs.wave_idle(t + 60));
    feed_at(&mut gs, "14:16:30", "Dealing 25 NON-STRIKE damage");
    feed_at(&mut gs, "14:16:31", "Dealing 30 STRIKE damage");
    assert_eq!(gs.fight_dmg, 55);
    assert_eq!(gs.runs[0].fights[0].dmg, 55);
    kill_at(&mut gs, "14:18:26", "NX-Obsidian", 6625);
    assert!(!gs.wave_idle(t + 180));
    feed_at(&mut gs, "14:18:35", "ECLIPTICA - now in intermission");
    assert!(!gs.wave_idle(t + 200));
    spawn_level(
        &mut gs,
        "14:20:00",
        "ECLIPTICA - now in stage: Stage_ProtoColony on phase: 0.4 as class: Spellhammer",
    );
    let t = gs.stage_stats.start_ts;
    assert_eq!(gs.tokens_missing(), Some((0, 3)));
    assert!(!gs.wave_idle(t + 19));
    assert!(gs.wave_idle(t + 20));
}

#[test]
fn empty_class_keeps_previous() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:10:01", HALL);
    feed_at(
        &mut gs,
        "09:20:00",
        "ECLIPTICA - now in stage: Stage_ProtoColony on phase: 0.1220348 as class: ",
    );
    assert_eq!(gs.class, "Spellhammer");
    assert_eq!(gs.runs[0].class, "Spellhammer");
    assert_eq!(gs.stage, "ProtoColony");
}

#[test]
fn stage_stats_before_the_boss() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:10:01", HALL);
    assert!(gs.pre_boss());
    feed_at(&mut gs, "09:10:05", "Dealing 40 STRIKE damage");
    feed_at(
        &mut gs,
        "09:10:06",
        "damage has been taken: 7, from source: machinegunShooter1",
    );
    feed_at(
        &mut gs,
        "09:10:07",
        "damage has been taken: 9, from source: machinegunShooter1",
    );
    feed_at(&mut gs, "09:10:08", DEAD);
    feed_at(
        &mut gs,
        "09:10:30",
        "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0.05 as class: Spellhammer",
    );
    feed_at(&mut gs, "09:10:31", "Dealing 60 STRIKE damage");
    let s = &gs.stage_stats;
    assert_eq!(
        (s.dmg, s.taken, s.hits, s.max_hit, s.deaths),
        (100, 16, 2, 9, 1)
    );
    assert_eq!(s.attacks.len(), 1);
    assert_eq!(gs.fight_dmg, 0);
    let t = feed_at(&mut gs, "09:10:41", "Dealing 0 STRIKE damage");
    assert_eq!(gs.stage_dps(t), 100 / 40);
    feed_at(
        &mut gs,
        "09:11:01",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0.05",
    );
    assert!(!gs.pre_boss());
    assert_eq!(gs.stage_stats.start_ts, 0);
    let run = gs.live_run().unwrap();
    assert_eq!(run.stages.len(), 1);
    assert_eq!(run.stages[0].end_ts, Some(t + 20));
    assert_eq!(run.stages[0].dmg, 100);
    feed_at(&mut gs, "09:11:05", "Dealing 500 STRIKE damage");
    feed_at(
        &mut gs,
        "09:11:06",
        "damage has been taken: 30, from source: (Nan) slam",
    );
    assert_eq!(gs.fight_dmg, 500);
    kill_at(&mut gs, "09:12:00", "Nan", 500);
    feed_at(&mut gs, "09:12:05", "ECLIPTICA - now in intermission");
    let run = gs.live_run().unwrap();
    assert_eq!(run.dmg(), 600);
    assert_eq!(run.taken(), 46);
    assert_eq!(run.hit_count(), 3);
    assert_eq!(run.max_hit(), 30);
    assert_eq!(run.kills(), 1);
    assert_eq!(run.attacks().len(), 2);
    assert_eq!(run.active_secs(0), 60 + 59);
    feed_at(
        &mut gs,
        "09:13:00",
        "ECLIPTICA - now in stage: Stage_GMFuncFlat on phase: 0.1 as class: Spellhammer",
    );
    assert_eq!(gs.stage, "GMFuncFlat");
    assert_eq!(gs.stage_stats.dmg, 0);
    feed_at(&mut gs, "09:13:10", "Dealing 5 STRIKE damage");
    feed_at(&mut gs, "09:14:00", LOBBY);
    assert_eq!(gs.runs[0].stages.len(), 2);
    assert_eq!(gs.runs[0].stages[1].dmg, 5);
    assert_eq!(gs.stage_stats.start_ts, 0);
}
