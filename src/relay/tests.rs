use super::proto::tests::{LOC, USR};
use super::proto::{claim_hash, member_hash, room_hash, Msg};
use super::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

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
    assert!(b.next_ms(0) == 60_000);
    b.reset();
    assert_eq!(b.next_ms(0), 2000);
    b.after_session(Duration::from_secs(5));
    assert_eq!(b.next_ms(0), 4000);
    b.after_session(Duration::from_secs(STABLE_SECS));
    assert_eq!(b.next_ms(0), 2000);
}

#[test]
fn throttle_sends_changes_at_most_once_a_second() {
    let mut t = Throttle::default();
    assert_eq!(t.due(0, None), None);
    assert_eq!(t.wait_ms(0, None), 1000);
    assert_eq!(t.wait_ms(0, Some(5)), 1);
    assert_eq!(t.due(0, Some(5)), Some(Msg::Dmg(5)));
    assert_eq!(t.due(10, Some(5)), None);
    assert_eq!(t.wait_ms(10, Some(5)), 1000);
    assert_eq!(t.due(300, Some(9)), None);
    assert_eq!(t.wait_ms(300, Some(9)), 700);
    assert_eq!(t.due(999, Some(12)), None);
    assert_eq!(t.due(1000, Some(12)), Some(Msg::Dmg(12)));
    assert_eq!(t.due(5000, Some(12)), None);
    assert_eq!(t.wait_ms(5000, None), 1);
    assert_eq!(t.due(5000, None), Some(Msg::End));
    assert_eq!(t.due(7000, None), None);
    assert_eq!(t.wait_ms(7000, None), 1000);
    assert_eq!(t.due(7000, Some(0)), Some(Msg::Dmg(0)));
    assert_eq!(t.due(7500, None), None);
    assert_eq!(t.wait_ms(7500, None), 500);
    assert_eq!(t.due(8000, None), Some(Msg::End));
    assert_eq!(t.due(9000, Some(u64::MAX)), Some(Msg::Dmg(MAX_DMG)));
    assert_eq!(t.due(20000, Some(MAX_DMG + 7)), None);
    let mut sent = 0;
    for ms in (0..10_000).step_by(16) {
        if t.due(30_000 + ms, Some(ms)).is_some() {
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
    let k: [u8; 32] = std::array::from_fn(|i| i as u8 * 7);
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
    let ok = want(true, true, Some(LOC), Some(USR)).unwrap();
    assert_eq!((ok.location.as_str(), ok.usr.as_str()), (LOC, USR));
    assert_eq!(ok.room(), room_hash(LOC));
    assert_eq!(ok.member(), member_hash(LOC, USR));
    assert_eq!(want(false, true, Some(LOC), Some(USR)), None);
    assert_eq!(want(true, false, Some(LOC), Some(USR)), None);
    assert_eq!(want(true, true, None, Some(USR)), None);
    assert_eq!(want(true, true, Some(LOC), None), None);
    assert_eq!(want(true, true, Some("garbage"), Some(USR)), None);
    assert_eq!(want(true, true, Some(LOC), Some("WhyKnot")), None);
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
                entries: vec![([1; 8], i)],
            },
        );
    }
    let p = hub.poll();
    assert_eq!(p.link, Link::Connected);
    assert_eq!(p.snaps.len(), SNAP_QUEUE);
    assert_eq!(p.snaps.last().unwrap().entries[0].1, 99);
    assert!(hub.poll().snaps.is_empty());
    hub.push_snap(
        id,
        Snap {
            at: Instant::now(),
            room: [0; 16],
            entries: vec![([1; 8], 7)],
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

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut m = data.to_vec();
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for block in m.as_chunks::<64>().0 {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (s, v) in h.iter_mut().zip([a, b, c, d, e]) {
            *s = s.wrapping_add(v);
        }
    }
    let mut out = [0u8; 20];
    for (o, v) in out.as_chunks_mut::<4>().0.iter_mut().zip(h) {
        *o = v.to_be_bytes();
    }
    out
}

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[test]
fn fake_server_helpers() {
    assert_eq!(
        crate::relay::sha256::hex(&sha1(b"abc")),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
    let key = "dGhlIHNhbXBsZSBub25jZQ==";
    let acc = sha1(format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes());
    assert_eq!(b64(&acc), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
}

struct Peer {
    s: TcpStream,
    path: String,
}

impl Peer {
    fn accept(l: &TcpListener) -> Peer {
        Peer::accept_then(l, || ())
    }

    fn accept_then(l: &TcpListener, before: impl FnOnce()) -> Peer {
        let (mut s, _) = l.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        let mut req = Vec::new();
        let mut b = [0u8; 1];
        while !req.ends_with(b"\r\n\r\n") {
            s.read_exact(&mut b).unwrap();
            req.push(b[0]);
        }
        let req = String::from_utf8(req).unwrap();
        let path = req.split(' ').nth(1).unwrap().to_string();
        let key = req
            .lines()
            .find_map(|l| {
                let (k, v) = l.split_once(':')?;
                k.eq_ignore_ascii_case("sec-websocket-key")
                    .then(|| v.trim())
            })
            .unwrap();
        before();
        let acc = b64(&sha1(
            format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes(),
        ));
        write!(
            s,
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {acc}\r\n\r\n"
        )
        .unwrap();
        Peer { s, path }
    }

    fn frame(&mut self) -> Option<(u8, Vec<u8>)> {
        let mut h = [0u8; 2];
        self.s.read_exact(&mut h).ok()?;
        let op = h[0] & 0x0f;
        let mut len = (h[1] & 0x7f) as usize;
        if len == 126 {
            let mut e = [0u8; 2];
            self.s.read_exact(&mut e).ok()?;
            len = u16::from_be_bytes(e) as usize;
        } else if len == 127 {
            let mut e = [0u8; 8];
            self.s.read_exact(&mut e).ok()?;
            len = u64::from_be_bytes(e) as usize;
        }
        let mut mask = [0u8; 4];
        if h[1] & 0x80 != 0 {
            self.s.read_exact(&mut mask).ok()?;
        }
        let mut p = vec![0u8; len];
        self.s.read_exact(&mut p).ok()?;
        for (i, b) in p.iter_mut().enumerate() {
            *b ^= mask[i % 4];
        }
        Some((op, p))
    }

    fn data(&mut self) -> Option<(u8, Vec<u8>)> {
        loop {
            let (op, p) = self.frame()?;
            if op == 0x9 {
                self.send(0xA, &p);
                continue;
            }
            if op == 0xA {
                continue;
            }
            return Some((op, p));
        }
    }

    fn send(&mut self, op: u8, p: &[u8]) {
        let mut f = vec![0x80 | op];
        if p.len() < 126 {
            f.push(p.len() as u8);
        } else {
            f.push(126);
            f.extend_from_slice(&(p.len() as u16).to_be_bytes());
        }
        f.extend_from_slice(p);
        let _ = self.s.write_all(&f);
    }

    fn close_later(mut self) -> std::thread::JoinHandle<u16> {
        std::thread::spawn(move || self.expect_close())
    }

    fn expect_close(&mut self) -> u16 {
        let (op, p) = self.data().unwrap();
        assert_eq!(op, 0x8);
        self.send(0x8, &p[..2.min(p.len())]);
        u16::from_be_bytes([p[0], p[1]])
    }
}

fn test_want(port: u16, location: &str) -> Want {
    Want {
        url: format!("ws://127.0.0.1:{port}/ws"),
        location: location.to_string(),
        usr: USR.to_string(),
    }
}

fn wait_for<T>(mut f: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(Instant::now() < end, "timed out");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn client_joins_sends_damage_and_reads_snaps() {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let w = test_want(port, LOC);
    hub.sync(Some(w.clone()), None);
    let mut p = Peer::accept(&l);
    assert_eq!(p.path, "/ws");
    let (op, join) = p.data().unwrap();
    assert_eq!(op, 0x2);
    assert_eq!(join.len(), 41);
    assert_eq!(
        crate::relay::sha256::hex(&join[..25]),
        "016e35fa9ce4780b2a38a74e3b8d9ce66b6a59070463d1200d"
    );
    assert_eq!(join[25..], claim_hash(&[0x42; 32], LOC));
    assert_eq!(w.claim(), claim_hash(&[0x42; 32], LOC));
    wait_for(|| (hub.poll().link == Link::Connected).then_some(()));
    hub.sync(Some(w.clone()), Some(100));
    let (op, d) = p.data().unwrap();
    let first = Instant::now();
    assert_eq!((op, Msg::decode(&d)), (0x2, Some(Msg::Dmg(100))));
    hub.sync(Some(w.clone()), Some(200));
    hub.sync(Some(w.clone()), Some(300));
    let (_, d) = p.data().unwrap();
    assert_eq!(Msg::decode(&d), Some(Msg::Dmg(300)));
    assert!(first.elapsed() >= Duration::from_millis(850));
    let other = [7u8; 8];
    p.send(
        0x2,
        &Msg::Snap(vec![(member_hash(LOC, USR), 300), (other, 4242)]).encode(),
    );
    let snaps = wait_for(|| Some(hub.poll().snaps).filter(|s| !s.is_empty()));
    assert_eq!(snaps[0].room, room_hash(LOC));
    assert_eq!(snaps[0].entries[1], (other, 4242));
    p.send(0x2, &[0x03]);
    let snaps = wait_for(|| Some(hub.poll().snaps).filter(|s| !s.is_empty()));
    assert!(snaps[0].entries.is_empty());
    hub.sync(Some(w.clone()), None);
    let (_, d) = p.data().unwrap();
    assert_eq!(Msg::decode(&d), Some(Msg::End));
    hub.sync(None, None);
    assert_eq!(p.expect_close(), 1001);
    assert!(hub.shutdown(Duration::from_secs(5)));
    assert_eq!(hub.poll().link, Link::Off);
}

#[test]
fn client_reconnects_when_location_changes() {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    hub.sync(Some(test_want(port, LOC)), None);
    let mut a = Peer::accept(&l);
    let (_, join) = a.data().unwrap();
    assert_eq!(&join[1..17], &room_hash(LOC));
    let other = "wrld_00000000-0000-0000-0000-000000000000:99999~region(eu)";
    hub.sync(Some(test_want(port, other)), Some(5));
    assert_eq!(a.expect_close(), 1000);
    let mut b = Peer::accept(&l);
    let (_, join) = b.data().unwrap();
    assert_eq!(
        Msg::decode(&join),
        Some(Msg::Join {
            room: room_hash(other),
            member: member_hash(other, USR),
            claim: claim_hash(&[0x42; 32], other),
        })
    );
    let (_, d) = b.data().unwrap();
    assert_eq!(Msg::decode(&d), Some(Msg::Dmg(5)));
    let closed = b.close_later();
    assert!(hub.shutdown(Duration::from_secs(5)));
    assert_eq!(closed.join().unwrap(), 1001);
}

#[test]
fn client_closes_on_bad_frames_and_backs_off() {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    hub.sync(Some(test_want(port, LOC)), None);
    let mut a = Peer::accept(&l);
    a.data().unwrap();
    a.send(0x1, b"hello");
    let shut = Instant::now();
    assert_eq!(a.expect_close(), 1008);
    wait_for(|| (hub.poll().link == Link::Retrying).then_some(()));
    let mut b = Peer::accept(&l);
    let gap = shut.elapsed();
    assert!(gap >= Duration::from_millis(1900), "{gap:?}");
    assert!(gap <= Duration::from_millis(4000), "{gap:?}");
    b.data().unwrap();
    b.send(0x2, &[0x03, 1, 2]);
    assert_eq!(b.expect_close(), 1008);
    let mut c = Peer::accept(&l);
    c.data().unwrap();
    let mut huge = vec![0x03];
    huge.resize(1 + 16 * (proto::MAX_MEMBERS + 1), 0);
    c.send(0x2, &huge);
    assert_eq!(c.expect_close(), 1008);
    let t = Instant::now();
    assert!(hub.shutdown(Duration::from_secs(5)));
    assert!(t.elapsed() < Duration::from_secs(1));
}

#[test]
fn refused_backs_off_to_the_maximum() {
    let mut b = Backoff::default();
    for r in [0, 1, 7, 15_000, 15_001, u64::MAX, rand()] {
        let ms = b.max_ms(r);
        assert!((60_000..=75_000).contains(&ms), "{ms}");
    }
    b.reset();
    assert_eq!(b.next_ms(0), 2000);
    assert_eq!(b.max_ms(0), 60_000);
    assert_eq!(b.next_ms(0), 60_000);
}

fn refused_by_server(code: u16) {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    hub.sync(Some(test_want(port, LOC)), None);
    let mut a = Peer::accept(&l);
    a.data().unwrap();
    wait_for(|| (hub.poll().link == Link::Connected).then_some(()));
    a.send(0x8, &code.to_be_bytes());
    let (op, p) = a.data().unwrap();
    assert_eq!(op, 0x8);
    assert_eq!(u16::from_be_bytes([p[0], p[1]]), 1000);
    assert!(a.data().is_none());
    wait_for(|| (hub.poll().link == Link::Retrying).then_some(()));
    l.set_nonblocking(true).unwrap();
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(4) {
        match l.accept() {
            Err(e) => assert_eq!(e.kind(), std::io::ErrorKind::WouldBlock),
            Ok(_) => panic!("reconnected after {code} in {:?}", t.elapsed()),
        }
        assert_eq!(hub.poll().link, Link::Retrying);
        std::thread::sleep(Duration::from_millis(100));
    }
    let t = Instant::now();
    assert!(hub.shutdown(Duration::from_secs(5)));
    assert!(t.elapsed() < Duration::from_secs(1));
    assert_eq!(hub.poll().link, Link::Off);
}

#[test]
fn room_full_backs_off_to_the_maximum() {
    refused_by_server(client::ROOM_FULL);
}

#[test]
fn member_taken_backs_off_to_the_maximum() {
    refused_by_server(client::MEMBER_TAKEN);
}

#[test]
fn no_join_once_sharing_is_off() {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    hub.sync(Some(test_want(port, LOC)), None);
    let mut a = Peer::accept_then(&l, || hub.sync(None, None));
    if let Some((op, _)) = a.data() {
        assert_eq!(op, 0x8);
    }
    assert!(hub.shutdown(Duration::from_secs(5)));
    assert_eq!(hub.poll().link, Link::Off);
}

#[test]
fn quiet_socket_stays_open() {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    hub.sync(Some(test_want(port, LOC)), None);
    let mut a = Peer::accept(&l);
    a.data().unwrap();
    wait_for(|| (hub.poll().link == Link::Connected).then_some(()));
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(14) {
        assert_eq!(hub.poll().link, Link::Connected);
        std::thread::sleep(Duration::from_millis(250));
    }
    a.send(0x2, &[0x03]);
    let snaps = wait_for(|| Some(hub.poll().snaps).filter(|s| !s.is_empty()));
    assert!(snaps[0].entries.is_empty());
    let closed = a.close_later();
    assert!(hub.shutdown(Duration::from_secs(5)));
    assert_eq!(closed.join().unwrap(), 1001);
}

#[test]
fn shutdown_gives_up_on_a_server_that_never_answers_close() {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    hub.sync(Some(test_want(port, LOC)), None);
    let mut a = Peer::accept(&l);
    a.data().unwrap();
    wait_for(|| (hub.poll().link == Link::Connected).then_some(()));
    let t = Instant::now();
    assert!(hub.shutdown(Duration::from_secs(8)));
    let took = t.elapsed();
    assert!(took >= Duration::from_millis(1900), "{took:?}");
    assert!(took < Duration::from_secs(5), "{took:?}");
    assert_eq!(hub.poll().link, Link::Off);
    drop(a);
}

#[test]
fn unreachable_server_retries_without_blocking_sync() {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let t = Instant::now();
    hub.sync(Some(test_want(port, LOC)), Some(1));
    assert!(t.elapsed() < Duration::from_millis(50));
    wait_for(|| (hub.poll().link == Link::Retrying).then_some(()));
    let t = Instant::now();
    assert!(hub.shutdown(Duration::from_secs(5)));
    assert!(t.elapsed() < Duration::from_secs(1));
    assert_eq!(hub.poll().link, Link::Off);
}

fn live_want(url: &str, location: &str, usr: &str) -> Want {
    Want {
        url: url.to_string(),
        location: location.to_string(),
        usr: usr.to_string(),
    }
}

fn live_seen(hub: &Hub, member: proto::Member, dmg: u64) -> bool {
    hub.poll()
        .snaps
        .iter()
        .any(|s| s.entries.contains(&(member, dmg)))
}

#[test]
#[ignore]
fn live_relay_round_trip() {
    let url = std::env::var("EHUD_E2E_URL").expect("EHUD_E2E_URL");
    let hold: u64 = std::env::var("EHUD_E2E_HOLD_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let room_b = "wrld_00000000-0000-0000-0000-000000000000:54321~region(us)";
    let usr2 = "usr_11111111-1111-1111-1111-111111111111";
    let usr3 = "usr_22222222-2222-2222-2222-222222222222";
    let a1: &'static Hub = Box::leak(Box::new(Hub::new()));
    let a2: &'static Hub = Box::leak(Box::new(Hub::new()));
    let b1: &'static Hub = Box::leak(Box::new(Hub::new()));
    let (w1, w2, w3) = (
        live_want(&url, LOC, USR),
        live_want(&url, LOC, usr2),
        live_want(&url, room_b, usr3),
    );
    for (hub, w) in [(a1, &w1), (a2, &w2), (b1, &w3)] {
        hub.sync(Some(w.clone()), None);
    }
    for hub in [a1, a2, b1] {
        wait_for(|| (hub.poll().link == Link::Connected).then_some(()));
    }
    a1.sync(Some(w1.clone()), Some(1111));
    a2.sync(Some(w2.clone()), Some(2222));
    b1.sync(Some(w3.clone()), Some(3333));
    wait_for(|| live_seen(a1, w2.member(), 2222).then_some(()));
    wait_for(|| live_seen(a2, w1.member(), 1111).then_some(()));
    std::thread::sleep(Duration::from_millis(1500));
    for s in a1.poll().snaps.iter().chain(a2.poll().snaps.iter()) {
        assert!(!s.entries.iter().any(|(m, _)| *m == w3.member()));
    }
    assert!(b1.poll().snaps.is_empty());
    let usr4 = "usr_44444444-4444-4444-4444-444444444444";
    let b2: &'static Hub = Box::leak(Box::new(Hub::new()));
    let w4 = live_want(&url, room_b, usr4);
    b2.sync(Some(w4.clone()), None);
    wait_for(|| live_seen(b2, w3.member(), 3333).then_some(()));
    let end = Instant::now() + Duration::from_secs(hold);
    let mut tick = 0u64;
    while Instant::now() < end {
        tick += 1;
        std::thread::sleep(Duration::from_millis(1000));
        for hub in [a1, a2, b1, b2] {
            assert_eq!(hub.poll().link, Link::Connected, "dropped after {tick} s");
        }
    }
    a1.sync(Some(w1.clone()), Some(5555));
    wait_for(|| live_seen(a2, w1.member(), 5555).then_some(()));
    for hub in [a1, a2, b1, b2] {
        assert!(hub.shutdown(Duration::from_secs(5)));
        assert_eq!(hub.poll().link, Link::Off);
    }
}

#[test]
#[ignore]
fn live_relay_rejoins_after_restart() {
    let url = std::env::var("EHUD_E2E_URL").expect("EHUD_E2E_URL");
    let usr2 = "usr_33333333-3333-3333-3333-333333333333";
    let a1: &'static Hub = Box::leak(Box::new(Hub::new()));
    let a2: &'static Hub = Box::leak(Box::new(Hub::new()));
    let (w1, w2) = (live_want(&url, LOC, USR), live_want(&url, LOC, usr2));
    a1.sync(Some(w1.clone()), None);
    a2.sync(Some(w2.clone()), None);
    for hub in [a1, a2] {
        wait_for(|| (hub.poll().link == Link::Connected).then_some(()));
    }
    a1.sync(Some(w1.clone()), Some(10));
    wait_for(|| live_seen(a2, w1.member(), 10).then_some(()));
    println!("ready for restart");
    let end = Instant::now() + Duration::from_secs(120);
    while a1.poll().link == Link::Connected {
        assert!(Instant::now() < end, "server never restarted");
        std::thread::sleep(Duration::from_millis(100));
    }
    let lost = Instant::now();
    for hub in [a1, a2] {
        let end = Instant::now() + Duration::from_secs(60);
        while hub.poll().link != Link::Connected {
            assert!(Instant::now() < end, "no reconnect");
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    println!("rejoined after {} ms", lost.elapsed().as_millis());
    a1.sync(Some(w1.clone()), Some(20));
    let end = Instant::now() + Duration::from_secs(10);
    while !live_seen(a2, w1.member(), 20) {
        assert!(Instant::now() < end, "no snap after rejoin");
        std::thread::sleep(Duration::from_millis(50));
    }
    for hub in [a1, a2] {
        assert!(hub.shutdown(Duration::from_secs(5)));
    }
}

pub(crate) fn hub() -> &'static Hub {
    Box::leak(Box::new(Hub::new()))
}

pub(crate) fn connect(hub: &Hub) -> u64 {
    hub.set_link(Link::Connected)
}

pub(crate) fn push(hub: &Hub, id: u64, snap: Snap) {
    hub.push_snap(id, snap);
}

pub(crate) fn drop_link(hub: &Hub) {
    hub.set_link(Link::Retrying);
}
