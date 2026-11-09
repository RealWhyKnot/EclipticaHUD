use super::*;

#[test]
fn shared_fights_go_last_without_names() {
    let mut db = fresh();
    db.save_party(&party(
        base(),
        &[
            ("Lena", false, 1_284_310, 4120),
            ("WhyKnot", true, 902_115, 18_904),
        ],
    ));
    db.save_party(&party(base() + 500, &[]));
    let batch = build(&db, "1", MAX_TEXT).unwrap();
    assert_eq!(
        body(&batch.text),
        "0\tparty\t1\t1792749600\tHakli\t0.3\t194\t1\t7\t1284310:4120:P1,902115:18904:you\n"
    );
    assert_eq!(
        batch.cursors,
        vec![("#party".to_string(), 2), (String::new(), base() + 500)]
    );
    assert!(db.usage_commit(&batch.cursors));
    assert!(build(&db, "1", MAX_TEXT).is_none());
    db.save_party(&party(base(), &[("Lena", false, 1_300_000, 4200)]));
    let batch = build(&db, "1", MAX_TEXT).unwrap();
    assert_eq!(
        body(&batch.text),
        "0\tparty\t3\t1792749600\tHakli\t0.3\t194\t1\t7\t1300000:4200:P1\n"
    );
    assert!(db.usage_commit(&batch.cursors));
    let many: Vec<(&str, bool, u64, u64)> = (0..100).map(|_| ("x", false, 1, 1)).collect();
    db.save_party(&party(base() + 600, &many));
    let batch = build(&db, "1", MAX_TEXT).unwrap();
    assert_eq!(body(&batch.text).matches(':').count(), MAX_ROWS * 2);
}

#[test]
fn floats_worlds_and_times_are_strict() {
    assert_eq!(float(0.5).as_deref(), Some("0.5"));
    assert_eq!(float(1.0).as_deref(), Some("1"));
    assert_eq!(float(0.1 + 0.2).as_deref(), Some("0.3"));
    assert_eq!(float(-1.0), None);
    assert_eq!(float(f32::NAN), None);
    assert_eq!(float(1e30), None);
    assert_eq!(
        world("wrld_abc-1:9~x", "s").as_deref(),
        Some("wrld_abc-1\t9")
    );
    let custom = format!("wrld_abc-1\tx{}", hash(&["s", "LenaRoom"], 1));
    assert_eq!(world("wrld_abc-1:LenaRoom~x", "s"), Some(custom));
    assert_ne!(
        world("wrld_abc-1:LenaRoom", "t"),
        world("wrld_abc-1:LenaRoom", "s")
    );
    assert_eq!(world("wrld_abc-1", "s").as_deref(), Some("wrld_abc-1\t"));
    let odd = world("wrld_abc-1:a b~x", "s").unwrap();
    assert!(odd.starts_with("wrld_abc-1\tx") && !odd.contains(' '));
    let long = format!("wrld_abc-1:{}", "9".repeat(17));
    assert!(!world(&long, "s").unwrap().contains("99"));
    assert_eq!(world("wrld_zzz", "s"), None);
    assert_eq!(world("usr_abc", "s"), None);
    assert_eq!(hash(&["a", "bc"], 2).len(), 32);
    assert_ne!(hash(&["a", "bc"], 1), hash(&["ab", "c"], 1));
    assert_eq!(
        hash(&["abc"], 4),
        "ce91dc5eec0139adf091900d225971d6ad246a845bad791b5693a9d0d55dd391"
    );
    assert_eq!(day(base()), "2026-10-23");
    assert_eq!(day(0), "0000-00-00");
    assert!(day(MIN_TS).starts_with("1001-"));
    let epoch = crate::game::event::split_line("1970.01.01 00:00:00 Log        -  x").unwrap();
    assert_eq!(unix(epoch.ts), 0);
    assert_eq!(unix(base()), 1_792_749_600);
    assert!((-14 * 60..=14 * 60).contains(&utc_offset_at(base())));
    assert_eq!(utc_offset_at(0), 0);
}

#[test]
fn ids_are_stable_per_install_and_differ_between_installs() {
    let mut a = fresh();
    add(&mut a, LOG, SESSION);
    let mut b = fresh();
    add(&mut b, LOG, SESSION);
    let one = build(&a, "1", MAX_TEXT).unwrap().text;
    let two = build(&a, "1", MAX_TEXT).unwrap().text;
    let other = build(&b, "1", MAX_TEXT).unwrap().text;
    assert_eq!(one, two);
    assert_ne!(one.lines().next(), other.lines().next());
    assert_ne!(one.lines().nth(1), other.lines().nth(1));
    assert_eq!(body(&one), body(&other));
}

#[test]
fn bad_version_text_is_cleaned() {
    let mut db = fresh();
    add(&mut db, LOG, &[ENTER, "ECLIPTICA - now in lobby"]);
    let batch = build(&db, "1 .2\t3", MAX_TEXT).unwrap();
    assert!(batch.text.starts_with("v2\t1.23\t"));
    let batch = build(&db, " ", MAX_TEXT).unwrap();
    assert!(batch.text.starts_with("v2\tdev\t"));
}

#[test]
fn pass_keeps_a_failed_batch_and_retries_it_as_is() {
    let mut db = fresh();
    add(&mut db, LOG, SESSION);
    let on = AtomicBool::new(true);
    let mut pending = None;
    let mut sent: Vec<Vec<u8>> = Vec::new();
    pass(&mut db, "1", &on, &mut pending, |b| {
        sent.push(b.to_vec());
        None
    });
    assert_eq!(sent.len(), 1);
    add(&mut db, LOG, &[ENTER, "Dealing 1 STRIKE damage"]);
    pass(&mut db, "1", &on, &mut pending, |b| {
        sent.push(b.to_vec());
        Some(503)
    });
    assert_eq!(sent[0], sent[1]);
    pass(&mut db, "1", &on, &mut pending, |b| {
        sent.push(b.to_vec());
        Some(204)
    });
    assert!(pending.is_none());
    assert_eq!(sent.len(), 4);
    let text = miniz_oxide::inflate::decompress_to_vec_zlib(&sent[3]).unwrap();
    assert_eq!(
        body(std::str::from_utf8(&text).unwrap()),
        format!(
            "0\tlog\th\t{}\n0\troom_enter\tEcliptica\n1\tdealt\t1\t1\n",
            SESSION.len()
        )
    );
    assert!(build(&db, "1", MAX_TEXT).is_none());
}

#[test]
fn pass_drops_refused_batches_and_stops_when_off() {
    let mut db = fresh();
    add(&mut db, LOG, SESSION);
    let on = AtomicBool::new(true);
    let mut pending = None;
    pass(&mut db, "1", &on, &mut pending, |_| Some(400));
    assert!(pending.is_none());
    assert!(build(&db, "1", MAX_TEXT).is_none());
    add(&mut db, LOG, &[ENTER, "Dealing 1 STRIKE damage"]);
    pass(&mut db, "1", &on, &mut pending, |_| None);
    assert!(pending.is_some());
    on.store(false, Ordering::Relaxed);
    let mut calls = 0;
    pass(&mut db, "1", &on, &mut pending, |_| {
        calls += 1;
        Some(204)
    });
    assert_eq!(calls, 0);
    assert!(pending.is_none());
    assert!(build(&db, "1", MAX_TEXT).is_some());
}

#[test]
fn outcome_sorts_statuses() {
    assert_eq!(outcome(Some(204)), Sent::Kept);
    assert_eq!(outcome(Some(200)), Sent::Kept);
    assert_eq!(outcome(Some(400)), Sent::Dropped);
    assert_eq!(outcome(Some(413)), Sent::Dropped);
    for s in [None, Some(404), Some(429), Some(500), Some(503), Some(507)] {
        assert_eq!(outcome(s), Sent::Retry, "{s:?}");
    }
}

#[test]
fn big_batches_shrink_to_fit_the_body_cap() {
    let mut db = fresh();
    let mut noisy = vec![ENTER.to_string()];
    noisy.extend((0..60_000u64).map(|i| {
        format!(
            "damage has been taken: {}, from source: {:x}",
            i * 7919 % 100_003,
            i * 2_654_435_761u64
        )
    }));
    let refs: Vec<&str> = noisy.iter().map(String::as_str).collect();
    add(&mut db, LOG, &refs);
    let (batch, body) = next(&db, "1").unwrap();
    assert!(body.len() <= MAX_BODY, "{}", body.len());
    assert!(batch.text.len() < MAX_TEXT);
}

#[test]
fn usage_url_swaps_the_path() {
    let url = crate::relay::usage_url("wss://ecliptica.whyknot.dev/ws").unwrap();
    assert!(url.secure);
    assert_eq!(
        (url.host.as_str(), url.port, url.path.as_str()),
        ("ecliptica.whyknot.dev", 443, "/usage")
    );
    let url = crate::relay::usage_url("ws://127.0.0.1:8080/ws").unwrap();
    assert_eq!(
        (url.secure, url.port, url.path.as_str()),
        (false, 8080, "/usage")
    );
}
