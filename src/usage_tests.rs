use super::*;
use crate::history::SavedRow;

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

fn have(db: &History, name: &str) -> u64 {
    db.log_ends()
        .into_iter()
        .find(|(n, _)| n == name)
        .map_or(0, |(_, e)| e)
}

fn add(db: &mut History, name: &str, msgs: &[&str]) {
    let have = have(db, name);
    let lines = kept(msgs, base() + have);
    let end = lines.last().map_or(0, |l| l.0);
    db.archive(name, have, &lines, end);
}

fn add_at(db: &mut History, name: &str, ts: u64, msgs: &[&str]) {
    let have = have(db, name);
    let lines: Vec<(u64, String)> = msgs.iter().map(|m| (ts, m.to_string())).collect();
    db.archive(name, have, &lines, ts);
}

const LOG: &str = "output_log_2026-10-23_10-00-00.txt";
const LATER: &str = "output_log_2026-10-23_12-00-00.txt";
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
0\tlog\th\t0
1\troom_enter\tEcliptica
1\tworld\twrld_4cf01f21-5c2a-4c7b-8a0a-0123456789ab\t98765
1\tplayer_join\tyou
0\tplayers\t1
1\tplayer_join\tP1
0\tplayers\t2
1\tplayer_join\tP2
0\tplayers\t3
1\townership\tAudioLink\tyou
1\townership\tBossSync\tP1
1\tstage\tGMBigcity\t0.5\tThaumaturge
1\tboss_fight\tHakli\t
1\tdealt\t25\t1
1\tdamage_taken\t12\t(Khepri) attack_Claws2
1\tdamage_taken\t3\tP1's Fireball
1\tdamage_taken\t4\tP2
1\tdamage_taken\t2\t
1\tenemy_spawn\t3\t17
1\tenemy_name\tKodama
1\tenemy_retire\t3
1\ttoken_spawn\t1\t40
1\tboss_dead\tHakli
1\tstrike_total\t5239
1\tnon_strike_total\t120
1\tplayer_left\tP1
0\tplayers\t2
1\tdeath
0\tplayer_dead
1\tintermission
1\tstage_end\tGMBigcity\t1\t0\t1\t0\t0\t0\t0\t0
0\tfight_end\tHakli\t1\tkilled\t1\t10\t25\t21\t4\t12\t0\t5239\t120
0\tfight_attack\tHakli\tKhepri\tClaws 2\t12\t1
0\tfight_attack\tHakli\tenemy\tP1's Fireball\t3\t1
0\tfight_attack\tHakli\tenemy\tP2\t4\t1
0\tfight_attack\tHakli\tunknown\tno source\t2\t1
0\trun_end\tlobby\t17\t1\t1\t1\t1\t1\tThaumaturge\tGMBigcity\t25\t21\t4\t12
0\tlobby
1\tsession_save\t4242
1\troom_left
";

fn fresh() -> History {
    let mut db = History::in_memory().unwrap();
    db.usage_start();
    db
}

fn masked(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let mut f: Vec<&str> = line.split('\t').collect();
        if f.get(1) == Some(&"log") {
            assert_eq!(f[2].len(), 16, "{line}");
            assert!(f[2].bytes().all(|b| b.is_ascii_hexdigit()), "{line}");
            f[2] = "h";
        }
        out.push_str(&f.join("\t"));
        out.push('\n');
    }
    out
}

fn body(text: &str) -> String {
    masked(text.split_once('\n').map_or("", |(_, rest)| rest))
}

fn tails(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.split_once('\t').unwrap().1.to_string())
        .collect()
}

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
    assert_eq!(batch.cursors, vec![(LOG.to_string(), SESSION.len() as u64)]);
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
    assert_eq!(text.matches("\tdeath\n").count(), 1);
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

const RUN: &[&str] = &[
    ENTER,
    "ECLIPTICA - now in stage: Stage_GMBigcity on phase: 0.5 as class: Thaumaturge",
    "ECLIPTICA - now fighting boss: Hakli(Clone) on phase: 0.25",
    "Dealing 40 STRIKE damage",
];

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
        db.archive(name, have - skew, &rows[have as usize..], base() + 3);
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
    db.archive(LOG, have - skew, &rows[have as usize..], base() + 3);
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
    db.archive(LOG, have - skew, &rows[have as usize..], base() + 5);
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
    let later = base() + SESSION.len() as u64 + 1;
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
