use super::*;

#[test]
fn kill_dedupe() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:20:00", HALL);
    feed_at(
        &mut gs,
        "09:21:00",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0",
    );
    kill_at(&mut gs, "09:24:12", "Kakarot", 4793);
    kill_at(&mut gs, "09:24:18", "Kakarot", 0);
    assert_eq!(gs.last_kill.as_ref().unwrap().strike, 4793);
    assert_eq!(gs.runs[0].fights[0].kill, Some((4793, 0)));
    feed_at(&mut gs, "09:25:00", "ECLIPTICA - now in intermission");
    feed_at(&mut gs, "09:25:30", HALL);
    feed_at(
        &mut gs,
        "09:25:40",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0",
    );
    kill_at(&mut gs, "09:26:00", "Kakarot", 900);
    assert_eq!(gs.last_kill.as_ref().unwrap().strike, 900);
    assert_eq!(gs.runs[0].fights[1].kill, Some((900, 0)));
}

#[test]
fn kill_echoes_never_replace_the_kill() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "00:58:00", HALL);
    feed_at(
        &mut gs,
        "00:59:00",
        "ECLIPTICA - now fighting boss: Gravetender(Clone) on phase: 0",
    );
    let kill = |gs: &mut GameState, secs: u64, s: u64| {
        kill_at(gs, &fmt_clock(3600 + secs), "Gravetender", s);
    };
    kill(&mut gs, 0, 9173);
    feed_at(&mut gs, "01:00:20", "ECLIPTICA - now in intermission");
    let mut t = 13;
    while t <= 110 {
        kill(&mut gs, t, 0);
        t += 3;
    }
    assert_eq!(gs.last_kill.as_ref().unwrap().strike, 9173);
    assert_eq!(gs.runs[0].fights[0].kill, Some((9173, 0)));
    feed_at(&mut gs, "01:02:00", HALL);
    feed_at(
        &mut gs,
        "01:04:00",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
    );
    feed_at(
        &mut gs,
        "01:05:00",
        "Boss Nan dead, personal damage dealt: ",
    );
    feed_at(&mut gs, "01:05:00", "STRIKE DMG: 700");
    feed_at(&mut gs, "01:05:10", "ECLIPTICA - now in intermission");
    feed_at(&mut gs, "01:06:00", HALL);
    feed_at(
        &mut gs,
        "01:07:30",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0",
    );
    kill_at(&mut gs, "01:08:00", "Kakarot", 900);
    assert_eq!(gs.runs[0].fights[1].kill, Some((700, 0)));
    assert_eq!(gs.runs[0].fights[2].kill, Some((900, 0)));
}

#[test]
fn jim_final_phase_kill_is_a_win() {
    const BRINGER: &str =
        "ECLIPTICA - now in stage: Stage_Bringer on phase: 1 as class: Spellhammer";
    let jim = |gs: &mut GameState| {
        feed_at(gs, "10:34:13", BRINGER);
        feed_at(
            gs,
            "10:40:00",
            "ECLIPTICA - now fighting boss: JimBringer(Clone) on phase: 1",
        );
        kill_at(gs, "10:48:00", "JimBringer", 9000);
        feed_at(
            gs,
            "10:48:00",
            "ECLIPTICA - now fighting boss: JimBringerPhase2(Clone) on phase: 1",
        );
        kill_at(gs, "10:56:27", "JimBringerPhase2", 0);
        feed_at(
            gs,
            "10:56:27",
            "ECLIPTICA - now fighting boss: JimBringerPhase3(Clone) on phase: 1",
        );
    };
    let mut gs = GameState::default();
    jim(&mut gs);
    kill_at(&mut gs, "11:12:46", "JimBringerPhase3", 21059);
    feed_at(&mut gs, "11:13:30", LOBBY);
    assert_eq!(gs.runs[0].end, Some(RunEnd::Won));
    let mut gs = GameState::default();
    jim(&mut gs);
    kill_at(&mut gs, "11:12:46", "JimBringerPhase3", 21059);
    feed_at(&mut gs, "11:12:46", LOBBY);
    assert_eq!(gs.runs[0].end, Some(RunEnd::Lost));
    let mut gs = GameState::default();
    jim(&mut gs);
    kill_at(&mut gs, "11:12:46", "JimBringerPhase3", 21059);
    feed_at(&mut gs, "11:14:00", "[Behaviour] OnLeftRoom");
    assert_eq!(gs.runs[0].end, Some(RunEnd::Won));
    let mut gs = GameState::default();
    feed_at(&mut gs, "09:10:01", HALL);
    feed_at(
        &mut gs,
        "09:12:00",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
    );
    kill_at(&mut gs, "09:16:00", "Nan", 900);
    feed_at(&mut gs, "09:16:00", LOBBY);
    assert_eq!(gs.runs[0].end, Some(RunEnd::Lost));
}

#[test]
fn gilgamesh_final_phase_kill_is_a_win() {
    const GILGAMESH: [(&str, &str); 20] = [
        (
            "16:18:23",
            "ECLIPTICA - now in stage: Stage_Gilgamesh on phase: 1 as class: Thaumaturge",
        ),
        ("16:18:40", "Advancing Stage Event: 0"),
        (
            "16:18:40",
            "ECLIPTICA - now fighting boss: Gilgamesh(Clone) on phase: 1",
        ),
        ("16:18:40", "Advancing Stage Progress to: 1"),
        ("16:27:19", "Boss Gilgamesh dead, personal damage dealt: "),
        ("16:27:19", "STRIKE DMG: 27635"),
        ("16:27:19", "NON-STRIKE DMG: 13248"),
        (
            "16:27:19",
            "ECLIPTICA - now fighting boss: GilgameshPhase2(Clone) on phase: 1",
        ),
        ("16:33:06", "Advancing Stage Event: 1"),
        ("16:33:06", "Advancing Stage Progress to: 2"),
        ("16:33:22", "Advancing Stage Event: 2"),
        (
            "16:33:22",
            "ECLIPTICA - now fighting boss: GilgameshPhase3(Clone) on phase: 1",
        ),
        ("16:33:22", "Advancing Stage Progress to: 3"),
        (
            "16:33:22",
            "Boss GilgameshPhase2 dead, personal damage dealt: ",
        ),
        ("16:33:22", "STRIKE DMG: 718"),
        ("16:33:22", "NON-STRIKE DMG: 0"),
        ("16:45:33", "Advancing Stage Event: 3"),
        ("16:45:33", "Advancing Stage Progress to: 4"),
        (
            "16:45:39",
            "Boss GilgameshPhase3 dead, personal damage dealt: ",
        ),
        ("16:45:39", "STRIKE DMG: 65918"),
    ];
    let gilgamesh = |gs: &mut GameState| {
        for (t, msg) in GILGAMESH {
            feed_at(gs, t, msg);
        }
        feed_at(gs, "16:45:39", "NON-STRIKE DMG: 30297");
    };
    let mut gs = GameState::default();
    gilgamesh(&mut gs);
    let lobby = feed_at(&mut gs, "16:48:10", LOBBY);
    let fights: Vec<_> = gs.runs[0]
        .fights
        .iter()
        .map(|f| (f.name.as_str(), f.kill))
        .collect();
    assert_eq!(
        fights,
        vec![
            ("Gilgamesh", Some((27635, 13248))),
            ("GilgameshPhase2", Some((718, 0))),
            ("GilgameshPhase3", Some((65918, 30297))),
        ]
    );
    assert_eq!(gs.runs[0].end, Some(RunEnd::Won));
    let kill = lobby - 151;
    assert_eq!(gs.runs[0].end_ts, Some(kill));
    assert_eq!(kill - gs.runs[0].start_ts, 1636);
    let mut gs = GameState::default();
    gilgamesh(&mut gs);
    feed_at(&mut gs, "16:45:39", LOBBY);
    assert_eq!(gs.runs[0].end, Some(RunEnd::Lost));
    assert_eq!(gs.runs[0].end_ts, Some(kill));
    let mut gs = GameState::default();
    for (t, msg) in &GILGAMESH[..16] {
        feed_at(&mut gs, t, msg);
    }
    feed_at(&mut gs, "16:40:00", LOBBY);
    assert_eq!(gs.runs[0].end, Some(RunEnd::Lost));
}

#[test]
fn echoes_stop_at_the_next_stage_with_the_same_boss() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "03:02:42", HALL);
    feed_at(
        &mut gs,
        "03:06:01",
        "ECLIPTICA - now fighting boss: DarkMouth(Clone) on phase: 0",
    );
    kill_at(&mut gs, "03:09:07", "DarkMouth", 4000);
    feed_at(&mut gs, "03:10:47", "ECLIPTICA - now in intermission");
    kill_at(&mut gs, "03:12:41", "DarkMouth", 0);
    kill_at(&mut gs, "03:12:44", "DarkMouth", 0);
    feed_at(
        &mut gs,
        "03:12:44",
        "ECLIPTICA - now in stage: Stage_GMFuncFlat on phase: 0.1 as class: Spellhammer",
    );
    kill_at(&mut gs, "03:12:44", "DarkMouth", 0);
    feed_at(
        &mut gs,
        "03:16:48",
        "ECLIPTICA - now fighting boss: DarkMouth(Clone) on phase: 0.1",
    );
    assert_eq!(gs.boss.as_deref(), Some("DarkMouth"));
    assert_eq!(gs.runs[0].fights[1].kill, None);
    kill_at(&mut gs, "03:21:00", "DarkMouth", 5000);
    assert_eq!(gs.runs[0].fights[0].kill, Some((4000, 0)));
    assert_eq!(gs.runs[0].fights[1].kill, Some((5000, 0)));
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
