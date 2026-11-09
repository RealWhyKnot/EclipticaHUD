use super::*;

#[test]
fn party_button_opens_a_window_and_the_window_holds_the_toggle() {
    let mut app = headless();
    assert!(!app.flags.has(Flag::Party));
    assert!(app.hit_enabled(Hit::Party));
    assert!(app.activate(Hit::Party));
    assert!(app.party_win.visible);
    assert!(!app.flags.has(Flag::Party));
    assert_eq!(app.party_win.alpha, 0);
    app.render_party();
    assert!(app.party_activate(PartyHit::Toggle));
    assert!(app.flags.has(Flag::Party));
    assert!(!app.party_activate(PartyHit::Board));
    assert!(app.activate(Hit::Party));
    assert!(!app.party_win.visible);
    assert!(app.flags.has(Flag::Party));
    app.party_open();
    app.party_set_hover(Some(PartyHit::Close));
    assert!(app.party_activate(PartyHit::Close));
    assert!(!app.party_win.visible);
    assert_eq!(app.party_win.hover, None);
    app.activate(Hit::Settings);
    assert!(app.hit_enabled(Hit::Party));
    assert!(app.hit_enabled(Hit::Sound));
    assert!(!app.hit_enabled(Hit::Mute));
    let (x, y, w, h) = crate::hud::render::PARTY_CLOSE_HIT;
    assert_eq!(app.party_hit(x + w / 2, y + h / 2), Some(PartyHit::Close));
    assert_eq!(app.party_hit(4, 4), None);
}

#[test]
fn finished_shared_fights_are_saved_and_paged() {
    let (mut app, want) = party_app();
    app.set_history(crate::history::History::in_memory());
    let count = |app: &App| app.archive.db.as_ref().unwrap().party_count(0);
    feed(
        &mut app,
        "09:12:50",
        "ECLIPTICA - now in stage: Stage_Forest on phase: 0.1 as class: Blade",
    );
    let t0 = Instant::now();
    let step = |app: &mut App, entries: &[(&str, u64)]| {
        let s = snap(t0, &want, entries);
        app.party_step(t0, Some(&want), Some(poll(Link::Connected, 1, vec![s])));
    };
    feed(
        &mut app,
        "09:13:00",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
    );
    step(&mut app, &[("usr_a", 300), ("usr_me", 100)]);
    assert_eq!(count(&app), 0);
    feed(
        &mut app,
        "09:14:00",
        "Boss Kakarot dead, personal damage dealt: ",
    );
    step(&mut app, &[("usr_a", 320), ("usr_me", 100)]);
    assert_eq!(count(&app), 1);
    step(&mut app, &[("usr_a", 340), ("usr_me", 100)]);
    assert_eq!(count(&app), 1);
    let rec = app.archive.db.as_ref().unwrap().party_page(0, 0).unwrap();
    assert_eq!(rec.boss, "Kakarot");
    assert!(rec.down);
    assert!((rec.progress - 0.1).abs() < 1e-6);
    assert_eq!(rec.rows[0].dealt, 340);
    assert_eq!(app.party.pane().pages, 0);
    feed(
        &mut app,
        "09:20:00",
        "ECLIPTICA - now fighting boss: DarkMouth(Clone) on phase: 0.2",
    );
    step(&mut app, &[("usr_me", 50)]);
    assert_eq!(app.party.pane().pages, 1);
    assert_eq!(app.party.pane().page_label(), "2/2");
    assert!(app.party_activate(PartyHit::Older));
    let pane = app.party.pane();
    assert_eq!(pane.saved, Some(rec.at));
    assert_eq!(pane.board_head(), "SAVED FIGHT");
    assert_eq!(pane.page_label(), "1/2");
    assert_eq!(pane.rows[0].name, "Alice");
    assert_eq!(pane.rows[1].name, "Me");
    assert!(pane.rows[1].you);
    assert!(!app.party_activate(PartyHit::Older));
    step(&mut app, &[("usr_me", 60)]);
    assert_eq!(app.party.pane().saved, Some(rec.at));
    assert!(app.party_activate(PartyHit::Newer));
    assert_eq!(app.party.pane().saved, None);
    assert_eq!(app.party.pane().rows.len(), 1);
    assert!(app.party.pane().fight.as_ref().is_some_and(|f| f.live));
    app.party_activate(PartyHit::Older);
    feed(
        &mut app,
        "09:21:00",
        "Boss DarkMouth dead, personal damage dealt: ",
    );
    step(&mut app, &[("usr_me", 60)]);
    assert_eq!(count(&app), 1);
    feed(
        &mut app,
        "09:25:00",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0.3",
    );
    step(&mut app, &[("usr_me", 5)]);
    assert_eq!(app.party.pane().saved, None);
    assert_eq!(app.party.pane().page, 0);
    let (x, y, w, h) = crate::hud::render::PARTY_OLDER;
    assert_eq!(app.party_hit(x + w / 2, y + h / 2), Some(PartyHit::Older));
}

#[test]
fn a_fight_cut_short_by_leaving_is_still_saved() {
    let (mut app, want) = party_app();
    app.set_history(crate::history::History::in_memory());
    feed(
        &mut app,
        "09:13:00",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
    );
    let t0 = Instant::now();
    let s = snap(t0, &want, &[("usr_a", 300), ("usr_me", 100)]);
    app.party_step(t0, Some(&want), Some(poll(Link::Connected, 1, vec![s])));
    feed(&mut app, "09:13:30", "[Behaviour] OnLeftRoom");
    feed(&mut app, "09:13:40", W2);
    app.party_step(t0, None, Some(poll(Link::Connected, 1, Vec::new())));
    let db = app.archive.db.as_ref().unwrap();
    assert_eq!(db.party_count(0), 1);
    let rec = db.party_page(0, 0).unwrap();
    assert_eq!(rec.rows.len(), 2);
    assert_eq!(rec.rows[0].dealt, 300);
}

#[test]
fn a_wiped_fight_is_saved_as_not_down() {
    let (mut app, want) = party_app();
    app.set_history(crate::history::History::in_memory());
    feed(
        &mut app,
        "09:12:50",
        "ECLIPTICA - now in stage: Stage_Forest on phase: 0.7 as class: Blade",
    );
    feed(
        &mut app,
        "09:13:00",
        "ECLIPTICA - now fighting boss: Pandora(Clone) on phase: 0.76",
    );
    let t0 = Instant::now();
    let s = snap(t0, &want, &[("usr_a", 900), ("usr_me", 400)]);
    app.party_step(t0, Some(&want), Some(poll(Link::Connected, 1, vec![s])));
    assert_eq!(app.archive.db.as_ref().unwrap().party_count(0), 0);
    feed(
        &mut app,
        "09:19:20",
        "Boss Pandora dead, personal damage dealt: ",
    );
    feed(&mut app, "09:19:20", "ECLIPTICA - now in lobby");
    app.party_step(t0, Some(&want), Some(poll(Link::Connected, 1, Vec::new())));
    let db = app.archive.db.as_ref().unwrap();
    assert_eq!(db.party_count(0), 1);
    let rec = db.party_page(0, 0).unwrap();
    assert_eq!(rec.boss, "Pandora");
    assert!(!rec.down);
    assert_eq!(rec.secs, 380);
    assert_eq!(rec.rows[0].dealt, 900);
    app.party_step(t0, Some(&want), Some(poll(Link::Connected, 1, Vec::new())));
    assert_eq!(app.archive.db.as_ref().unwrap().party_count(0), 1);
}

#[test]
fn snaps_fill_the_table_with_roster_names() {
    let (mut app, want) = party_app();
    feed(
        &mut app,
        "09:13:00",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
    );
    feed(&mut app, "09:13:01", "Dealing 100 STRIKE damage");
    let t0 = Instant::now();
    let s = snap(
        t0,
        &want,
        &[("usr_a", 300), ("usr_b", 100), ("usr_me", 100)],
    );
    assert!(app.party_step(t0, Some(&want), Some(poll(Link::Connected, 1, vec![s]))));
    assert_eq!(rows(&app), [there("Alice", 300), mine(100)]);
    assert_eq!(app.party.pane().status().0, "connected");
    assert!(app.party.pane().ready);
    assert_eq!(app.party.pane().here, 2);
    assert!(app.party.pane().fight.as_ref().is_some_and(|f| f.live));
    assert!(!app.party_step(t0, Some(&want), Some(poll(Link::Connected, 1, Vec::new()))));
    let wrong = Snap {
        room: room_hash("wrld_other:1"),
        ..snap(t0, &want, &[("usr_a", 999)])
    };
    app.party_step(t0, Some(&want), Some(poll(Link::Connected, 1, vec![wrong])));
    assert_eq!(rows(&app)[0], there("Alice", 300));
    app.party_step(
        t0,
        None,
        Some(poll(
            Link::Connected,
            1,
            vec![snap(t0, &want, &[("usr_a", 999)])],
        )),
    );
    assert_eq!(rows(&app)[0], there("Alice", 300));
    let t1 = t0 + Duration::from_secs(1);
    let s = snap(t1, &want, &[("usr_a", 500), ("usr_b", 100)]);
    app.party_step(t1, Some(&want), Some(poll(Link::Connected, 1, vec![s])));
    assert_eq!(rows(&app)[0], there("Alice", 500));
    assert_eq!(app.party.pane().rows[0].dps, 200);
    assert_eq!(rows(&app).len(), 2);
    feed(
        &mut app,
        "09:13:02",
        "[Behaviour] OnPlayerJoined Bob (usr_b)",
    );
    app.party_step(t1, Some(&want), Some(poll(Link::Connected, 1, Vec::new())));
    assert_eq!(rows(&app)[2], there("Bob", 100));
}

#[test]
fn members_missing_from_a_snap_or_a_dropped_link_freeze() {
    let (mut app, want) = party_app();
    feed(
        &mut app,
        "09:13:00",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
    );
    let t0 = Instant::now();
    let s = snap(t0, &want, &[("usr_a", 300), ("usr_b", 100)]);
    app.party_step(t0, Some(&want), Some(poll(Link::Connected, 1, vec![s])));
    let t1 = t0 + Duration::from_secs(1);
    app.party_step(
        t1,
        Some(&want),
        Some(poll(
            Link::Connected,
            1,
            vec![snap(t1, &want, &[("usr_a", 400)])],
        )),
    );
    assert_eq!(rows(&app), [there("Alice", 400), mine(0)]);
    app.party_step(t1, Some(&want), Some(poll(Link::Retrying, 1, Vec::new())));
    assert_eq!(rows(&app)[0], gone("Alice", 400));
    assert_eq!(app.party.pane().status().0, "can't reach the server");
    app.party_step(
        t1,
        Some(&want),
        Some(poll(
            Link::Connected,
            2,
            vec![snap(t1, &want, &[("usr_a", 450)])],
        )),
    );
    assert_eq!(rows(&app)[0], there("Alice", 450));
    app.party_step(t1, Some(&want), Some(poll(Link::Connected, 3, Vec::new())));
    assert_eq!(rows(&app)[0], gone("Alice", 450));
}

#[test]
fn rows_clear_at_the_next_fight_and_reset_on_a_new_room() {
    let (mut app, want) = party_app();
    feed(
        &mut app,
        "09:12:40",
        "[Behaviour] OnPlayerJoined Bob (usr_b)",
    );
    feed(
        &mut app,
        "09:12:41",
        "[Behaviour] OnPlayerJoined Cara (usr_c)",
    );
    feed(
        &mut app,
        "09:13:00",
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
    );
    feed(&mut app, "09:13:01", "Dealing 100 STRIKE damage");
    let t0 = Instant::now();
    app.party_step(
        t0,
        Some(&want),
        Some(poll(
            Link::Connected,
            1,
            vec![snap(t0, &want, &[("usr_b", 300)])],
        )),
    );
    feed(
        &mut app,
        "09:13:30",
        "Boss Kakarot dead, personal damage dealt: ",
    );
    app.party_step(
        t0,
        Some(&want),
        Some(poll(Link::Connected, 1, vec![snap(t0, &want, &[])])),
    );
    assert_eq!(rows(&app), [gone("Bob", 300), mine(100)]);
    let fight = app.party.pane().fight.clone().unwrap();
    assert!(fight.down && !fight.live);
    assert_eq!(fight.secs, 30);
    feed(
        &mut app,
        "09:14:00",
        "ECLIPTICA - now fighting boss: DarkMouth(Clone) on phase: 0",
    );
    app.party_step(t0, Some(&want), Some(poll(Link::Connected, 1, Vec::new())));
    assert_eq!(rows(&app), [mine(0)]);
    app.party_step(
        t0,
        Some(&want),
        Some(poll(
            Link::Connected,
            1,
            vec![snap(t0, &want, &[("usr_c", 5), ("usr_b", 9)])],
        )),
    );
    assert_eq!(rows(&app)[0], there("Bob", 9));
    assert_eq!(rows(&app)[1], there("Cara", 5));
    feed(&mut app, "09:15:00", "[Behaviour] OnLeftRoom");
    feed(&mut app, "09:15:01", ENTER);
    feed(&mut app, "09:15:01", W2);
    feed(
        &mut app,
        "09:15:02",
        "[Behaviour] OnPlayerJoined Cara (usr_c)",
    );
    let want2 = relay::want(true, app.gs.location.as_deref(), Some("usr_me")).unwrap();
    assert_ne!(want2.room(), want.room());
    app.party_step(t0, Some(&want2), Some(poll(Link::Connected, 1, Vec::new())));
    assert_eq!(rows(&app).len(), 0);
    app.party_step(
        t0,
        Some(&want2),
        Some(poll(
            Link::Connected,
            2,
            vec![snap(t0, &want2, &[("usr_c", 7)])],
        )),
    );
    assert_eq!(rows(&app), [there("Cara", 7)]);
    app.party_step(
        t0,
        Some(&want2),
        Some(poll(
            Link::Connected,
            2,
            vec![snap(t0, &want2, &[("usr_c", 8), ("usr_b", 50)])],
        )),
    );
    assert_eq!(rows(&app), [there("Cara", 8)]);
}
