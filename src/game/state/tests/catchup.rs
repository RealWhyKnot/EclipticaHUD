use super::*;
use crate::game::state::{Mark, Summary};

const HALLOW: &str = "ECLIPTICA - now in stage: Stage_Hallow on phase: 0.1 as class: Spellhammer";

fn peer() -> GameState {
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:10:00", HALL);
    feed_at(
        &mut gs,
        "09:12:00",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
    );
    kill_at(&mut gs, "09:15:00", "Nan", 500);
    feed_at(&mut gs, "09:15:10", "ECLIPTICA - now in intermission");
    feed_at(&mut gs, "09:20:00", HALLOW);
    feed_at(
        &mut gs,
        "09:25:00",
        "ECLIPTICA - now fighting boss: HaKli(Clone) on phase: 0",
    );
    gs
}

fn late() -> GameState {
    let mut gs = GameState::default();
    feed_at(
        &mut gs,
        "09:25:03",
        "ECLIPTICA - now fighting boss: HaKli(Clone) on phase: 0",
    );
    gs
}

#[test]
fn late_joiner_gets_the_earlier_stages_and_bosses() {
    let peer = peer();
    let mut gs = late();
    let own_start = gs.runs[0].start_ts;
    let own_fight = gs.runs[0].fights[0].start_ts;
    let s = peer.summary().unwrap();
    assert_eq!(gs.catch_up(&s), Some(true));
    let run = &gs.runs[0];
    assert_eq!(run.start_ts, peer.runs[0].start_ts);
    assert!(run.start_ts < own_start);
    let names: Vec<&str> = run.fights.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Nan", "HaKli"]);
    assert!(run.fights[0].missed);
    assert!(run.fights[0].kill.is_some());
    assert_eq!(run.fights[0].dmg, 0);
    assert_eq!(run.fights[1].start_ts, own_fight);
    assert!(!run.fights[1].missed);
    assert!(run.stages.iter().all(|s| s.missed));
    assert_eq!(run.stages.len(), 2);
    assert_eq!(gs.stage_no, Some(2));
    assert_eq!(run.stage_no, Some(2));
    assert_eq!(gs.catch_up(&peer.summary().unwrap()), Some(false));
    assert_eq!(gs.runs[0].fights.len(), 2);
}

#[test]
fn a_full_log_ignores_peer_history() {
    let mut gs = peer();
    let s = gs.summary().unwrap();
    assert_eq!(gs.catch_up(&s), Some(false));
    assert_eq!(gs.summary().unwrap(), s);
    assert!(gs.runs[0].fights.iter().all(|f| !f.missed));
}

#[test]
fn unmatched_history_waits() {
    let mut gs = late();
    let other = Summary {
        start: 1,
        stages: None,
        marks: vec![Mark {
            boss: true,
            name: "Nan".into(),
            start: 2,
            end: Some(3),
            killed: true,
            lost: false,
        }],
    };
    assert_eq!(gs.catch_up(&other), None);
    assert_eq!(gs.runs[0].fights.len(), 1);
}

#[test]
fn bad_summaries_change_nothing() {
    let mut gs = late();
    let before = gs.summary().unwrap();
    assert_eq!(gs.catch_up(&Summary::default()), Some(false));
    let mut s = peer().summary().unwrap();
    s.marks.reverse();
    assert_eq!(gs.catch_up(&s), Some(false));
    assert_eq!(gs.summary().unwrap(), before);
}

#[test]
fn joining_in_intermission_opens_the_run() {
    let mut peer = peer();
    kill_at(&mut peer, "09:28:00", "HaKli", 900);
    feed_at(&mut peer, "09:28:10", "ECLIPTICA - now in intermission");
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:29:00", "ECLIPTICA - now in intermission");
    assert!(gs.live_run().is_none());
    let s = peer.summary().unwrap();
    assert_eq!(gs.catch_up(&s), Some(true));
    let run = gs.live_run().unwrap();
    assert_eq!(run.start_ts, peer.runs[0].start_ts);
    assert_eq!(run.fights.len(), 2);
    assert_eq!(run.stages.len(), 2);
    assert_eq!(gs.stage_no, Some(2));
    feed_at(
        &mut gs,
        "09:35:00",
        "ECLIPTICA - now in stage: Stage_Hallow on phase: 0.2 as class: Spellhammer",
    );
    assert_eq!(gs.stage_no, Some(3));
}

#[test]
fn summary_keeps_the_newest_marks() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:00:00", HALL);
    for i in 0..30u32 {
        let t = format!("09:{:02}:{:02}", 1 + i / 30, (i * 2) % 60);
        feed_at(
            &mut gs,
            &t,
            &format!("ECLIPTICA - now fighting boss: Boss{i}(Clone) on phase: 0"),
        );
        kill_at(&mut gs, &t, &format!("Boss{i}"), 10);
    }
    let s = gs.summary().unwrap();
    assert_eq!(s.marks.len(), crate::game::state::MAX_MARKS);
    assert_eq!(s.marks.last().unwrap().name, "Boss29");
    assert!(s.marks.windows(2).all(|w| w[0].start <= w[1].start));
}

#[test]
fn borrowed_history_goes_when_the_run_ends() {
    let mut gs = late();
    let own_start = gs.runs[0].start_ts;
    assert_eq!(gs.catch_up(&peer().summary().unwrap()), Some(true));
    feed_at(&mut gs, "09:27:00", LOBBY);
    let run = &gs.runs[0];
    assert!(run.end_ts.is_some());
    assert_eq!(run.start_ts, own_start);
    let names: Vec<&str> = run.fights.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["HaKli"]);
    assert!(run.stages.is_empty());
    assert!(run.borrowed.is_none());
}

#[test]
fn turning_sharing_off_drops_borrowed_history() {
    let mut gs = late();
    let own = gs.summary().unwrap();
    let stage_no = gs.stage_no;
    assert_eq!(gs.catch_up(&peer().summary().unwrap()), Some(true));
    gs.drop_borrowed();
    assert_eq!(gs.summary().unwrap(), own);
    assert_eq!(gs.stage_no, stage_no);
    assert_eq!(gs.runs[0].stage_no, stage_no);
    assert!(gs.runs[0].fights.iter().all(|f| !f.missed));
    assert_eq!(gs.catch_up(&peer().summary().unwrap()), Some(true));
    assert_eq!(gs.runs[0].fights.len(), 2);
}

#[test]
fn a_borrowed_intermission_run_goes_entirely() {
    let mut peer = peer();
    kill_at(&mut peer, "09:28:00", "HaKli", 900);
    feed_at(&mut peer, "09:28:10", "ECLIPTICA - now in intermission");
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:29:00", "ECLIPTICA - now in intermission");
    assert_eq!(gs.catch_up(&peer.summary().unwrap()), Some(true));
    gs.drop_borrowed();
    assert!(gs.runs.is_empty());
    assert_eq!(gs.stage_no, None);
    assert_eq!(gs.catch_up(&peer.summary().unwrap()), Some(true));
    feed_at(&mut gs, "09:35:00", HALLOW);
    let started = gs.stage_stats.start_ts;
    feed_at(&mut gs, "09:40:00", LOBBY);
    assert_eq!(gs.runs.len(), 1);
    assert_eq!(gs.runs[0].start_ts, started);
    assert_eq!(gs.runs[0].stages.len(), 1);
    assert!(gs.runs[0].fights.is_empty());
    assert_eq!(gs.runs[0].stage_no, None);
}

const BIGCITY: &str =
    "ECLIPTICA - now in stage: Stage_GMBigcity on phase: 0.6166094 as class: Spellhammer";

#[test]
fn a_late_joiner_alone_shows_no_stage_number() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:30:00", BIGCITY);
    assert_eq!(gs.stage_no, None);
    assert_eq!(gs.summary().unwrap().stages, None);
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:00:00", HALL);
    assert_eq!(gs.stage_no, Some(1));
    feed_at(&mut gs, "09:05:00", "ECLIPTICA - now in intermission");
    feed_at(&mut gs, "09:06:00", BIGCITY);
    assert_eq!(gs.stage_no, Some(2));
    assert_eq!(gs.summary().unwrap().stages, Some(2));
}

#[test]
fn the_stage_count_survives_a_cut_summary() {
    let mut s = peer().summary().unwrap();
    s.stages = Some(7);
    let mut gs = late();
    assert_eq!(gs.catch_up(&s), Some(true));
    assert_eq!(gs.stage_no, Some(7));
    feed_at(&mut gs, "09:30:00", "ECLIPTICA - now in intermission");
    feed_at(&mut gs, "09:31:00", BIGCITY);
    assert_eq!(gs.stage_no, Some(8));
    assert_eq!(gs.summary().unwrap().stages, Some(8));
}

#[test]
fn a_sharer_who_joined_late_gives_bosses_but_no_stage_number() {
    let mut s = peer().summary().unwrap();
    s.stages = None;
    let mut gs = late();
    assert_eq!(gs.catch_up(&s), Some(true));
    assert_eq!(gs.runs[0].fights.len(), 2);
    assert_eq!(gs.stage_no, None);
    assert_eq!(gs.catch_up(&peer().summary().unwrap()), Some(true));
    assert_eq!(gs.stage_no, Some(2));
    assert_eq!(gs.runs[0].fights.len(), 2);
    assert_eq!(gs.catch_up(&peer().summary().unwrap()), Some(false));
}

#[test]
fn caught_up_history_passes_on_to_the_next_joiner() {
    let mut s = peer().summary().unwrap();
    s.marks.drain(..1);
    s.stages = Some(2);
    let mut middle = late();
    assert_eq!(middle.catch_up(&s), Some(true));
    let mut last = GameState::default();
    feed_at(
        &mut last,
        "09:26:00",
        "ECLIPTICA - now fighting boss: HaKli(Clone) on phase: 0",
    );
    assert_eq!(last.catch_up(&middle.summary().unwrap()), Some(true));
    let names: Vec<&str> = last.runs[0]
        .fights
        .iter()
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(names, ["Nan", "HaKli"]);
    assert_eq!(last.stage_no, Some(2));
    assert_eq!(last.runs[0].stages.len(), 1);
}
