use super::*;

fn lines(msgs: &[&str]) -> Vec<(u64, String)> {
    msgs.iter()
        .enumerate()
        .map(|(i, m)| (100 + i as u64, m.to_string()))
        .collect()
}

fn replayed(h: &History, name: &str) -> (Vec<(u64, String)>, u64) {
    let mut out = Vec::new();
    let end = h.replay(name, |ts, m| out.push((ts, m.to_string())));
    (out, end)
}

fn record(at: u64, names: &[(&str, bool, u64, u64)]) -> PartyRecord {
    PartyRecord {
        at,
        boss: "Kakarot".into(),
        progress: 0.25,
        secs: 95,
        down: true,
        here: 3,
        rows: names
            .iter()
            .map(|&(name, you, dealt, taken)| SavedRow {
                name: name.into(),
                you,
                dealt,
                taken,
            })
            .collect(),
    }
}

#[test]
fn archive_round_trip_keeps_order_and_end() {
    let mut h = History::in_memory().unwrap();
    let a = lines(&["Dealing 25 STRIKE damage", "Boss Kakarot dead"]);
    h.archive("output_log_a.txt", 0, &a, 150);
    assert_eq!(h.stored("output_log_a.txt"), 2);
    assert_eq!(replayed(&h, "output_log_a.txt"), (a, 150));
    assert_eq!(h.logs(), vec!["output_log_a.txt".to_string()]);
    assert_eq!(replayed(&h, "missing"), (Vec::new(), 0));
}

#[test]
fn archive_is_idempotent_and_appends() {
    let mut h = History::in_memory().unwrap();
    let all = lines(&["one", "two", "three", "four"]);
    h.archive("l", 0, &all[..2], 101);
    h.archive("l", 0, &all[..3], 102);
    h.archive("l", 3, &all[3..], 103);
    h.archive("l", 0, &all, 90);
    assert_eq!(h.stored("l"), 4);
    assert_eq!(replayed(&h, "l"), (all, 103));
}

#[test]
fn logs_list_sorted_by_name() {
    let mut h = History::in_memory().unwrap();
    for n in [
        "output_log_2026-10-23",
        "output_log_2026-10-21",
        "output_log_2026-10-22",
    ] {
        h.archive(n, 0, &lines(&["x"]), 1);
    }
    assert_eq!(
        h.logs(),
        vec![
            "output_log_2026-10-21".to_string(),
            "output_log_2026-10-22".to_string(),
            "output_log_2026-10-23".to_string(),
        ]
    );
}

#[test]
fn odd_text_survives() {
    let mut h = History::in_memory().unwrap();
    let odd = "ownership of Boss transferred to Be\u{430}r\tHands\n'; DROP TABLE lines; --";
    let a = vec![(5, odd.to_string())];
    h.archive("it's \"quoted\"", 0, &a, 5);
    assert_eq!(replayed(&h, "it's \"quoted\"").0, a);
    let rec = record(7, &[(odd, false, 1, 2), ("me", true, 3, 4)]);
    h.save_party(&rec);
    assert_eq!(h.party_page(0, 0).unwrap().rows[1].name, odd);
}

#[test]
fn party_upsert_replaces_rows() {
    let mut h = History::in_memory().unwrap();
    h.save_party(&record(
        10,
        &[("a", true, 5, 1), ("b", false, 4, 2), ("c", false, 1, 0)],
    ));
    let later = record(10, &[("a", true, 9, 1), ("b", false, 12, 3)]);
    h.save_party(&later);
    assert_eq!(h.party_count(0), 1);
    let got = h.party_page(0, 0).unwrap();
    assert_eq!(got.rows.len(), 2);
    assert_eq!(got.rows[0].name, "b");
    assert_eq!(got.rows[0].dealt, 12);
    assert_eq!((got.at, got.secs, got.here, got.down), (10, 95, 3, true));
    assert!((got.progress - 0.25).abs() < 1e-6);
}

#[test]
fn party_pages_newest_first_and_skip_current() {
    let mut h = History::in_memory().unwrap();
    for at in [30, 10, 20, 40] {
        h.save_party(&record(at, &[("a", true, at, 0), ("b", false, 1, 0)]));
    }
    assert_eq!(h.party_count(0), 4);
    let ats: Vec<u64> = (0..4).map(|i| h.party_page(0, i).unwrap().at).collect();
    assert_eq!(ats, vec![40, 30, 20, 10]);
    assert_eq!(h.party_count(40), 3);
    assert_eq!(h.party_page(40, 0).unwrap().at, 30);
    assert_eq!(h.party_page(40, 3), None);
}

#[test]
fn file_store_reopens() {
    let dir = std::env::temp_dir().join(format!("ehud-hist-{}", std::process::id()));
    let path = dir.join("history.db");
    {
        let mut h = History::open(&path).unwrap();
        h.archive("l", 0, &lines(&["kept"]), 9);
        h.save_party(&record(1, &[("a", true, 1, 1), ("b", false, 2, 2)]));
    }
    let h = History::open(&path).unwrap();
    assert_eq!(replayed(&h, "l"), (lines(&["kept"]), 9));
    assert_eq!(h.party_count(0), 1);
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

fn rows(items: &[(u64, &str)]) -> Vec<(u64, String)> {
    items.iter().map(|&(t, m)| (t, m.to_string())).collect()
}

#[test]
fn old_archives_line_up_with_new_kept_lines() {
    let mut h = History::in_memory().unwrap();
    h.archive("l", 0, &rows(&[(100, "a"), (101, "b"), (102, "c")]), 102);
    h.forget_skew();
    let kept = rows(&[
        (100, "a"),
        (100, "w1"),
        (101, "b"),
        (101, "w2"),
        (102, "c"),
        (102, "w3"),
        (103, "d"),
    ]);
    assert_eq!(h.resume("l", &kept), (5, 2));
    h.archive("l", 3, &kept[5..], 103);
    assert_eq!(h.resume("l", &kept), (7, 2));
    assert_eq!(
        replayed(&h, "l").0,
        rows(&[(100, "a"), (101, "b"), (102, "c"), (102, "w3"), (103, "d")])
    );
    h.archive("m", 0, &kept, 103);
    assert_eq!(h.resume("m", &kept), (7, 0));
    assert_eq!(h.resume("new", &kept), (0, 0));
    h.archive("z", 0, &rows(&[(100, "q")]), 100);
    h.forget_skew();
    let moved = rows(&[(100, "x"), (100, "y"), (101, "z")]);
    assert_eq!(h.resume("z", &moved), (2, 1));
    assert_eq!(h.resume("m", &kept), (7, 0));
}

#[test]
fn upgrading_keeps_upload_cursors_in_place() {
    let dir = std::env::temp_dir().join(format!("ehud-v1-{}", std::process::id()));
    let path = dir.join("history.db");
    {
        let mut h = History::open(&path).unwrap();
        h.archive("a", 0, &lines(&["x", "y"]), 99);
        h.archive("b", 0, &lines(&["x", "y", "z"]), 9);
        h.archive("c", 0, &lines(&["x"]), 9);
        h.save_party(&record(50, &[("a", true, 1, 1)]));
        h.save_party(&record(60, &[("a", true, 1, 1)]));
        h.save_party(&record(70, &[("a", true, 1, 1)]));
        h.db.execute_batch(
            "DELETE FROM party_saves; DELETE FROM sqlite_sequence;
             INSERT INTO usage_cursor (name, n) VALUES ('', 60), ('a', 2), ('b', 1), ('c', 1);
             ALTER TABLE logs DROP COLUMN skew; DROP TABLE usage_floor; DELETE FROM meta;
             PRAGMA user_version = 1;",
        )
        .unwrap();
    }
    let mut h = History::open(&path).unwrap();
    let mut got: Vec<(String, u64)> = h.usage_cursors().into_iter().collect();
    got.sort();
    let want = [("", 60), ("#party", 2), ("a", 3), ("b", 1), ("c", 1)];
    assert_eq!(got, want.map(|(n, c)| (n.to_string(), c)).to_vec());
    assert_eq!(h.party_saved_after(2, 10).len(), 1);
    let mut floors: Vec<(String, u64)> = h.usage_floors().into_iter().collect();
    floors.sort();
    assert_eq!(floors, got[2..].to_vec());
    let floor: String =
        h.db.query_row(
            "SELECT value FROM meta WHERE key = 'party_floor'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(floor, "60");
    let wider = rows(&[(100, "x"), (100, "new"), (101, "y")]);
    assert_eq!(h.resume("a", &wider), (3, 1));
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_older_build_opening_the_file_leaves_it_working() {
    let dir = std::env::temp_dir().join(format!("ehud-back-{}", std::process::id()));
    let path = dir.join("history.db");
    {
        let mut h = History::open(&path).unwrap();
        h.archive("a", 0, &lines(&["x", "y"]), 40);
        h.save_party(&record(50, &[("a", true, 1, 1)]));
        h.usage_start();
        h.save_party(&record(60, &[("a", true, 1, 1)]));
        h.db.execute_batch(
            "UPDATE usage_cursor SET n = 60 WHERE name = '';
             INSERT INTO party_fights (at, boss, progress, secs, down, here)
                 VALUES (70, 'Kakarot', 0.5, 9, 0, 3);
             UPDATE logs SET skew = 5;
             PRAGMA user_version = 1;",
        )
        .unwrap();
    }
    let h = History::open(&path).unwrap();
    let count = |sql: &str| h.db.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
    assert_eq!(count("PRAGMA user_version"), 3);
    assert_eq!(count("SELECT COUNT(*) FROM party_saves"), 3);
    assert_eq!(count("SELECT COUNT(*) FROM logs WHERE skew IS NOT NULL"), 0);
    let cursor = h.usage_cursors()["#party"];
    assert_eq!(cursor, 1);
    let ats: Vec<u64> = h
        .party_saved_after(cursor, 10)
        .into_iter()
        .map(|(_, r)| r.at)
        .collect();
    assert_eq!(ats, vec![70]);
    assert_eq!(replayed(&h, "a").0, lines(&["x", "y"]));
    drop(h);
    let h = History::open(&path).unwrap();
    assert_eq!(h.usage_cursors()["#party"], 1);
    assert_eq!(h.usage_cursors()[""], 60);
    assert_eq!(h.party_saved_after(1, 10).len(), 1);
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opting_in_on_an_older_build_still_keeps_earlier_data_out() {
    let dir = std::env::temp_dir().join(format!("ehud-v1-in-{}", std::process::id()));
    let path = dir.join("history.db");
    {
        let mut h = History::open(&path).unwrap();
        h.archive("a", 0, &lines(&["x", "y"]), 101);
        h.archive("b", 0, &lines(&["x"]), 100);
        h.save_party(&record(50, &[("a", true, 1, 1)]));
        h.db.execute_batch(
            "INSERT INTO usage_cursor (name, n) VALUES ('', 50), ('a', 2), ('b', 1);
             PRAGMA user_version = 1;",
        )
        .unwrap();
    }
    let h = History::open(&path).unwrap();
    let mut got: Vec<(String, u64)> = h.usage_cursors().into_iter().collect();
    got.sort();
    let want = [("", 50), ("#party", 1), ("a", 3), ("b", 1)];
    assert_eq!(got, want.map(|(n, c)| (n.to_string(), c)).to_vec());
    let mut floors: Vec<(String, u64)> = h.usage_floors().into_iter().collect();
    floors.sort();
    assert_eq!(floors, got[2..].to_vec());
    assert!(h.party_saved_after(0, 10).is_empty());
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

fn wal_size(path: &Path) -> u64 {
    let mut wal = path.as_os_str().to_owned();
    wal.push("-wal");
    std::fs::metadata(wal).map_or(0, |m| m.len())
}

fn fill(h: &mut History, path: &Path, name: &str, batches: u64) -> u64 {
    let mut biggest = 0;
    for b in 0..batches {
        let kept: Vec<(u64, String)> = (0..400)
            .map(|i| {
                let n = b * 400 + i;
                (n, format!("{name} line {n} {}", "x".repeat(200)))
            })
            .collect();
        h.archive(name, b * 400, &kept, b * 400 + 399);
        biggest = biggest.max(wal_size(path));
    }
    biggest
}

#[test]
fn wal_is_capped_and_truncated() {
    let dir = std::env::temp_dir().join(format!("ehud-wal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("history.db");
    let mut h = History::open(&path).unwrap();
    let limit: i64 =
        h.db.query_row("PRAGMA journal_size_limit", [], |r| r.get(0))
            .unwrap();
    assert_eq!(limit, WAL_LIMIT);
    let biggest = fill(&mut h, &path, "a", 60);
    assert!(biggest > WAL_LIMIT as u64 * 2, "wal only reached {biggest}");
    let grown = wal_size(&path);
    assert!(grown > 0);

    let other = History::open(&path).unwrap();
    let reading = other.db.unchecked_transaction().unwrap();
    let _: i64 = reading
        .query_row("SELECT COUNT(*) FROM lines", [], |r| r.get(0))
        .unwrap();
    fill(&mut h, &path, "b", 2);
    let started = std::time::Instant::now();
    assert!(!h.checkpoint());
    assert!(started.elapsed() < Duration::from_millis(500));
    drop(reading);

    let started = std::time::Instant::now();
    assert!(h.checkpoint());
    let took = started.elapsed();
    assert_eq!(wal_size(&path), 0);
    println!(
        "wal peaked at {biggest} bytes, {grown} before the checkpoint, truncate took {took:?}"
    );
    assert_eq!(h.stored("a"), 60 * 400);
    assert_eq!(h.stored("b"), 2 * 400);

    fill(&mut h, &path, "c", 3);
    assert!(wal_size(&path) > 0);
    drop(h);
    assert_eq!(wal_size(&path), 0);
    assert_eq!(other.stored("c"), 3 * 400);
    drop(other);
    let _ = std::fs::remove_dir_all(&dir);
}
