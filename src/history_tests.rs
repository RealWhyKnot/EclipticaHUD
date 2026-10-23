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
