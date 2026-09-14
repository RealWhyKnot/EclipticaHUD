use super::*;

#[test]
fn kill_dedupe() {
    let mut gs = GameState::default();
    let kill = |gs: &mut GameState, t: &str, s: u64| {
        gs.feed(&format!(
            "2026.09.07 {t} Debug      -  Boss Kakarot dead, personal damage dealt: "
        ));
        gs.feed(&format!("2026.09.07 {t} Debug      -  STRIKE DMG: {s}"));
        gs.feed(&format!("2026.09.07 {t} Debug      -  NON-STRIKE DMG: 0"));
    };
    kill(&mut gs, "09:24:12", 4793);
    kill(&mut gs, "09:24:18", 0);
    let k = gs.last_kill.as_ref().unwrap();
    assert_eq!(k.strike, 4793);
    kill(&mut gs, "09:26:00", 900);
    assert_eq!(gs.last_kill.as_ref().unwrap().strike, 900);
}

#[test]
fn kill_echo_chain_outlives_window() {
    let mut gs = GameState::default();
    let kill = |gs: &mut GameState, secs: u64, s: u64| {
        let t = fmt_clock(3600 + secs);
        gs.feed(&format!(
            "2026.09.08 {t} Debug      -  Boss Gravetender dead, personal damage dealt: "
        ));
        gs.feed(&format!("2026.09.08 {t} Debug      -  STRIKE DMG: {s}"));
        gs.feed(&format!("2026.09.08 {t} Debug      -  NON-STRIKE DMG: 0"));
    };
    kill(&mut gs, 0, 9173);
    let first_ts = gs.last_kill.as_ref().unwrap().ts;
    let mut t = 13;
    while t <= 110 {
        kill(&mut gs, t, 0);
        t += 3;
    }
    let k = gs.last_kill.as_ref().unwrap();
    assert_eq!(k.strike, 9173);
    assert_eq!(k.ts, first_ts);
    kill(&mut gs, 380, 0);
    assert_eq!(gs.last_kill.as_ref().unwrap().strike, 0);
}

#[test]
fn jim_kill_then_lobby_is_a_win() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:10:01", HALL);
    feed_at(
        &mut gs,
        "09:12:00",
        "ECLIPTICA - now fighting boss: JimBringer(Clone) on phase: 1",
    );
    feed_at(
        &mut gs,
        "09:14:00",
        "ECLIPTICA - now fighting boss: JimBringerPhase2(Clone) on phase: 1",
    );
    kill_at(&mut gs, "09:16:00", "JimBringerPhase2", 900);
    feed_at(&mut gs, "09:16:01", LOBBY);
    assert!(gs.runs[0].won);
    assert!(!gs.runs[0].lost);
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:10:01", HALL);
    feed_at(
        &mut gs,
        "09:12:00",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
    );
    kill_at(&mut gs, "09:16:00", "Nan", 900);
    feed_at(&mut gs, "09:16:01", LOBBY);
    assert!(!gs.runs[0].won);
}

#[test]
fn boss_dead_without_totals_still_records_kill() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:10:01", HALL);
    feed_at(
        &mut gs,
        "09:12:00",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
    );
    feed_at(
        &mut gs,
        "09:13:00",
        "Boss Nan dead, personal damage dealt: ",
    );
    feed_at(&mut gs, "09:13:00", "STRIKE DMG: 700");
    feed_at(
        &mut gs,
        "09:13:30",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0",
    );
    kill_at(&mut gs, "09:14:00", "Kakarot", 900);
    assert_eq!(gs.runs[0].fights[0].kill, Some((700, 0)));
    assert_eq!(gs.runs[0].fights[1].kill, Some((900, 0)));
}

#[test]
fn last_kill_sums_the_phases() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:10:01", HALL);
    assert_eq!(gs.last_kill_total(), None);
    feed_at(
        &mut gs,
        "09:12:00",
        "ECLIPTICA - now fighting boss: Mephiel(Clone) on phase: 0.8",
    );
    kill_at(&mut gs, "09:15:00", "Mephiel", 6388);
    assert_eq!(gs.last_kill_total(), Some(("Mephiel".into(), 6388, 0)));
    feed_at(
        &mut gs,
        "09:15:02",
        "ECLIPTICA - now fighting boss: MephielPhase2(Clone) on phase: 0.8",
    );
    feed_at(
        &mut gs,
        "09:18:00",
        "Boss MephielPhase2 dead, personal damage dealt: ",
    );
    feed_at(&mut gs, "09:18:00", "STRIKE DMG: 5113");
    feed_at(&mut gs, "09:18:00", "NON-STRIKE DMG: 7");
    assert_eq!(gs.last_kill_total(), Some(("Mephiel".into(), 11501, 7)));
    feed_at(&mut gs, "09:18:05", "ECLIPTICA - now in intermission");
    assert_eq!(gs.last_kill_total(), Some(("Mephiel".into(), 11501, 7)));
    feed_at(
        &mut gs,
        "09:20:00",
        "ECLIPTICA - now in stage: Stage_GMFuncFlat on phase: 0.9 as class: Spellhammer",
    );
    feed_at(
        &mut gs,
        "09:22:00",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0.9",
    );
    kill_at(&mut gs, "09:23:00", "Nan", 100);
    assert_eq!(gs.last_kill_total(), Some(("Nan".into(), 100, 0)));
}

#[test]
fn gap_after_a_kill_is_not_pre_boss() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:10:01", HALL);
    assert!(gs.pre_boss());
    feed_at(
        &mut gs,
        "09:11:01",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0.05",
    );
    kill_at(&mut gs, "09:12:00", "Nan", 100);
    assert!(gs.boss.is_none());
    assert_eq!(gs.mode, Mode::Stage);
    assert!(!gs.pre_boss());
    feed_at(&mut gs, "09:12:04", "Dealing 9 STRIKE damage");
    assert_eq!(gs.stage_stats.dmg, 0);
    feed_at(&mut gs, "09:12:10", "ECLIPTICA - now in intermission");
    assert_eq!(gs.live_run().unwrap().stages.len(), 1);
}
