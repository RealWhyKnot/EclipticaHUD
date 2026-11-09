use super::*;

#[test]
fn backoff_doubles_to_sixty_with_bounded_jitter() {
    let mut b = Backoff::default();
    let bases: Vec<u64> = (0..8).map(|_| b.next_ms(0) / 1000).collect();
    assert_eq!(bases, [2, 4, 8, 16, 32, 60, 60, 60]);
    let mut b = Backoff::default();
    for want in [2000u64, 4000, 8000, 16000, 32000, 60000, 60000] {
        let got = b.next_ms(u64::MAX);
        assert!(got >= want && got <= want + want / 4, "{got} {want}");
    }
    let mut spread = std::collections::HashSet::new();
    for _ in 0..50 {
        let mut b = Backoff::default();
        let d = b.next_ms(rand());
        assert!((2000..=2500).contains(&d));
        spread.insert(d);
    }
    assert!(spread.len() > 1);
    let mut b = Backoff { fails: u32::MAX };
    assert_eq!(b.next_ms(0), 60_000);
    b.reset();
    assert_eq!(b.next_ms(0), 2000);
    b.after_session(Duration::from_secs(5));
    assert_eq!(b.next_ms(0), 4000);
    b.after_session(Duration::from_secs(STABLE_SECS));
    assert_eq!(b.next_ms(0), 2000);
}

#[test]
fn throttle_sends_a_change_in_taken_alone() {
    let mut t = Throttle::default();
    let a = Totals::from((50, 4));
    let b = Totals::from((50, 9));
    assert_eq!(t.due(0, Some(a)), Some(Msg::Dmg(a)));
    assert_eq!(t.wait_ms(500, Some(b)), 500);
    assert_eq!(t.due(1000, Some(b)), Some(Msg::Dmg(b)));
    let big = Totals::from((1, u64::MAX));
    assert_eq!(
        t.due(2000, Some(big)),
        Some(Msg::Dmg(Totals::from((1, MAX_DMG))))
    );
}

#[test]
fn throttle_sends_changes_at_most_once_a_second() {
    let mut t = Throttle::default();
    assert_eq!(t.due(0, None), None);
    assert_eq!(t.wait_ms(0, None), 1000);
    assert_eq!(t.wait_ms(0, Some(dealt(5))), 1);
    assert_eq!(t.due(0, Some(dealt(5))), Some(Msg::Dmg(dealt(5))));
    assert_eq!(t.due(10, Some(dealt(5))), None);
    assert_eq!(t.wait_ms(10, Some(dealt(5))), 1000);
    assert_eq!(t.due(300, Some(dealt(9))), None);
    assert_eq!(t.wait_ms(300, Some(dealt(9))), 700);
    assert_eq!(t.due(999, Some(dealt(12))), None);
    assert_eq!(t.due(1000, Some(dealt(12))), Some(Msg::Dmg(dealt(12))));
    assert_eq!(t.due(5000, Some(dealt(12))), None);
    assert_eq!(t.wait_ms(5000, None), 1);
    assert_eq!(t.due(5000, None), Some(Msg::End));
    assert_eq!(t.due(7000, None), None);
    assert_eq!(t.wait_ms(7000, None), 1000);
    assert_eq!(t.due(7000, Some(dealt(0))), Some(Msg::Dmg(dealt(0))));
    assert_eq!(t.due(7500, None), None);
    assert_eq!(t.wait_ms(7500, None), 500);
    assert_eq!(t.due(8000, None), Some(Msg::End));
    assert_eq!(
        t.due(9000, Some(dealt(u64::MAX))),
        Some(Msg::Dmg(dealt(MAX_DMG)))
    );
    assert_eq!(t.due(20000, Some(dealt(MAX_DMG + 7))), None);
    let mut sent = 0;
    for ms in (0..10_000).step_by(16) {
        if t.due(30_000 + ms, Some(dealt(ms))).is_some() {
            sent += 1;
        }
    }
    assert_eq!(sent, 10);
}

#[test]
fn key_file_is_made_once_and_read_back() {
    let dir = std::env::temp_dir().join(format!("ehud-key-{}", rand()));
    let path = dir.join("EclipticaHUD").join("relay.key");
    let made = load_or_make_key(Some(&path), || [7; 32]);
    assert_eq!(made, [7; 32]);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "07".repeat(32));
    let again = load_or_make_key(Some(&path), || panic!("key was regenerated"));
    assert_eq!(again, [7; 32]);
    std::fs::write(&path, "not a key").unwrap();
    assert_eq!(load_or_make_key(Some(&path), || [9; 32]), [9; 32]);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "09".repeat(32));
    assert_eq!(load_or_make_key(None, || [3; 32]), [3; 32]);
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(dir.join("EclipticaHUD")).unwrap();
    std::fs::remove_dir(&dir).unwrap();
}

#[test]
fn key_hex_must_be_exactly_sixty_four_hex_digits() {
    let k: [u8; 32] = std::array::from_fn(|i| crate::num::to_u8(i) * 7);
    let h = crate::relay::sha256::hex(&k);
    assert_eq!(key_from_hex(&h), Some(k));
    assert_eq!(
        key_from_hex(&format!("  {}\r\n", h.to_uppercase())),
        Some(k)
    );
    assert_eq!(key_from_hex(&h[..62]), None);
    assert_eq!(key_from_hex(&format!("{h}00")), None);
    assert_eq!(key_from_hex(&format!("+f{}", &h[2..])), None);
    assert_eq!(key_from_hex(&format!("zz{}", &h[2..])), None);
    assert_eq!(key_from_hex(&format!("\u{e9}{}", &h[2..])), None);
    assert_eq!(key_from_hex(""), None);
}

#[test]
fn url_parsing_and_env_override() {
    let u = Url::parse(DEFAULT_URL).unwrap();
    assert_eq!(
        u,
        Url {
            secure: true,
            host: "ecliptica.whyknot.dev".into(),
            port: 443,
            path: "/ws".into()
        }
    );
    let l = Url::parse("ws://127.0.0.1:9001/ws").unwrap();
    assert_eq!(
        (l.secure, l.host.as_str(), l.port),
        (false, "127.0.0.1", 9001)
    );
    assert_eq!(Url::parse("ws://localhost").unwrap().path, "/");
    assert_eq!(Url::parse("ws://localhost").unwrap().port, 80);
    assert_eq!(Url::parse("ws://[::1]:8080/ws").unwrap().host, "::1");
    assert_eq!(Url::parse("wss://[::1]/ws").unwrap().port, 443);
    for bad in [
        "",
        "http://x/ws",
        "https://ecliptica.whyknot.dev/ws",
        "ws://",
        "ws:///ws",
        "wss://host:0/ws",
        "wss://host:99999/ws",
        "wss://host:abc/ws",
        "wss://ho st/ws",
        "wss://host/w s",
        "ws://[::1/ws",
        "ws://[::1]x/ws",
        "wss://user@host/ws",
        "ws://evil.example/ws",
        "ws://10.0.0.1:9001/ws",
        "ws://localhost.evil.example/ws",
        "ws://[::2]/ws",
    ] {
        assert_eq!(Url::parse(bad), None, "{bad}");
    }
    for local in [
        "ws://localhost:9001/ws",
        "ws://LocalHost/ws",
        "ws://127.0.0.1/ws",
        "ws://127.8.0.1/ws",
        "ws://[::1]:9001/ws",
    ] {
        assert!(!Url::parse(local).unwrap().secure, "{local}");
    }
    assert!(Url::parse("wss://evil.example/ws").unwrap().secure);
    assert!(Url::parse("wss://10.0.0.1:9001/ws").unwrap().secure);
    assert_eq!(url_from(Some("ws://evil.example/ws")), DEFAULT_URL);
    assert_eq!(
        url_from(Some("wss://other.example/ws")),
        "wss://other.example/ws"
    );
    assert_eq!(url_from(None), DEFAULT_URL);
    assert_eq!(url_from(Some("ws://127.0.0.1:1/ws")), "ws://127.0.0.1:1/ws");
    assert_eq!(
        url_from(Some(" ws://127.0.0.1:1/ws\n")),
        "ws://127.0.0.1:1/ws"
    );
    assert_eq!(url_from(Some("http://evil/ws")), DEFAULT_URL);
}

#[test]
fn want_needs_toggle_world_location_and_usr() {
    let ok = want(true, Some(LOC), Some(USR)).unwrap();
    assert_eq!((ok.location.as_str(), ok.usr.as_str()), (LOC, USR));
    assert_eq!(ok.room(), room_hash(LOC));
    assert_eq!(ok.member(), member_hash(LOC, USR));
    assert_eq!(want(false, Some(LOC), Some(USR)), None);
    assert_eq!(want(true, None, Some(USR)), None);
    assert_eq!(want(true, Some(LOC), None), None);
    assert_eq!(want(true, Some("garbage"), Some(USR)), None);
    assert_eq!(want(true, Some(LOC), Some("WhyKnot")), None);
}

#[test]
fn hub_queue_is_bounded_and_session_scoped() {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let id = hub.set_link(Link::Connected);
    for i in 0..100u64 {
        hub.push_snap(
            id,
            Snap {
                at: Instant::now(),
                room: [0; 16],
                entries: vec![([1; 8], dealt(i))],
            },
        );
    }
    let p = hub.poll();
    assert_eq!(p.link, Link::Connected);
    assert_eq!(p.snaps.len(), SNAP_QUEUE);
    assert_eq!(p.snaps.last().unwrap().entries[0].1, dealt(99));
    assert!(hub.poll().snaps.is_empty());
    hub.push_snap(
        id,
        Snap {
            at: Instant::now(),
            room: [0; 16],
            entries: vec![([1; 8], dealt(7))],
        },
    );
    let next = hub.set_link(Link::Retrying);
    assert_ne!(next, id);
    let p = hub.poll();
    assert_eq!((p.link, p.session), (Link::Retrying, next));
    assert!(p.snaps.is_empty());
    hub.push_snap(
        id,
        Snap {
            at: Instant::now(),
            room: [0; 16],
            entries: Vec::new(),
        },
    );
    assert!(hub.poll().snaps.is_empty());
    assert!(hub.shutdown(Duration::from_millis(10)));
}

#[test]
fn fake_server_helpers() {
    assert_eq!(
        crate::relay::sha256::hex(&sha1(b"abc")),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
    let nonce = "dGhlIHNhbXBsZSBub25jZQ==";
    let acc = sha1(format!("{nonce}258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes());
    assert_eq!(b64(&acc), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
}
