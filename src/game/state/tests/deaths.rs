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
    for t in ["08:20:42", "08:20:42", "08:20:43", "08:20:44", "08:20:44"] {
        feed_at(&mut gs, t, "Local controller dead, switching off.");
    }
    let last = feed_at(&mut gs, "08:20:45", "Local controller dead, switching off.");
    assert_eq!(gs.runs[0].deaths, 1);
    assert_eq!(gs.runs[0].fights[0].deaths, 1);
    assert!(gs.is_dead(last));
    assert!(gs.is_dead(last + 1));
    assert!(!gs.is_dead(last + 2));
    feed_at(&mut gs, "08:25:55", "Local controller dead, switching off.");
    assert_eq!(gs.runs[0].deaths, 2);
    assert_eq!(gs.deaths_log.len(), 2);
    assert!(gs.deaths_log[0].1 < gs.deaths_log[1].1);
    assert!(gs.runs[0].fights.last().filter(|f| f.lost).is_none());
    feed_at(&mut gs, "08:25:57", "ECLIPTICA - now in lobby");
    assert_eq!(gs.runs[0].end, Some(RunEnd::Lost));
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
    assert_eq!(run.end, Some(RunEnd::Lost));
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
fn lobby_in_the_kill_second_is_a_loss_without_death_lines() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "16:58:12", HALL);
    feed_at(
        &mut gs,
        "17:02:18",
        "ECLIPTICA - now fighting boss: Pandora(Clone) on phase: 0.9276195",
    );
    kill_at(&mut gs, "17:06:44", "Pandora", 16582);
    feed_at(&mut gs, "17:06:44", LOBBY);
    assert_eq!(gs.runs[0].end, Some(RunEnd::Lost));
    assert!(gs.runs[0].fights[0].lost);
    assert_eq!(gs.runs[0].fights[0].kill, Some((16582, 0)));
    assert_eq!(gs.runs[0].group(0).unwrap().result(), "lost");
}

#[test]
fn lobby_from_intermission_is_not_a_loss() {
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
    assert_eq!(gs.runs[0].end, Some(RunEnd::Lobby));
    assert!(!gs.runs[0].fights[0].lost);
}
