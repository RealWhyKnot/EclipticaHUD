use super::*;

#[test]
fn turning_sharing_off_clears_the_table() {
    let (mut app, want) = party_app();
    feed(
        &mut app,
        "09:13:00",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
    );
    let t0 = Instant::now();
    app.party_step(
        t0,
        Some(&want),
        Some(poll(
            Link::Connected,
            1,
            vec![snap(t0, &want, &[("usr_a", 300)])],
        )),
    );
    assert_ne!(app.party.pane().rows.len(), 0);
    app.set_party(false);
    assert!(app.party_step(t0, None, None));
    assert_eq!(rows(&app).len(), 0);
    assert!(!app.party.pane().on);
    assert_eq!(app.party.pane().status().0, "off");
    app.set_party(true);
    app.party_step(t0, Some(&want), None);
    assert_eq!(rows(&app), [mine(0)]);
    assert_eq!(app.party.pane().status().0, "connecting");
}

#[test]
fn sharing_off_never_touches_the_relay() {
    let mut app = headless();
    let fake = fake_relay(&mut app);
    app.flags.set(Flag::VrcRunning, true);
    feed(&mut app, "09:12:29", "User Authenticated: Me (usr_me)");
    feed(&mut app, "09:12:30", W1);
    feed(
        &mut app,
        "09:13:00",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
    );
    feed(&mut app, "09:13:01", "Dealing 100 STRIKE damage");
    for _ in 0..5 {
        assert!(!app.poll_party());
    }
    assert_eq!(fake.take().len(), 0);
    assert!(!app.party.pane().on);
}

#[test]
fn damage_is_sent_from_the_first_fight_on() {
    let (mut app, want) = party_app();
    let fake = fake_relay(&mut app);
    app.flags.set(Flag::VrcRunning, true);
    app.poll_party();
    assert_eq!(fake.take(), [(Some(want.clone()), None)]);
    feed(
        &mut app,
        "09:13:00",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
    );
    feed(&mut app, "09:13:01", "Dealing 100 STRIKE damage");
    feed(
        &mut app,
        "09:13:02",
        "damage has been taken: 25, from source: (Kakarot) attack_RockStomp",
    );
    app.poll_party();
    let sent = Some(Totals::from((100, 25)));
    assert_eq!(fake.take(), [(Some(want.clone()), sent)]);
    feed(
        &mut app,
        "09:13:30",
        "Boss Kakarot dead, personal damage dealt: ",
    );
    app.poll_party();
    assert_eq!(fake.take(), [(Some(want), sent)]);
    app.flags.set(Flag::VrcRunning, false);
    app.poll_party();
    assert_eq!(fake.take(), [(None, None)]);
}

#[test]
fn turning_sharing_off_tells_the_relay_once() {
    let (mut app, want) = party_app();
    let fake = fake_relay(&mut app);
    app.flags.set(Flag::VrcRunning, true);
    feed(
        &mut app,
        "09:13:00",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
    );
    feed(&mut app, "09:13:01", "Dealing 100 STRIKE damage");
    app.poll_party();
    assert_eq!(fake.take(), [(Some(want), Some(Totals::from((100, 0))))]);
    app.set_party(false);
    assert!(app.poll_party());
    assert_eq!(fake.take(), [(None, None)]);
    assert!(!app.poll_party());
    assert_eq!(fake.take().len(), 0);
}

#[test]
fn a_late_joiner_syncs_from_the_party_until_sharing_stops() {
    use crate::game::state::GameState;
    use crate::hud::render::RunSync;
    let (mut app, want) = party_app();
    app.flags.set(Flag::VrcRunning, true);
    feed(
        &mut app,
        "09:13:00",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
    );
    let t0 = Instant::now();
    app.party_step(
        t0,
        Some(&want),
        Some(poll(
            Link::Connected,
            1,
            vec![snap(t0, &want, &[("usr_me", 0), ("usr_a", 300)])],
        )),
    );
    let ts = |t: &str| {
        crate::game::event::split_line(&format!("2026.09.07 {t} Debug      -  x"))
            .unwrap()
            .ts
    };
    let now = ts("09:13:05");
    app.catch_up(Some(&want), &[], now);
    assert_eq!(app.run_sync(Instant::now()), RunSync::Waiting);
    assert_eq!(
        app.run_sync(Instant::now() + Duration::from_secs(31)),
        RunSync::Idle
    );
    let mut peer = GameState::default();
    for (t, m) in [
        (
            "09:00:00",
            "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer",
        ),
        (
            "09:05:00",
            "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
        ),
        ("09:08:00", "ECLIPTICA - now in intermission"),
        (
            "09:10:00",
            "ECLIPTICA - now in stage: Stage_Hallow on phase: 0.1 as class: Spellhammer",
        ),
        (
            "09:12:58",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
        ),
    ] {
        peer.feed(&format!("2026.09.07 {t} Debug      -  {m}"));
    }
    let run = crate::relay::PeerRun {
        at: Instant::now(),
        room: want.room(),
        held: 0,
        payload: crate::relay::proto::encode_run(&peer.summary().unwrap(), now),
    };
    app.catch_up(Some(&want), &[run], now);
    assert_eq!(app.run_sync(Instant::now()), RunSync::Done);
    assert_eq!(
        app.run_sync(Instant::now() + Duration::from_secs(9)),
        RunSync::Idle
    );
    assert_eq!(app.gs.stage_no, Some(2));
    let names: Vec<&str> = app.gs.runs[0]
        .fights
        .iter()
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(names, ["Nan", "Kakarot"]);
    assert!(app.gs.runs[0].fights[0].missed);
    app.set_party(false);
    assert!(app.gs.runs[0].fights.iter().all(|f| !f.missed));
    assert!(app.gs.runs[0].stages.is_empty());
    assert_eq!(app.gs.stage_no, None);
    assert_eq!(app.run_sync(Instant::now()), RunSync::Idle);
}

#[test]
fn leaving_the_world_tells_the_relay() {
    let (mut app, want) = party_app();
    let fake = fake_relay(&mut app);
    app.flags.set(Flag::VrcRunning, true);
    app.poll_party();
    assert_eq!(fake.take(), [(Some(want), None)]);
    feed(&mut app, "09:15:00", "[Behaviour] OnLeftRoom");
    app.poll_party();
    assert_eq!(fake.take(), [(None, None)]);
}

#[test]
fn snaps_from_a_dropped_link_are_never_applied() {
    let (mut app, want) = party_app();
    feed(
        &mut app,
        "09:13:00",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
    );
    let hub = relay::tests::hub();
    let id = relay::tests::connect(hub);
    let t0 = Instant::now();
    relay::tests::push(hub, id, snap(t0, &want, &[("usr_a", 300)]));
    app.party_step(t0, Some(&want), Some(hub.poll()));
    assert_eq!(rows(&app)[0], there("Alice", 300));
    let t1 = t0 + Duration::from_secs(1);
    relay::tests::push(hub, id, snap(t1, &want, &[("usr_a", 900)]));
    relay::tests::drop_link(hub);
    app.party_step(t1, Some(&want), Some(hub.poll()));
    assert_eq!(rows(&app)[0], gone("Alice", 300));
    relay::tests::push(hub, id, snap(t1, &want, &[("usr_a", 950)]));
    let p = hub.poll();
    assert!(p.snaps.is_empty());
    app.party_step(t1, Some(&want), Some(p));
    let stale = snap(t1, &want, &[("usr_a", 999)]);
    let session = hub.poll().session;
    app.party_step(
        t1,
        Some(&want),
        Some(poll(Link::Retrying, session, vec![stale])),
    );
    assert_eq!(rows(&app)[0], gone("Alice", 300));
}

#[test]
fn want_follows_the_toggle_and_the_world() {
    let (mut app, _) = party_app();
    app.flags.set(Flag::VrcRunning, true);
    assert!(
        relay::want(
            app.flags.has(Flag::Party) && app.gs.in_ecliptica(),
            app.gs.location.as_deref(),
            app.gs.me_usr.as_deref()
        )
        .is_some()
    );
    feed(&mut app, "09:15:00", "[Behaviour] OnLeftRoom");
    assert_eq!(app.gs.location, None);
    app.poll_party();
    assert_eq!(app.party.pane().rows.len(), 0);
}
