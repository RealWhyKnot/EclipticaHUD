use super::*;

#[test]
fn death_clusters_count_once() {
    let mut gs = GameState::default();
    feed_at(
        &mut gs,
        "08:18:01",
        "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer",
    );
    feed_at(
        &mut gs,
        "08:19:00",
        "ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0",
    );
    for t in ["08:20:42", "08:20:42", "08:20:43", "08:20:44", "08:20:46"] {
        feed_at(&mut gs, t, "Local controller dead, switching off.");
    }
    let last = feed_at(&mut gs, "08:20:48", "Local controller dead, switching off.");
    assert_eq!(gs.runs[0].deaths, 1);
    assert_eq!(gs.runs[0].fights[0].deaths, 1);
    assert!(gs.is_dead(last));
    assert!(gs.is_dead(last + 2));
    assert!(!gs.is_dead(last + 3));
    feed_at(&mut gs, "08:25:55", "Local controller dead, switching off.");
    assert_eq!(gs.runs[0].deaths, 2);
    assert_eq!(gs.deaths_log.len(), 2);
    assert!(gs.deaths_log[0].1 < gs.deaths_log[1].1);
    assert!(gs.runs[0].fights.last().filter(|f| f.lost).is_none());
    feed_at(&mut gs, "08:25:57", "ECLIPTICA - now in lobby");
    assert!(gs.runs[0].lost);
    assert_eq!(
        gs.runs[0]
            .fights
            .last()
            .filter(|f| f.lost)
            .map(|f| f.name.as_str()),
        Some("Yuki")
    );
    assert_eq!(gs.runs[0].death_log.len(), 2);
    assert!(gs.deaths_log.is_empty());
}

#[test]
fn wipe_with_kill_triple_marks_run_lost() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "04:12:36", HALL);
    feed_at(
        &mut gs,
        "05:40:45",
        "ECLIPTICA - now fighting boss: MephielPhase2(Clone) on phase: 0.8",
    );
    for t in [
        "05:46:03", "05:46:04", "05:46:05", "05:46:06", "05:46:07", "05:46:08",
    ] {
        feed_at(&mut gs, t, DEAD);
    }
    kill_at(&mut gs, "05:46:08", "MephielPhase2", 5113);
    feed_at(&mut gs, "05:46:08", LOBBY);
    kill_at(&mut gs, "05:46:08", "MephielPhase2", 0);
    let run = &gs.runs[0];
    assert!(run.lost);
    assert_eq!(run.end_ts, run.fights[0].end_ts);
    assert_eq!(run.fights[0].kill, Some((5113, 0)));
    assert!(run.fights[0].lost);
    assert_eq!(
        run.fights
            .last()
            .filter(|f| f.lost)
            .map(|f| f.name.as_str()),
        Some("MephielPhase2")
    );
    assert_eq!(gs.mode, Mode::Lobby);
    assert_eq!(gs.last_kill.as_ref().map(|k| k.strike), Some(5113));
    feed_at(&mut gs, "05:50:00", HALL);
    assert_eq!(gs.runs.len(), 2);
    assert!(gs.last_kill.is_none());
}

#[test]
fn kill_long_after_death_is_a_win() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "04:00:00", HALL);
    feed_at(
        &mut gs,
        "04:05:00",
        "ECLIPTICA - now fighting boss: Melon(Clone) on phase: 0.9",
    );
    feed_at(&mut gs, "04:06:00", DEAD);
    kill_at(&mut gs, "04:21:15", "Melon", 8738);
    feed_at(&mut gs, "04:21:15", LOBBY);
    assert!(!gs.runs[0].lost);
    assert!(gs.runs[0].fights.last().filter(|f| f.lost).is_none());
    assert_eq!(gs.runs[0].fights[0].kill, Some((8738, 0)));
}

#[test]
fn death_after_boss_kill_loses_run_not_fight() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "04:00:00", HALL);
    feed_at(
        &mut gs,
        "04:05:00",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
    );
    kill_at(&mut gs, "04:08:00", "Nan", 1000);
    feed_at(&mut gs, "04:08:05", "ECLIPTICA - now in intermission");
    feed_at(&mut gs, "04:12:00", DEAD);
    feed_at(&mut gs, "04:12:01", LOBBY);
    assert!(gs.runs[0].lost);
    assert!(gs.runs[0].fights.last().filter(|f| f.lost).is_none());
    assert!(!gs.runs[0].fights[0].lost);
}
