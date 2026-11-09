use super::*;

#[test]
fn a_run_open_at_opt_in_is_not_summed_up() {
    let mut db = History::in_memory().unwrap();
    add(&mut db, LOG, RUN);
    db.usage_start();
    add(
        &mut db,
        LOG,
        &[
            "Dealing 9 STRIKE damage",
            "ECLIPTICA - now in lobby",
            "ECLIPTICA - now in stage: Stage_GMBigcity on phase: 0.5 as class: Thaumaturge",
            "Dealing 3 STRIKE damage",
            "ECLIPTICA - now in lobby",
        ],
    );
    let text = body(&build(&db, "1", MAX_TEXT).unwrap().text);
    assert!(text.starts_with("0\tlog\th\t4\n"), "{text}");
    assert!(text.contains("\tdealt\t9\t1\n"), "{text}");
    assert_eq!(text.matches("\trun_end\t").count(), 1, "{text}");
    assert_eq!(text.matches("\tstage_end\t").count(), 1, "{text}");
    assert!(!text.contains("fight_end"), "{text}");
    assert!(
        text.contains("\tThaumaturge\tGMBigcity\t3\t0\t0\t0\n"),
        "{text}"
    );
}

#[test]
fn a_run_cut_off_before_opt_in_is_not_summed_up() {
    let mut db = History::in_memory().unwrap();
    add(&mut db, LOG, RUN);
    db.usage_start();
    add(&mut db, LATER, &[ENTER]);
    let batch = build(&db, "1", MAX_TEXT).unwrap();
    let text = body(&batch.text);
    assert_eq!(
        text,
        "0\tlog\th\t4\n0\tlog_end\t4\n0\tlog\th\t0\n0\troom_enter\tEcliptica\n",
    );
    assert!(db.usage_commit(&batch.cursors));
    assert!(build(&db, "1", MAX_TEXT).is_none());
}

#[test]
fn lines_an_upgrade_finds_in_finished_logs_stay_out() {
    let mut db = History::in_memory().unwrap();
    add(&mut db, LOG, RUN);
    db.usage_start();
    add(&mut db, LATER, &[ENTER]);
    let first = build(&db, "1", MAX_TEXT).unwrap();
    assert!(db.usage_commit(&first.cursors));
    db.forget_skew();
    let mut old = kept(RUN, base());
    old.push((base() + 3, "ECLIPTICA - leftover one".into()));
    old.push((base() + 3, "ECLIPTICA - leftover two".into()));
    let mut live = kept(&[ENTER], base());
    live.push((base() + 1, "ECLIPTICA - new after restart".into()));
    for (name, rows) in [(LOG, &old), (LATER, &live)] {
        let (have, skew) = db.resume(name, rows);
        db.archive(
            name,
            have - skew,
            &rows[crate::num::to_usize(have)..],
            base() + 3,
        );
    }
    assert_eq!(db.stored(LOG), 6);
    let next = build(&db, "1", MAX_TEXT).unwrap();
    assert_eq!(
        body(&next.text),
        "0\tlog\th\t1\n0\tworld_line\tECLIPTICA - new after restart\n"
    );
    assert!(db.usage_commit(&next.cursors));
    assert!(build(&db, "1", MAX_TEXT).is_none());
}

#[test]
fn a_finished_log_that_grows_sends_every_new_line() {
    let mut db = History::in_memory().unwrap();
    add(&mut db, LOG, &[ENTER]);
    db.usage_start();
    add(&mut db, LATER, &[ENTER]);
    let first = build(&db, "1", MAX_TEXT).unwrap();
    assert!(db.usage_commit(&first.cursors));
    add(
        &mut db,
        LOG,
        &["Dealing 7 STRIKE damage", "Dealing 8 STRIKE damage"],
    );
    let next = build(&db, "1", MAX_TEXT).unwrap();
    assert_eq!(
        body(&next.text),
        "0\tlog\th\t1\n0\tdealt\t7\t1\n1\tdealt\t8\t1\n0\tlog_end\t3\n"
    );
    assert!(db.usage_commit(&next.cursors));
    assert!(build(&db, "1", MAX_TEXT).is_none());
}

#[test]
fn lines_the_old_build_saw_before_opt_in_stay_out_of_the_live_log() {
    let mut db = History::in_memory().unwrap();
    add(&mut db, LOG, &[ENTER]);
    db.archive(LOG, 1, &[], base() + 2);
    db.usage_start();
    db.forget_skew();
    let mut rows = kept(&[ENTER], base());
    rows.push((base() + 1, "ECLIPTICA - seen before opt-in".into()));
    rows.push((base() + 3, "ECLIPTICA - after the old build".into()));
    let (have, skew) = db.resume(LOG, &rows);
    assert_eq!((have, skew), (1, 0));
    db.archive(
        LOG,
        have - skew,
        &rows[crate::num::to_usize(have)..],
        base() + 3,
    );
    assert_eq!(db.stored(LOG), 3);
    let batch = build(&db, "1", MAX_TEXT).unwrap();
    assert_eq!(
        body(&batch.text),
        "0\tlog\th\t2\n0\tworld_line\tECLIPTICA - after the old build\n"
    );
}

#[test]
fn lines_added_to_a_log_finished_before_opt_in_stay_out() {
    let mut db = History::in_memory().unwrap();
    add(&mut db, LOG, &[ENTER]);
    add(&mut db, LATER, &[ENTER]);
    db.usage_start();
    add(&mut db, LOG, &["Dealing 7 STRIKE damage"]);
    add(&mut db, LATER, &["Dealing 9 STRIKE damage"]);
    let batch = build(&db, "1", MAX_TEXT).unwrap();
    assert_eq!(body(&batch.text), "0\tlog\th\t1\n0\tdealt\t9\t1\n");
    assert!(db.usage_commit(&batch.cursors));
    assert!(build(&db, "1", MAX_TEXT).is_none());
}

#[test]
fn an_empty_first_read_does_not_hide_new_lines_after_an_upgrade() {
    let mut db = History::in_memory().unwrap();
    add(&mut db, LOG, &[ENTER]);
    db.usage_start();
    db.forget_skew();
    db.archive(LOG, 0, &[], base() + 5);
    let mut rows = kept(&[ENTER], base());
    rows.push((base() + 3, "ECLIPTICA - after the old build".into()));
    let (have, skew) = db.resume(LOG, &rows);
    db.archive(
        LOG,
        have - skew,
        &rows[crate::num::to_usize(have)..],
        base() + 5,
    );
    let batch = build(&db, "1", MAX_TEXT).unwrap();
    assert_eq!(
        body(&batch.text),
        "0\tlog\th\t1\n0\tworld_line\tECLIPTICA - after the old build\n"
    );
}

#[test]
fn fights_from_before_opt_in_stay_out_when_saved_again() {
    let mut db = History::in_memory().unwrap();
    add(&mut db, LOG, SESSION);
    db.save_party(&party(base() + 5, &[("Lena", false, 10, 2)]));
    db.usage_start();
    db.save_party(&party(base() + 5, &[("Lena", false, 30, 2)]));
    assert!(build(&db, "1", MAX_TEXT).is_none());
    let later = base() + crate::num::to_u64(SESSION.len()) + 1;
    db.save_party(&party(later, &[("Lena", false, 7, 1)]));
    let text = build(&db, "1", MAX_TEXT).unwrap().text;
    assert_eq!(text.matches("\tparty\t").count(), 1, "{text}");
    assert!(text.ends_with("\t7:1:P1\n"), "{text}");
    assert!(!text.contains("30:2"), "{text}");
}

#[test]
fn world_lines_are_kept_and_retires_packed() {
    let mut db = fresh();
    add(
        &mut db,
        LOG,
        &[
            ENTER,
            "[Behaviour] OnPlayerJoined Lena (usr_aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee)",
            "Tracking boss as defeated in-run.",
            "Removed player Lena",
            "fading out music #2",
        ],
    );
    let retires: Vec<String> = (0..201)
        .map(|i| format!("Retiring Enemy POOL ID{i}"))
        .collect();
    let refs: Vec<&str> = retires.iter().map(String::as_str).collect();
    add_at(&mut db, LOG, base() + 9, &refs);
    add_at(&mut db, LOG, base() + 10, &["Retiring Enemy POOL ID7"]);
    let text = body(&build(&db, "1", MAX_TEXT).unwrap().text);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines[4], "1\tworld_line\tTracking boss as defeated in-run.",
        "{text}"
    );
    assert_eq!(lines[5], "1\tworld_line\tRemoved player P1");
    assert_eq!(lines[6], "1\tworld_line\tfading out music #2");
    let all: Vec<String> = (0..200).map(|i| i.to_string()).collect();
    assert_eq!(lines[7], format!("5\tenemy_retire\t{}", all.join(",")));
    assert_eq!(lines[8], "0\tenemy_retire\t200");
    assert_eq!(lines[9], "1\tenemy_retire\t7");
    assert_eq!(lines.len(), 10);
}

#[test]
fn a_small_limit_splits_without_losing_lines() {
    let mut db = fresh();
    add(&mut db, LOG, SESSION);
    add(&mut db, LATER, SESSION);
    let mut got = Vec::new();
    let mut rounds = 0;
    while let Some(batch) = build(&db, "1", 64) {
        got.extend(tails(&body(&batch.text)));
        assert!(db.usage_commit(&batch.cursors));
        rounds += 1;
        assert!(rounds < 100);
    }
    assert!(rounds > 4);
    let ends: Vec<&String> = got.iter().filter(|l| l.starts_with("log_end\t")).collect();
    assert_eq!(ends, vec![&format!("log_end\t{}", SESSION.len())]);
    got.retain(|l| !l.starts_with("log\t") && !l.starts_with("log_end\t"));
    let mut want: Vec<String> = tails(WANT).into_iter().skip(1).collect();
    want.extend(want.clone());
    assert_eq!(got, want);
}
