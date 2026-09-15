use super::*;

#[test]
fn boss_gating() {
    let mut gs = GameState::default();
    gs.feed(&format!("{P}ownership of Neko1 transferred to Alice"));
    assert!(gs.target.is_none());
    gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1"
    ));
    gs.feed(&format!("{P}ownership of Neko1 transferred to Alice"));
    assert!(gs.target.is_none());
    gs.feed(&format!("{P}ownership of Kakarot transferred to Alice"));
    assert_eq!(gs.target.as_deref(), Some("Alice"));
    assert_eq!(gs.history.len(), 1);
}

#[test]
fn boss_flap_after_kill_ignored() {
    let mut gs = GameState::default();
    feed_at(
        &mut gs,
        "10:55:34",
        "ECLIPTICA - now fighting boss: AntKing(Clone) on phase: 0.9562449",
    );
    feed_at(
        &mut gs,
        "10:58:36",
        "Boss AntKing dead, personal damage dealt: ",
    );
    feed_at(&mut gs, "10:58:36", "STRIKE DMG: 23681");
    feed_at(&mut gs, "10:58:36", "NON-STRIKE DMG: 0");
    feed_at(
        &mut gs,
        "10:58:36",
        "ECLIPTICA - now fighting boss: AntKingPhase2(Clone) on phase: 0.9562449",
    );
    feed_at(
        &mut gs,
        "10:58:36",
        "ECLIPTICA - now fighting boss: AntKing(Clone) on phase: 0.9562449",
    );
    feed_at(
        &mut gs,
        "10:58:36",
        "ECLIPTICA - now fighting boss: AntKingPhase2(Clone) on phase: 0.9562449",
    );
    assert_eq!(gs.boss.as_deref(), Some("AntKingPhase2"));
    let run = &gs.runs[0];
    assert_eq!(run.fights.len(), 2);
    assert_eq!(run.fights[0].kill, Some((23681, 0)));
    assert_eq!(run.groups(), vec![0..2]);
}

#[test]
fn stray_phase1_before_kill_triple_ignored() {
    let mut gs = GameState::default();
    feed_at(
        &mut gs,
        "05:38:15",
        "ECLIPTICA - now fighting boss: Bravera(Clone) on phase: 0.9",
    );
    feed_at(
        &mut gs,
        "05:42:52",
        "ECLIPTICA - now fighting boss: BraveraPhase2(Clone) on phase: 0.9",
    );
    feed_at(
        &mut gs,
        "05:42:53",
        "ECLIPTICA - now fighting boss: Bravera(Clone) on phase: 0.9",
    );
    feed_at(
        &mut gs,
        "05:42:53",
        "Boss Bravera dead, personal damage dealt: ",
    );
    feed_at(&mut gs, "05:42:53", "STRIKE DMG: 18620");
    feed_at(&mut gs, "05:42:53", "NON-STRIKE DMG: 0");
    assert_eq!(gs.boss.as_deref(), Some("BraveraPhase2"));
    let run = &gs.runs[0];
    assert_eq!(run.fights.len(), 2);
    assert_eq!(run.fights[0].kill, Some((18620, 0)));
    assert_eq!(run.groups(), vec![0..2]);
}

#[test]
fn boss_flap_after_lobby_ignored() {
    let mut gs = GameState::default();
    feed_at(
        &mut gs,
        "10:34:13",
        "ECLIPTICA - now in stage: Stage_Bringer on phase: 1 as class: Spellhammer",
    );
    feed_at(
        &mut gs,
        "10:56:27",
        "ECLIPTICA - now fighting boss: JimBringerPhase3(Clone) on phase: 1",
    );
    feed_at(
        &mut gs,
        "11:12:46",
        "Boss JimBringerPhase3 dead, personal damage dealt: ",
    );
    feed_at(&mut gs, "11:12:46", "STRIKE DMG: 21059");
    feed_at(&mut gs, "11:12:46", "NON-STRIKE DMG: 0");
    feed_at(&mut gs, "11:12:46", "ECLIPTICA - now in lobby");
    feed_at(
        &mut gs,
        "11:12:46",
        "ECLIPTICA - now fighting boss: JimBringerPhase3(Clone) on phase: 0",
    );
    assert_eq!(gs.runs.len(), 1);
    assert!(gs.boss.is_none());
    assert_eq!(gs.mode, Mode::Lobby);
}

#[test]
fn boss_without_stage_still_records() {
    let mut gs = GameState::default();
    feed_at(
        &mut gs,
        "09:00:00",
        "ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0",
    );
    assert_eq!(gs.runs.len(), 1);
    assert_eq!(gs.runs[0].fights[0].name, "Yuki");
}

#[test]
fn boss_fight_resets() {
    let mut gs = GameState::default();
    gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0.6"
    ));
    gs.feed(&format!("{P}Dealing 100 STRIKE damage"));
    gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.6"
    ));
    assert_eq!(gs.fight_dmg, 0);
    assert_eq!(gs.boss.as_deref(), Some("Kakarot"));
}

#[test]
fn phase_transition_keeps_live_stats() {
    let mut gs = GameState::default();
    let t0 = feed_at(
        &mut gs,
        "12:21:33",
        "ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0.6",
    );
    feed_at(&mut gs, "12:21:40", "Dealing 100 STRIKE damage");
    feed_at(
        &mut gs,
        "12:22:45",
        "ECLIPTICA - now fighting boss: YukiPhase2(Clone) on phase: 0.6",
    );
    feed_at(&mut gs, "12:22:50", "Dealing 50 STRIKE damage");
    assert_eq!(gs.fight_dmg, 150);
    assert_eq!(gs.fight_start, t0);
    assert_eq!(gs.boss.as_deref(), Some("YukiPhase2"));
    let run = &gs.runs[0];
    assert_eq!(run.fights.len(), 2);
    assert_eq!(run.groups(), vec![0..2]);
    assert_eq!(run.fights[0].dmg, 100);
    assert_eq!(run.fights[1].dmg, 50);
}

#[test]
fn jimbringer_three_phase_ordering() {
    let mut gs = GameState::default();
    let t0 = feed_at(
        &mut gs,
        "10:34:45",
        "ECLIPTICA - now fighting boss: JimBringer(Clone) on phase: 1",
    );
    feed_at(&mut gs, "10:40:00", "Dealing 500 STRIKE damage");
    feed_at(
        &mut gs,
        "10:46:56",
        "Boss JimBringer dead, personal damage dealt: ",
    );
    feed_at(&mut gs, "10:46:56", "STRIKE DMG: 1000");
    feed_at(&mut gs, "10:46:56", "NON-STRIKE DMG: 0");
    feed_at(
        &mut gs,
        "10:46:57",
        "ECLIPTICA - now fighting boss: JimBringerPhase2(Clone) on phase: 1",
    );
    feed_at(
        &mut gs,
        "10:46:57",
        "ECLIPTICA - now fighting boss: JimBringerPhase2(Clone) on phase: 1",
    );
    feed_at(&mut gs, "10:50:00", "Dealing 700 STRIKE damage");
    feed_at(
        &mut gs,
        "10:56:27",
        "ECLIPTICA - now fighting boss: JimBringerPhase3(Clone) on phase: 1",
    );
    feed_at(
        &mut gs,
        "10:56:27",
        "Boss JimBringerPhase2 dead, personal damage dealt: ",
    );
    feed_at(&mut gs, "10:56:27", "STRIKE DMG: 2000");
    feed_at(&mut gs, "10:56:27", "NON-STRIKE DMG: 0");
    feed_at(&mut gs, "10:57:00", "Dealing 300 STRIKE damage");
    assert_eq!(gs.boss.as_deref(), Some("JimBringerPhase3"));
    assert_eq!(gs.fight_dmg, 1500);
    assert_eq!(gs.fight_start, t0);
    let run = &gs.runs[0];
    assert_eq!(run.fights.len(), 3);
    assert_eq!(run.groups(), vec![0..3]);
    assert_eq!(run.fights[0].kill, Some((1000, 0)));
    assert_eq!(run.fights[1].kill, Some((2000, 0)));
    assert_eq!(run.fights[2].kill, None);
    assert_eq!(run.fights[1].dmg, 700);
    assert_eq!(run.fights[2].dmg, 300);
}

#[test]
fn boss_line_in_intermission_is_ignored() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:10:01", HALL);
    feed_at(
        &mut gs,
        "09:12:00",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
    );
    kill_at(&mut gs, "09:15:00", "Nan", 500);
    feed_at(&mut gs, "09:15:30", "ECLIPTICA - now in intermission");
    feed_at(
        &mut gs,
        "09:15:33",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
    );
    assert!(gs.boss.is_none());
    assert_eq!(gs.runs[0].fights.len(), 1);
}

#[test]
fn intermission_closes_the_fight() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:10:01", HALL);
    feed_at(
        &mut gs,
        "09:12:00",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
    );
    let t = feed_at(&mut gs, "09:12:30", "ECLIPTICA - now in intermission");
    assert_eq!(gs.runs[0].fights[0].end_ts, Some(t));
}
