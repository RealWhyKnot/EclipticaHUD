use super::*;
use crate::history::SavedRow;

const ID: &str = "0123456789abcdef0123456789abcdef";

fn base() -> u64 {
    crate::game::event::split_line("2026.10.23 10:00:00 Log        -  x")
        .unwrap()
        .ts
}

fn kept(msgs: &[&str], from: u64) -> Vec<(u64, String)> {
    msgs.iter()
        .enumerate()
        .map(|(i, m)| (from + i as u64, m.to_string()))
        .collect()
}

fn add(db: &mut History, name: &str, msgs: &[&str]) {
    let have = db
        .log_ends()
        .into_iter()
        .find(|(n, _)| n == name)
        .map_or(0, |(_, e)| e);
    let lines = kept(msgs, base() + have);
    let end = lines.last().map_or(0, |l| l.0);
    db.archive(name, have, &lines, end);
}

const ENTER: &str = "[Behaviour] Entering Room: Ecliptica";

const SESSION: &[&str] = &[
    "User Authenticated: WhyKnot (usr_11111111-2222-3333-4444-555555555555)",
    ENTER,
    "[Behaviour] Joining wrld_4cf01f21-5c2a-4c7b-8a0a-0123456789ab:98765~private(usr_abc)~region(us)",
    "[Behaviour] OnPlayerJoined WhyKnot (usr_11111111-2222-3333-4444-555555555555)",
    "[Behaviour] OnPlayerJoined Lena (usr_aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee)",
    "[Behaviour] OnPlayerJoined Al (usr_ffffffff-bbbb-cccc-dddd-eeeeeeeeeeee)",
    "ownership of AudioLink transferred to WhyKnot",
    "ownership of BossSync transferred to Lena",
    "ECLIPTICA - now in stage: Stage_GMBigcity on phase: 0.5 as class: Thaumaturge",
    "ECLIPTICA - now fighting boss: Hakli(Clone) (UnityEngine.GameObject)",
    "Dealing 25 STRIKE damage",
    "damage has been taken: 12, from source: (Khepri) attack_Claws2",
    "damage has been taken: 3, from source: Lena's Fireball",
    "damage has been taken: 4, from source: Al",
    "damage has been taken: 2, from source: ",
    "Initializing Enemy POOL ID3 as ENEMY ID 17",
    "[Behaviour] No targets to encode on Kodama(Clone)",
    "Retiring Enemy POOL ID3",
    "spawn token, True, 40",
    "Boss Hakli dead, personal damage dealt: ",
    "STRIKE DMG: 5239",
    "NON-STRIKE DMG: 120",
    "[Behaviour] OnPlayerLeft Lena (usr_aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee)",
    "Local controller dead, switching off.",
    "ECLIPTICA - now in intermission",
    "ECLIPTICA - now in lobby",
    "ECLIPTICA saving SESSION ID 4242",
    "[Behaviour] OnLeftRoom",
];

const WANT: &str = "\
0\troom_enter\tEcliptica
1\tworld\twrld_4cf01f21-5c2a-4c7b-8a0a-0123456789ab
1\tplayers\t1
1\tplayers\t2
1\tplayers\t3
1\townership\tAudioLink\tyou
1\townership\tBossSync\tother
1\tstage\tGMBigcity\t0.5\tThaumaturge
1\tboss_fight\tHakli
1\tdealt\t25\t1
1\tdamage_taken\t12\t(Khepri) attack_Claws2
1\tdamage_taken\t3\tplayer's Fireball
1\tdamage_taken\t4\tplayer
1\tdamage_taken\t2\t
1\tenemy_spawn\t3\t17
1\tenemy_name\tKodama
1\tenemy_retire\t3
1\ttoken_spawn\t1\t40
1\tboss_dead\tHakli
1\tstrike_total\t5239
1\tnon_strike_total\t120
1\tplayers\t2
1\tplayer_dead
1\tintermission
1\tlobby
1\tsession_save
1\troom_left
";

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
        "[Behaviour] OnLeftRoom",
    ];
    msgs.extend_from_slice(SESSION);
    msgs.extend_from_slice(&[
        "[Behaviour] Entering Room: Home",
        "[Behaviour] Joining wrld_22222222-5c2a-4c7b-8a0a-0123456789ab:2~public",
        "[Behaviour] OnPlayerJoined WhyKnot (usr_11111111-2222-3333-4444-555555555555)",
    ]);
    add(&mut db, "output_log_2026-10-23_10-00-00.txt", &msgs);
    let batch = build(&db, "1", ID.into(), MAX_TEXT).unwrap();
    let text = body(&batch.text);
    let want = format!(
        "0\troom_enter\tother\n7{}1\troom_enter\tother\n",
        &WANT[1..]
    );
    assert_eq!(text, want);
}

fn fresh() -> History {
    let mut db = History::in_memory().unwrap();
    db.usage_start();
    db
}

fn body(text: &str) -> &str {
    text.split_once('\n').map_or("", |(_, rest)| rest)
}

#[test]
fn scrub_drops_names_and_ids() {
    let scrub = Scrub::new(["Lena", "Al", "  ", "WhyKnot"]);
    assert_eq!(scrub.clean("Lena's Fireball"), "player's Fireball");
    assert_eq!(scrub.clean("WhyKnotLena"), "playerplayer");
    assert_eq!(scrub.clean("Al"), "player");
    assert_eq!(scrub.clean("Altar"), "Altar");
    assert_eq!(scrub.clean("hit by usr_abc-123 twice"), "hit by usr twice");
    assert_eq!(scrub.clean("a\tb\nc\u{1b}"), "a b c");
    assert_eq!(scrub.clean(&"x".repeat(500)).len(), MAX_STR);
    assert_eq!(scrub.clean(&"気".repeat(500)).chars().count(), MAX_STR);
}

#[test]
fn nothing_before_opt_in_is_sent() {
    let mut db = History::in_memory().unwrap();
    add(&mut db, "output_log_2026-10-01_10-00-00.txt", SESSION);
    db.save_party(&party(base() + 5, &[("Lena", false, 10, 2)]));
    assert!(build(&db, "1.0", ID.into(), MAX_TEXT).is_none());
    db.usage_start();
    assert!(build(&db, "1.0", ID.into(), MAX_TEXT).is_none());
    add(
        &mut db,
        "output_log_2026-10-01_10-00-00.txt",
        &[ENTER, "Dealing 7 STRIKE damage"],
    );
    let batch = build(&db, "1.0", ID.into(), MAX_TEXT).unwrap();
    assert_eq!(
        body(&batch.text),
        "0\troom_enter\tEcliptica\n1\tdealt\t7\t1\n"
    );
}

#[test]
fn a_session_becomes_clean_lines() {
    let mut db = fresh();
    add(&mut db, "output_log_2026-10-23_10-00-00.txt", SESSION);
    let batch = build(&db, "1.2.3", ID.into(), MAX_TEXT).unwrap();
    let (head, rest) = batch.text.split_once('\n').unwrap();
    assert_eq!(head, format!("v1\t1.2.3\t{ID}\t{}", day(base())));
    assert_eq!(rest, WANT);
    for word in ["WhyKnot", "Lena", "usr_", "private", "98765"] {
        assert!(!rest.contains(word), "{word}");
    }
    assert_eq!(
        batch.cursors,
        vec![(
            "output_log_2026-10-23_10-00-00.txt".to_string(),
            SESSION.len() as u64
        )]
    );
}

#[test]
fn committed_lines_are_not_sent_again() {
    let mut db = fresh();
    let log = "output_log_2026-10-23_10-00-00.txt";
    add(&mut db, log, SESSION);
    let first = build(&db, "1", ID.into(), MAX_TEXT).unwrap();
    assert!(db.usage_commit(&first.cursors));
    assert!(build(&db, "1", ID.into(), MAX_TEXT).is_none());
    add(
        &mut db,
        log,
        &[
            ENTER,
            "Dealing 9 NON-STRIKE damage",
            "damage has been taken: 5, from source: Lena",
        ],
    );
    let next = build(&db, "1", ID.into(), MAX_TEXT).unwrap();
    assert_eq!(
        body(&next.text),
        "0\troom_enter\tEcliptica\n1\tdealt\t9\t0\n1\tdamage_taken\t5\tplayer\n"
    );
}

#[test]
fn old_logs_found_later_are_skipped_and_new_ones_sent() {
    let mut db = History::in_memory().unwrap();
    add(
        &mut db,
        "output_log_2026-10-10_10-00-00.txt",
        &[ENTER, "Dealing 1 STRIKE damage"],
    );
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
    let batch = build(&db, "1", ID.into(), MAX_TEXT).unwrap();
    assert_eq!(
        body(&batch.text),
        "0\troom_enter\tEcliptica\n1\tdealt\t3\t1\n"
    );
    assert!(db.usage_commit(&batch.cursors));
    assert!(build(&db, "1", ID.into(), MAX_TEXT).is_none());
}

#[test]
fn a_small_limit_splits_without_losing_lines() {
    let mut db = fresh();
    add(&mut db, "output_log_2026-10-23_10-00-00.txt", SESSION);
    add(&mut db, "output_log_2026-10-23_12-00-00.txt", SESSION);
    let mut got = String::new();
    let mut rounds = 0;
    while let Some(batch) = build(&db, "1", ID.into(), 64) {
        got.push_str(body(&batch.text));
        assert!(db.usage_commit(&batch.cursors));
        rounds += 1;
        assert!(rounds < 100);
    }
    assert!(rounds > 4);
    let lines = |s: &str| {
        s.lines()
            .map(|l| l.split_once('\t').unwrap().1.to_string())
            .collect::<Vec<_>>()
    };
    let mut want = lines(WANT);
    want.extend(lines(WANT));
    assert_eq!(lines(&got), want);
}

fn party(at: u64, rows: &[(&str, bool, u64, u64)]) -> PartyRecord {
    PartyRecord {
        at,
        boss: "Hakli".into(),
        progress: 0.3,
        secs: 194,
        down: true,
        here: 7,
        rows: rows
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
    let batch = build(&db, "1", ID.into(), MAX_TEXT).unwrap();
    assert_eq!(
        body(&batch.text),
        "0\tparty\tHakli\t0.3\t194\t1\t7\t1284310:4120:0,902115:18904:1\n"
    );
    assert_eq!(batch.cursors, vec![(String::new(), base() + 500)]);
    assert!(db.usage_commit(&batch.cursors));
    assert!(build(&db, "1", ID.into(), MAX_TEXT).is_none());
    let many: Vec<(&str, bool, u64, u64)> = (0..100).map(|_| ("x", false, 1, 1)).collect();
    db.save_party(&party(base() + 600, &many));
    let batch = build(&db, "1", ID.into(), MAX_TEXT).unwrap();
    assert_eq!(body(&batch.text).matches(':').count(), MAX_ROWS * 2);
}

#[test]
fn floats_and_worlds_are_strict() {
    assert_eq!(float(0.5).as_deref(), Some("0.5"));
    assert_eq!(float(1.0).as_deref(), Some("1"));
    assert_eq!(float(0.1 + 0.2).as_deref(), Some("0.3"));
    assert_eq!(float(-1.0), None);
    assert_eq!(float(f32::NAN), None);
    assert_eq!(float(1e30), None);
    assert_eq!(world("wrld_abc-1:9~x").as_deref(), Some("wrld_abc-1"));
    assert_eq!(world("wrld_zzz"), None);
    assert_eq!(world("usr_abc"), None);
    assert_eq!(day(base()), "2026-10-23");
    assert_eq!(day(0), "0000-00-00");
    assert!(day(MIN_TS).starts_with("1001-"));
}

#[test]
fn bad_version_text_is_cleaned() {
    let mut db = fresh();
    add(
        &mut db,
        "output_log_2026-10-23_10-00-00.txt",
        &[ENTER, "ECLIPTICA - now in lobby"],
    );
    let batch = build(&db, "1 .2\t3", ID.into(), MAX_TEXT).unwrap();
    assert!(batch.text.starts_with(&format!("v1\t1.23\t{ID}\t")));
    let batch = build(&db, " ", ID.into(), MAX_TEXT).unwrap();
    assert!(batch.text.starts_with(&format!("v1\tdev\t{ID}\t")));
}

#[test]
fn pass_keeps_a_failed_batch_and_retries_it_as_is() {
    let mut db = fresh();
    let log = "output_log_2026-10-23_10-00-00.txt";
    add(&mut db, log, SESSION);
    let on = AtomicBool::new(true);
    let mut pending = None;
    let mut sent: Vec<Vec<u8>> = Vec::new();
    pass(&mut db, "1", &on, &mut pending, |b| {
        sent.push(b.to_vec());
        None
    });
    assert_eq!(sent.len(), 1);
    add(&mut db, log, &[ENTER, "Dealing 1 STRIKE damage"]);
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
        "0\troom_enter\tEcliptica\n1\tdealt\t1\t1\n"
    );
    assert!(build(&db, "1", ID.into(), MAX_TEXT).is_none());
}

#[test]
fn pass_drops_refused_batches_and_stops_when_off() {
    let mut db = fresh();
    let log = "output_log_2026-10-23_10-00-00.txt";
    add(&mut db, log, SESSION);
    let on = AtomicBool::new(true);
    let mut pending = None;
    pass(&mut db, "1", &on, &mut pending, |_| Some(400));
    assert!(pending.is_none());
    assert!(build(&db, "1", ID.into(), MAX_TEXT).is_none());
    add(&mut db, log, &[ENTER, "Dealing 1 STRIKE damage"]);
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
    assert!(build(&db, "1", ID.into(), MAX_TEXT).is_some());
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
    let noisy: Vec<String> = (0..60_000)
        .map(|i| {
            format!(
                "damage has been taken: {}, from source: {:x}",
                i * 7919 % 100_003,
                i * 2_654_435_761u64
            )
        })
        .collect();
    let refs: Vec<&str> = noisy.iter().map(String::as_str).collect();
    add(&mut db, "output_log_2026-10-23_10-00-00.txt", &refs);
    let (batch, body) = next(&db, "1", ID.into()).unwrap();
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
