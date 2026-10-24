use super::*;

const KEY: &str = "aa00000000000000000000000000000000000000000000000000000000000001";

fn header() -> Header {
    Header {
        name: "Bear Hands".into(),
        party: 3,
        room: "0123456789abcdef0123456789abcdef".into(),
    }
}

fn lines(msgs: &[&str]) -> Vec<(u64, String)> {
    msgs.iter()
        .enumerate()
        .map(|(i, m)| (1_790_000_000 + i as u64, m.to_string()))
        .collect()
}

#[test]
fn streams_only_fight_lines() {
    for m in [
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0.25",
        "Dealing 120 STRIKE damage",
        "damage has been taken: 50, from source: attack_Spit",
        "Boss Nan dead, personal damage dealt: ",
        "STRIKE DMG: 5900",
        "NON-STRIKE DMG: 1180",
        "Initializing Enemy POOL ID0 as ENEMY ID 37",
        "Retiring Enemy POOL ID0",
        "Local controller dead, switching off.",
        "[Behaviour] OnLeftRoom",
        "VRCApplication: HandleApplicationQuit at 12",
    ] {
        assert!(streams(m), "{m}");
    }
    for m in [
        "[Behaviour] OnPlayerJoined Bear Hands (usr_1234)",
        "[Behaviour] Joining wrld_abc:123~private(usr_1)",
        "ownership of Thing transferred to Bear Hands",
        "[Behaviour] No targets to encode on Bear Hands",
        "User Authenticated: Bear Hands (usr_1)",
        "Removed player Bear Hands",
    ] {
        assert!(!streams(m), "{m}");
    }
}

#[test]
fn body_has_header_labels_and_lines() {
    let l = lines(&[
        "ECLIPTICA - now fighting boss: FlyLord(Clone) on phase: 0.25",
        "Dealing 5 STRIKE damage",
        "ECLIPTICA - now fighting boss: FlyLordPhase2(Clone) on phase: 0.25",
    ]);
    let b = body(KEY, "00112233aabbccdd", 7, 1_790_000_003_250, &header(), &l);
    let want = format!(
        "eboard 2\nkey {KEY}\nname Bear Hands\nstream 00112233aabbccdd\nseq 7\nnow 1790000003250\nparty 3\nroom 0123456789abcdef0123456789abcdef\nlabel FlyLord\tBeelzebub\n1790000000\tECLIPTICA - now fighting boss: FlyLord(Clone) on phase: 0.25\n1790000001\tDealing 5 STRIKE damage\n1790000002\tECLIPTICA - now fighting boss: FlyLordPhase2(Clone) on phase: 0.25\n"
    );
    assert_eq!(b, want);
}

#[test]
fn unknown_bosses_still_get_a_label() {
    assert_eq!(
        label("ECLIPTICA - now fighting boss: BrandNewThing(Clone) on phase: 1"),
        Some(("BrandNewThing".into(), "BrandNewThing".into()))
    );
    assert_eq!(label("Dealing 5 STRIKE damage"), None);
}

#[test]
fn oversized_text_is_trimmed() {
    let h = Header {
        name: "x".repeat(500),
        ..header()
    };
    let long = format!("Dealing {} STRIKE damage", "9".repeat(5000));
    let b = body(KEY, "00112233aabbccdd", 0, 1, &h, &[(1, clean(&long))]);
    let name = b.lines().nth(2).unwrap();
    assert_eq!(name.chars().count(), "name ".len() + MAX_TEXT);
    assert!(b.lines().all(|l| l.chars().count() <= MAX_MSG + 12));
    assert_eq!(
        clean("Dealing 5\u{7} STRIKE damage \r"),
        "Dealing 5  STRIKE damage"
    );
}

#[test]
fn old_lines_are_not_sent() {
    assert!(fresh(1000, 1060));
    assert!(!fresh(1000, 1061));
}

#[test]
fn chunks_once_a_second_with_rising_seq() {
    let mut o = Outbox::default();
    let t = Instant::now();
    o.push(lines(&["Dealing 1 STRIKE damage"]));
    o.flush(KEY, header, 1, t);
    o.push(lines(&["Dealing 2 STRIKE damage"]));
    o.flush(KEY, header, 2, t + Duration::from_millis(400));
    assert_eq!(o.queue.len(), 1);
    o.flush(KEY, header, 3, t + Duration::from_millis(1000));
    assert_eq!(o.queue.len(), 2);
    assert!(o.queue[0].contains("\nseq 0\n"));
    assert!(o.queue[1].contains("\nseq 1\n"));
    assert!(o.queue[1].contains("Dealing 2"));
    o.flush(KEY, header, 4, t + Duration::from_secs(5));
    assert_eq!(o.queue.len(), 2);
}

#[test]
fn big_bursts_split_into_several_chunks() {
    let mut o = Outbox::default();
    o.push((0..MAX_LINES as u64 + 5).map(|i| (i, "Dealing 1 STRIKE damage".to_string())));
    o.flush(KEY, header, 1, Instant::now());
    assert_eq!(o.queue.len(), 2);
    assert_eq!(
        o.queue[1].lines().filter(|l| l.contains("Dealing")).count(),
        5
    );
}

#[test]
fn wide_lines_split_by_size() {
    let mut o = Outbox::default();
    let wide = clean(&"Ω".repeat(MAX_MSG + 50));
    o.push((0..1000).map(|i| (i, wide.clone())));
    o.flush(KEY, header, 1, Instant::now());
    assert!(o.queue.len() >= 10);
    assert!(o.queue.iter().all(|b| b.len() <= 256 * 1024));
    assert_eq!(
        o.queue
            .iter()
            .map(|b| b.lines().filter(|l| l.contains('Ω')).count())
            .sum::<usize>(),
        1000
    );
}

#[test]
fn header_is_built_only_when_a_chunk_is_due() {
    let mut o = Outbox::default();
    let t = Instant::now();
    let mut built = 0;
    o.flush(
        KEY,
        || {
            built += 1;
            header()
        },
        1,
        t,
    );
    o.push(lines(&["Dealing 1 STRIKE damage"]));
    o.flush(
        KEY,
        || {
            built += 1;
            header()
        },
        1,
        t,
    );
    o.push(lines(&["Dealing 2 STRIKE damage"]));
    o.flush(
        KEY,
        || {
            built += 1;
            header()
        },
        2,
        t + Duration::from_millis(300),
    );
    assert_eq!(built, 1);
}

#[test]
fn no_name_means_nothing_is_sent() {
    let mut o = Outbox::default();
    o.push(lines(&["Dealing 1 STRIKE damage"]));
    let h = Header {
        name: String::new(),
        ..header()
    };
    o.flush(KEY, || h, 1, Instant::now());
    assert!(o.queue.is_empty());
    assert!(o.lines.is_empty());
}

#[test]
fn failed_sends_retry_the_same_chunk_with_backoff() {
    let mut o = Outbox::default();
    o.push(lines(&["Dealing 1 STRIKE damage"]));
    o.flush(KEY, header, 1, Instant::now());
    let Some(Job::Send(first)) = o.job(true) else {
        panic!("no job")
    };
    let waits: Vec<_> = (0..5)
        .map(|_| o.done(&Job::Send(first.clone()), Reply::Retry))
        .collect();
    assert_eq!(
        waits
            .iter()
            .map(|w| w.1.unwrap().as_secs())
            .collect::<Vec<_>>(),
        vec![1, 2, 4, 8, 8]
    );
    assert!(waits.iter().all(|w| w.0 == FAILING));
    let Some(Job::Send(again)) = o.job(true) else {
        panic!("no job")
    };
    assert_eq!(again, first);
    assert_eq!(o.done(&Job::Send(again), Reply::Done), (OK, None));
    assert!(o.job(true).is_none());
}

#[test]
fn taken_name_is_reported() {
    let mut o = Outbox::default();
    o.push(lines(&["Dealing 1 STRIKE damage"]));
    o.flush(KEY, header, 1, Instant::now());
    let job = o.job(true).unwrap();
    assert_eq!(o.done(&job, reply(Some(409))), (TAKEN, None));
    assert!(o.job(true).is_none());
}

#[test]
fn long_outage_starts_a_new_stream() {
    let mut o = Outbox::default();
    let first = o.stream.clone();
    let t = Instant::now();
    for i in 0..=MAX_QUEUE as u64 {
        o.push(lines(&["Dealing 1 STRIKE damage"]));
        o.flush(KEY, header, i, t + Duration::from_secs(i));
    }
    assert!(o.queue.is_empty());
    assert_ne!(o.stream, first);
    assert_eq!(o.seq, 0);
}

#[test]
fn forget_wins_and_clears_the_queue() {
    let mut o = Outbox::default();
    o.push(lines(&["Dealing 1 STRIKE damage"]));
    o.flush(KEY, header, 1, Instant::now());
    o.forget();
    assert!(matches!(o.job(false), Some(Job::Forget)));
    assert!(o.queue.is_empty());
    o.done(&Job::Forget, Reply::Done);
    assert!(o.job(false).is_none());
    assert_eq!(forget_body(KEY), format!("eboard 2\nkey {KEY}\nforget\n"));
}

#[test]
fn replies_map_to_outcomes() {
    assert_eq!(reply(Some(204)), Reply::Done);
    assert_eq!(reply(Some(409)), Reply::Taken);
    assert_eq!(reply(Some(400)), Reply::Refused);
    assert_eq!(reply(Some(413)), Reply::Refused);
    assert_eq!(reply(Some(503)), Reply::Retry);
    assert_eq!(reply(None), Reply::Retry);
}

#[test]
fn keys_and_stream_ids_are_random_hex() {
    let a = new_key();
    assert!(valid_key(&a));
    assert_ne!(a, new_key());
    let s = random_hex(8);
    assert_eq!(s.len(), 16);
    assert!(s.bytes().all(|b| b.is_ascii_hexdigit()));
}
