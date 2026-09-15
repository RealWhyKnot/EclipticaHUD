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
fn enemy_pool_clear_stops_the_stage_clock() {
    let mut gs = GameState::default();
    spawn_level(&mut gs, "14:13:32", HALL);
    let start = gs.stage_stats.start_ts;
    feed_at(&mut gs, "14:13:32", "Retiring Enemy POOL ID4");
    assert_eq!(gs.stage_clear(), None);
    feed_at(
        &mut gs,
        "14:13:40",
        "Initializing Enemy POOL ID1 as ENEMY ID 8",
    );
    feed_at(
        &mut gs,
        "14:13:50",
        "Initializing Enemy POOL ID2 as ENEMY ID 8",
    );
    feed_at(&mut gs, "14:14:10", "Dealing 580 NON-STRIKE damage");
    feed_at(&mut gs, "14:14:20", "Retiring Enemy POOL ID1");
    assert_eq!(gs.stage_clear(), None);
    assert_eq!(gs.stage_secs(start + 50), 50);
    let clear = feed_at(&mut gs, "14:14:30", "Retiring Enemy POOL ID2");
    assert_eq!(gs.stage_clear(), Some(clear));
    assert_eq!(gs.stage_secs(clear + 100), 58);
    assert_eq!(gs.stage_dps(clear + 100), 10);
    feed_at(&mut gs, "14:14:40", "Retiring Enemy POOL ID2");
    assert_eq!(gs.stage_clear(), Some(clear));
    feed_at(
        &mut gs,
        "14:14:45",
        "Initializing Enemy POOL ID0 as ENEMY ID 3",
    );
    assert_eq!(gs.stage_clear(), None);
    assert_eq!(gs.stage_secs(start + 80), 80);
    let clear = feed_at(&mut gs, "14:14:52", "Retiring Enemy POOL ID0");
    assert_eq!(gs.stage_secs(clear + 30), 80);
    feed_at(
        &mut gs,
        "14:16:25",
        "Initializing Enemy POOL ID0 as ENEMY ID 47",
    );
    assert_eq!(gs.stage_clear(), None);
    feed_at(
        &mut gs,
        "14:16:26",
        "ECLIPTICA - now fighting boss: NX-Obsidian(Clone) on phase: 0.34",
    );
    assert_eq!(gs.stage_clear(), None);
    assert_eq!(gs.runs[0].stages[0].end_ts, Some(clear));
    feed_at(&mut gs, "14:16:30", "Retiring Enemy POOL ID0");
    assert_eq!(gs.stage_clear(), None);
    assert_eq!(gs.token_alerts, 2);
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
    feed_at(
        &mut gs,
        "09:11:07",
        "damage has been taken: 10, from source: (Nan) slam",
    );
    assert_eq!(gs.fight_dmg, 500);
    kill_at(&mut gs, "09:12:00", "Nan", 500);
    feed_at(&mut gs, "09:12:05", "ECLIPTICA - now in intermission");
    let run = gs.live_run().unwrap();
    assert_eq!(run.dmg(), 600);
    assert_eq!(run.taken(), 56);
    assert_eq!(run.hit_count(), 4);
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
