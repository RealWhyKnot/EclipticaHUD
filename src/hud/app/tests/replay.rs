use super::*;

#[test]
fn archived_logs_replay_after_vrchat_deletes_them() {
    let dir = temp_logs("archive");
    let log_a = dir.join("output_log_2026-01-01_00-00-00.txt");
    std::fs::write(&log_a, DEATHS).unwrap();
    let log_b = dir.join("output_log_2026-01-02_00-00-00.txt");
    std::fs::write(
        &log_b,
        format!(
            "2026.01.02 10:00:00 Debug      -  {STAGE_B}
"
        ),
    )
    .unwrap();
    let mut db = crate::history::History::in_memory().unwrap();
    let mut from_file = GameState::default();
    archive::backfill(&mut from_file, Some(&dir), Some(&mut db));
    assert!(
        db.stored("output_log_2026-01-01_00-00-00.txt")
            < crate::num::to_u64(DEATHS.lines().count())
    );
    std::fs::remove_file(&log_a).unwrap();
    let mut from_db = GameState::default();
    archive::backfill(&mut from_db, Some(&dir), Some(&mut db));
    assert_eq!(snapshot(&from_db), snapshot(&from_file));
    assert_eq!(from_file.runs[0].deaths, 0);
    assert!(from_file.runs[0].end_ts.is_some());
    archive::backfill(&mut GameState::default(), Some(&dir), Some(&mut db));
    assert_eq!(db.logs().len(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn archives_from_older_builds_line_up_after_an_upgrade() {
    let dir = temp_logs("upgrade");
    let world = |t: &str, m: &str| format!("2026.01.01 10:00:{t} Debug      -  {m}\n");
    let mut text = String::new();
    for (i, line) in DEATHS.lines().enumerate() {
        text.push_str(line);
        text.push('\n');
        if i == 1 {
            text.push_str(&world("01", "ECLIPTICA - some new message"));
        }
        if i == 4 {
            text.push_str(&world("08", "Removed player Bob"));
        }
    }
    let closed = "output_log_2026-01-01_00-00-00.txt";
    let live = "output_log_2026-01-02_00-00-00.txt";
    std::fs::write(dir.join(closed), &text).unwrap();
    std::fs::write(dir.join(live), &text).unwrap();
    let old = kept_of(&text, true);
    assert_eq!(kept_of(&text, false).len(), old.len() + 2);
    let mut db = crate::history::History::in_memory().unwrap();
    for name in [closed, live] {
        db.archive(name, 0, &old, old.last().unwrap().0);
    }
    db.forget_skew();
    let mut app = App::new(96);
    app.set_history(Some(db));
    archive::backfill(&mut app.gs, Some(&dir), app.archive.db.as_mut());
    assert_eq!(
        app.archive.db.as_ref().unwrap().stored(closed),
        crate::num::to_u64(old.len())
    );
    app.watch = LogWatch::for_dir(dir.clone());
    app.tick();
    let tail = format!(
        "{}{}",
        world("40", "Dealing 5 STRIKE damage"),
        world("41", "ECLIPTICA - another new message")
    );
    std::fs::OpenOptions::new()
        .append(true)
        .open(dir.join(live))
        .unwrap()
        .write_all(tail.as_bytes())
        .unwrap();
    app.tick();
    let db = app.archive.db.as_mut().unwrap();
    let mut got = Vec::new();
    db.replay(closed, |ts, m| got.push((ts, m.to_string())));
    assert_eq!(got, old);
    let mut got = Vec::new();
    db.replay(live, |ts, m| got.push((ts, m.to_string())));
    let mut want = old.clone();
    want.extend(kept_of(&tail, false));
    assert_eq!(got.len(), old.len() + 2);
    assert_eq!(got, want);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn lines_drained_from_the_old_log_are_archived_under_its_name() {
    let dir = temp_logs("drain_archive");
    let old = "output_log_2026-01-01_00-00-00.txt";
    let new = "output_log_2026-01-02_00-00-00.txt";
    let (head, tail) = DEATHS.split_at(DEATHS.find("2026.01.01 10:00:10").unwrap());
    std::fs::write(dir.join(old), head).unwrap();
    let mut app = App::new(96);
    app.set_history(crate::history::History::in_memory());
    app.watch = LogWatch::for_dir(dir.clone());
    app.tick();
    let before = app.archive.db.as_ref().unwrap().stored(old);
    std::fs::write(dir.join(new), "").unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(dir.join(old))
        .unwrap()
        .write_all(tail.as_bytes())
        .unwrap();
    app.tick();
    assert_eq!(app.watch.path.as_deref(), Some(dir.join(old).as_path()));
    app.tick();
    assert_eq!(app.watch.path.as_deref(), Some(dir.join(new).as_path()));
    let db = app.archive.db.as_ref().unwrap();
    assert!(before < 10);
    assert_eq!(db.stored(old), 10);
    assert_eq!(db.stored(new), 0);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn live_log_is_archived_as_it_is_read() {
    let dir = temp_logs("live_archive");
    let log_a = dir.join("output_log_2026-01-01_00-00-00.txt");
    let (head, tail) = DEATHS.split_at(DEATHS.find("2026.01.01 10:00:10").unwrap());
    std::fs::write(&log_a, head).unwrap();
    let mut app = App::new(96);
    app.set_history(crate::history::History::in_memory());
    app.watch = LogWatch::for_dir(dir.clone());
    app.tick();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&log_a)
        .unwrap()
        .write_all(tail.as_bytes())
        .unwrap();
    app.tick();
    let mut whole = GameState::default();
    for line in DEATHS.lines() {
        whole.feed(line);
    }
    whole.log_rotated();
    let db = app.archive.db.as_ref().unwrap();
    let mut replayed = GameState::default();
    let end = db.replay("output_log_2026-01-01_00-00-00.txt", |ts, m| {
        replayed.feed_msg(ts, m);
    });
    replayed.seen(end);
    replayed.log_rotated();
    assert_eq!(snapshot(&replayed), snapshot(&whole));
    let mut again = App::new(96);
    again.set_history(app.archive.db.take());
    again.watch = LogWatch::for_dir(dir.clone());
    again.tick();
    let n = again
        .archive
        .db
        .as_ref()
        .unwrap()
        .stored("output_log_2026-01-01_00-00-00.txt");
    let mut fresh = GameState::default();
    let kept = crate::num::to_u64(
        DEATHS
            .lines()
            .filter(|l| fresh.feed_kept(l).is_some_and(|(_, k)| k.is_some()))
            .count(),
    );
    assert_eq!(kept, 10);
    assert_eq!(n, kept);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
#[ignore = "needs real VRChat logs on this machine"]
fn real_logs_replay_the_same() {
    let dir = crate::vrchat::log::log_dir().unwrap();
    for path in crate::vrchat::log::all_logs(&dir) {
        let mut whole = GameState::default();
        let mut kept = Vec::new();
        let mut end = 0;
        let t = std::time::Instant::now();
        crate::vrchat::log::read_lines(&path, |line| {
            if let Some((ts, keep)) = whole.feed_kept(line) {
                end = end.max(ts);
                if let Some(at) = keep {
                    kept.push((ts, line[at..].to_string()));
                }
            }
        })
        .unwrap();
        let file_ms = t.elapsed().as_millis();
        whole.log_rotated();
        let file = std::env::temp_dir().join(format!("ehud-size-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&file);
        let mut db = crate::history::History::open(&file).unwrap();
        db.archive("x", 0, &kept, end);
        drop(db);
        let size = std::fs::metadata(&file).unwrap().len();
        let db = crate::history::History::open(&file).unwrap();
        let t = std::time::Instant::now();
        let mut replayed = GameState::default();
        let end = db.replay("x", |ts, m| {
            replayed.feed_msg(ts, m);
        });
        replayed.seen(end);
        replayed.log_rotated();
        let db_ms = t.elapsed().as_millis();
        let bytes: usize = kept.iter().map(|(_, m)| m.len() + 8).sum();
        crate::console::say(format_args!(
            "{} kept={} bytes={} db={} runs={} file_ms={} db_ms={}",
            path.file_name().unwrap().to_string_lossy(),
            kept.len(),
            bytes,
            size,
            whole.runs.len(),
            file_ms,
            db_ms
        ));
        assert_eq!(snapshot(&replayed), snapshot(&whole), "{}", path.display());
        drop(db);
        let _ = std::fs::remove_file(&file);
    }
}

#[test]
#[ignore = "timing run, started by hand"]
fn many_sessions_load_cost() {
    let dir = crate::vrchat::log::log_dir().unwrap();
    let path = dir.join("output_log_2026-10-22_13-44-11.txt");
    let file = std::env::temp_dir().join("ehud-load-cost.db");
    let fresh = !file.exists();
    let mut db = crate::history::History::open(&file).unwrap();
    let mut probe = GameState::default();
    let mut kept = Vec::new();
    let mut end = 0;
    crate::vrchat::log::read_lines(&path, |line| {
        if let Some((ts, keep)) = probe.feed_kept(line) {
            end = end.max(ts);
            if let Some(at) = keep {
                kept.push((ts, line[at..].to_string()));
            }
        }
    })
    .unwrap();
    let n: usize = std::env::var("EHUD_SESSIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(365);
    for i in (0..n).filter(|_| fresh) {
        let shift = crate::num::to_u64(i) * 86400;
        let moved: Vec<(u64, String)> = kept.iter().map(|(t, m)| (t + shift, m.clone())).collect();
        db.archive(&format!("output_log_{i:05}.txt"), 0, &moved, end + shift);
    }
    let t = std::time::Instant::now();
    let mut gs = GameState::default();
    archive::backfill(&mut gs, None, Some(&mut db));
    crate::console::say(format_args!(
        "sessions={n} runs={} fights={} load_ms={} pid={}",
        gs.runs.len(),
        gs.runs.iter().map(|r| r.fights.len()).sum::<usize>(),
        t.elapsed().as_millis(),
        std::process::id()
    ));
    std::thread::sleep(std::time::Duration::from_secs(4));
    drop(gs);
}

#[test]
fn leaderboard_switch_sits_over_the_taken_card() {
    let mut app = headless();
    let (x, y, w, h) = BOARD_HIT;
    let (bx, by) = (x + w / 2, y + BODY_DY + h / 2);
    assert!(!app.hit_enabled(Hit::Board));
    assert_ne!(app.enabled_hit(bx, by), Some(Hit::Board));
    assert!(app.hit_enabled(Hit::Info(Info::Taken(0))));
    app.activate(Hit::Settings);
    assert_eq!(app.enabled_hit(bx, by), Some(Hit::Board));
    for i in 0..3 {
        assert!(!app.hit_enabled(Hit::Info(Info::Taken(i))));
    }
    assert!(!app.hit_enabled(Hit::Info(Info::TakenGraph)));
    assert!(app.hit_enabled(Hit::Info(Info::Status)));
    assert!(!app.board_header().share_name);
    assert!(app.activate(Hit::Board));
    assert!(app.flags.has(Flag::Board));
    assert!(crate::board::ON.load(std::sync::atomic::Ordering::Relaxed));
    assert!(app.board_header().share_name);
    assert!(app.activate(Hit::Board));
    assert!(!app.flags.has(Flag::Board));
    assert!(!crate::board::ON.load(std::sync::atomic::Ordering::Relaxed));
    assert!(!app.board_header().share_name);
}

#[test]
fn deleting_board_kills_does_not_share_the_name() {
    let mut app = headless();
    app.delete_board();
    assert!(!app.board_header().share_name);
    app.set_board(true);
    app.delete_board();
    assert!(app.board_header().share_name);
}

#[test]
fn board_delete_needs_a_second_click() {
    let mut app = headless();
    let (x, y, w, h) = BOARD_DELETE_HIT;
    let (bx, by) = (x + w / 2, y + BODY_DY + h / 2);
    assert!(!app.hit_enabled(Hit::BoardDelete));
    app.activate(Hit::Settings);
    assert_eq!(app.enabled_hit(bx, by), Some(Hit::BoardDelete));
    app.activate(Hit::Board);
    app.activate(Hit::Board);
    assert!(!app.flags.has(Flag::BoardDeleting));
    assert!(app.activate(Hit::BoardDelete));
    assert!(!app.flags.has(Flag::BoardDeleting));
    assert!(app.board_armed.is_some());
    assert!(app.activate(Hit::BoardDelete));
    assert!(app.flags.has(Flag::BoardDeleting));
    assert!(app.board_armed.is_none());
    assert!(crate::board::DELETING.load(std::sync::atomic::Ordering::Relaxed));
    assert!(!app.hit_enabled(Hit::BoardDelete));
}
