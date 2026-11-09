use super::*;

#[test]
fn a_session_becomes_clean_lines() {
    let mut db = fresh();
    add(&mut db, LOG, SESSION);
    let batch = build(&db, "1.2.3", MAX_TEXT).unwrap();
    let (head, rest) = batch.text.split_once('\n').unwrap();
    let f: Vec<&str> = head.split('\t').collect();
    assert_eq!(f.len(), 6);
    assert_eq!(&f[..2], &["v2", "1.2.3"]);
    assert!(f[2].len() == 32 && f[2].bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(f[3], "2026-10-23");
    assert_eq!(f[4], "1792749600");
    assert_eq!(f[5], utc_offset_at(base()).to_string());
    assert_eq!(masked(rest), WANT);
    for word in ["WhyKnot", "Lena", "usr_", "private", "region"] {
        assert!(!rest.contains(word), "{word}");
    }
    assert_eq!(
        batch.cursors,
        vec![(LOG.to_string(), crate::num::to_u64(SESSION.len()))]
    );
}

#[test]
fn other_worlds_only_show_up_as_other() {
    let mut db = fresh();
    let mut msgs = vec![
        "User Authenticated: WhyKnot (usr_11111111-2222-3333-4444-555555555555)",
        "[Behaviour] Entering Room: Club Night",
        "[Behaviour] Joining wrld_11111111-5c2a-4c7b-8a0a-0123456789ab:1~public",
        "[Behaviour] OnPlayerJoined WhyKnot (usr_11111111-2222-3333-4444-555555555555)",
        "[Behaviour] OnPlayerJoined Lena (usr_aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee)",
        "damage has been taken: 7, from source: Lena",
        "Tracking boss as defeated in-run.",
        "[Behaviour] OnLeftRoom",
    ];
    msgs.extend_from_slice(SESSION);
    msgs.extend_from_slice(&[
        "[Behaviour] Entering Room: Home",
        "[Behaviour] Joining wrld_22222222-5c2a-4c7b-8a0a-0123456789ab:2~public",
        "[Behaviour] OnPlayerJoined WhyKnot (usr_11111111-2222-3333-4444-555555555555)",
    ]);
    add(&mut db, LOG, &msgs);
    let got = body(&build(&db, "1", MAX_TEXT).unwrap().text);
    let mut want = vec!["log\th\t0".to_string(), "room_enter\tother".to_string()];
    want.extend(tails(WANT).into_iter().skip(1));
    want.push("room_enter\tother".to_string());
    assert_eq!(tails(&got), want);
}

#[test]
fn scrub_drops_names_and_ids() {
    let me = ["WhyKnot".to_string()];
    let others = ["Lena".to_string(), "Al".to_string(), "  ".to_string()];
    let mut scrub = Scrub::new(&me, &others);
    assert_eq!(scrub.clean("Lena's Fireball"), "P1's Fireball");
    assert_eq!(scrub.clean("WhyKnot hit Lena, Lena!"), "you hit P1, P1!");
    assert_eq!(scrub.clean("Removed player Al"), "Removed player P2");
    assert_eq!(scrub.clean("Al"), "P2");
    assert_eq!(scrub.clean(" Al "), "P2");
    assert_eq!(scrub.clean("Altar"), "Altar");
    assert_eq!(scrub.clean("Lena1 and Malena"), "Lena1 and Malena");
    assert_eq!(scrub.who("Lena"), "P1");
    assert_eq!(scrub.who(" WhyKnot "), "you");
    assert_eq!(scrub.who("Stranger"), "P3");
    assert_eq!(scrub.clean("hit by usr_abc-123 twice"), "hit by usr twice");
    assert_eq!(scrub.clean("a\tb\nc\u{1b}"), "a b c");
    assert_eq!(scrub.clean(&"x".repeat(5000)).len(), MAX_STR);
    assert_eq!(scrub.clean(&"気".repeat(5000)).chars().count(), MAX_STR);
}

#[test]
fn names_come_from_the_whole_history() {
    let mut db = fresh();
    add(&mut db, LOG, SESSION);
    db.save_party(&party(
        base(),
        &[
            ("Bea", false, 1, 1),
            ("WhyKnot", true, 1, 1),
            ("Old Me", true, 1, 1),
        ],
    ));
    let (me, others) = db.known_names();
    assert_eq!(me, vec!["Old Me".to_string(), "WhyKnot".to_string()]);
    assert_eq!(
        others,
        vec!["Al".to_string(), "Bea".to_string(), "Lena".to_string()]
    );
}

#[test]
fn nothing_before_opt_in_is_sent() {
    let mut db = History::in_memory().unwrap();
    add(&mut db, LOG, SESSION);
    db.save_party(&party(base() + 5, &[("Lena", false, 10, 2)]));
    assert!(build(&db, "1.0", MAX_TEXT).is_none());
    db.usage_start();
    assert!(build(&db, "1.0", MAX_TEXT).is_none());
    add(&mut db, LOG, &[ENTER, "Dealing 7 STRIKE damage"]);
    let batch = build(&db, "1.0", MAX_TEXT).unwrap();
    assert_eq!(
        body(&batch.text),
        format!(
            "0\tlog\th\t{}\n0\troom_enter\tEcliptica\n1\tdealt\t7\t1\n",
            SESSION.len()
        )
    );
}

#[test]
fn committed_lines_are_not_sent_again() {
    let mut db = fresh();
    add(&mut db, LOG, SESSION);
    let first = build(&db, "1", MAX_TEXT).unwrap();
    assert!(db.usage_commit(&first.cursors));
    assert!(build(&db, "1", MAX_TEXT).is_none());
    add(
        &mut db,
        LOG,
        &[
            ENTER,
            "Dealing 9 NON-STRIKE damage",
            "damage has been taken: 5, from source: Lena",
        ],
    );
    let next = build(&db, "1", MAX_TEXT).unwrap();
    assert_eq!(
        body(&next.text),
        format!(
            "0\tlog\th\t{}\n0\troom_enter\tEcliptica\n1\tdealt\t9\t0\n1\tdamage_taken\t5\tP1\n",
            SESSION.len()
        )
    );
}

#[test]
fn old_logs_found_later_are_skipped_and_new_ones_sent() {
    let mut db = History::in_memory().unwrap();
    let mid = "output_log_2026-10-10_10-00-00.txt";
    add(&mut db, mid, &[ENTER, "Dealing 1 STRIKE damage"]);
    db.usage_start();
    add(
        &mut db,
        "output_log_2026-10-01_10-00-00.txt",
        &[ENTER, "Dealing 2 STRIKE damage"],
    );
    add(
        &mut db,
        "output_log_2026-10-20_10-00-00.txt",
        &[ENTER, "Dealing 3 STRIKE damage"],
    );
    let batch = build(&db, "1", MAX_TEXT).unwrap();
    assert_eq!(
        body(&batch.text),
        "0\tlog\th\t2\n0\tlog_end\t2\n0\tlog\th\t0\n0\troom_enter\tEcliptica\n1\tdealt\t3\t1\n"
    );
    assert!(db.usage_commit(&batch.cursors));
    assert!(build(&db, "1", MAX_TEXT).is_none());
}

#[test]
fn a_closed_log_ends_its_open_run() {
    let mut db = fresh();
    add(
        &mut db,
        LOG,
        &[
            ENTER,
            "ECLIPTICA - now in stage: Stage_GMBigcity on phase: 0.5 as class: Thaumaturge",
            "ECLIPTICA - now fighting boss: Hakli(Clone) on phase: 0.25",
            "Dealing 40 STRIKE damage",
            "Local controller dead, switching off.",
        ],
    );
    let first = build(&db, "1", MAX_TEXT).unwrap();
    assert!(db.usage_commit(&first.cursors));
    let text = body(&first.text);
    assert!(text.contains("\tboss_fight\tHakli\t0.25\n"), "{text}");
    assert_eq!(text.matches("\tdeath\n").count(), 0);
    assert!(!text.contains("run_end"));
    add(&mut db, LATER, &[ENTER]);
    let next = build(&db, "1", MAX_TEXT).unwrap();
    let text = body(&next.text);
    assert!(text.starts_with("0\tlog\th\t5\n"), "{text}");
    assert!(
        text.contains("\tfight_end\tHakli\t1\tunfinished\t"),
        "{text}"
    );
    assert!(text.contains("\trun_end\tleft\t"), "{text}");
    assert!(text.contains("\tlog_end\t5\n"), "{text}");
    assert!(db.usage_commit(&next.cursors));
    assert!(build(&db, "1", MAX_TEXT).is_none());
}
