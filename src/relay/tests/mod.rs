use super::proto::tests::{LOC, USR};
use super::proto::{
    MAX_DMG, Msg, RUN, RUNS, Totals, claim_hash, decode_run, member_hash, room_hash,
};

fn dealt(n: u64) -> Totals {
    Totals::from((n, 0))
}
use super::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

mod basics;
mod links;
mod live;

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut digest: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&(crate::num::to_u64(data.len()) * 8).to_be_bytes());
    for block in padded.as_chunks::<64>().0 {
        let mut words = [0u32; 80];
        for (word, chunk) in words.iter_mut().zip(block.as_chunks::<4>().0) {
            *word = u32::from_be_bytes(*chunk);
        }
        for at in 16..80 {
            words[at] =
                (words[at - 3] ^ words[at - 8] ^ words[at - 14] ^ words[at - 16]).rotate_left(1);
        }
        let mut state = digest;
        for (round, word) in words.iter().enumerate() {
            let (mixed, constant) = match round {
                0..=19 => ((state[1] & state[2]) | (!state[1] & state[3]), 0x5A82_7999),
                20..=39 => (state[1] ^ state[2] ^ state[3], 0x6ED9_EBA1),
                40..=59 => (
                    (state[1] & state[2]) | (state[1] & state[3]) | (state[2] & state[3]),
                    0x8F1B_BCDC,
                ),
                _ => (state[1] ^ state[2] ^ state[3], 0xCA62_C1D6),
            };
            let next = state[0]
                .rotate_left(5)
                .wrapping_add(mixed)
                .wrapping_add(state[4])
                .wrapping_add(constant)
                .wrapping_add(*word);
            state.rotate_right(1);
            state[2] = state[2].rotate_left(30);
            state[0] = next;
        }
        for (sum, value) in digest.iter_mut().zip(state) {
            *sum = sum.wrapping_add(value);
        }
    }
    let mut out = [0u8; 20];
    for (bytes, value) in out.as_chunks_mut::<4>().0.iter_mut().zip(digest) {
        *bytes = value.to_be_bytes();
    }
    out
}

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = u32::from(c[0]) << 16
            | u32::from(*c.get(1).unwrap_or(&0)) << 8
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                out.push(char::from(T[crate::num::to_usize(n >> (18 - 6 * i) & 63)]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

struct Peer {
    s: TcpStream,
    path: String,
    runs: std::collections::VecDeque<Vec<u8>>,
}

impl Peer {
    fn accept(l: &TcpListener) -> Peer {
        Peer::accept_then(l, || ())
    }

    fn accept_then(listener: &TcpListener, before: impl FnOnce()) -> Peer {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        let mut req = Vec::new();
        let mut byte = [0u8; 1];
        while !req.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            req.push(byte[0]);
        }
        let req = String::from_utf8(req).unwrap();
        let path = req.split(' ').nth(1).unwrap().to_string();
        let key = req
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("sec-websocket-key")
                    .then(|| value.trim())
            })
            .unwrap();
        before();
        let acc = b64(&sha1(
            format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes(),
        ));
        write!(
            stream,
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {acc}\r\n\r\n"
        )
        .unwrap();
        Peer {
            s: stream,
            path,
            runs: VecDeque::default(),
        }
    }

    fn frame(&mut self) -> Option<(u8, Vec<u8>)> {
        let mut h = [0u8; 2];
        self.s.read_exact(&mut h).ok()?;
        let op = h[0] & 0x0f;
        let mut len = crate::num::to_usize(h[1] & 0x7f);
        if len == 126 {
            let mut e = [0u8; 2];
            self.s.read_exact(&mut e).ok()?;
            len = crate::num::to_usize(u16::from_be_bytes(e));
        } else if len == 127 {
            let mut e = [0u8; 8];
            self.s.read_exact(&mut e).ok()?;
            len = crate::num::to_usize(u64::from_be_bytes(e));
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

    fn raw(&mut self) -> Option<(u8, Vec<u8>)> {
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

    fn data(&mut self) -> Option<(u8, Vec<u8>)> {
        loop {
            let (op, p) = self.raw()?;
            if op == 0x2 && p.first() == Some(&RUN) {
                self.runs.push_back(p);
                continue;
            }
            return Some((op, p));
        }
    }

    fn run(&mut self) -> Vec<u8> {
        if let Some(p) = self.runs.pop_front() {
            return p;
        }
        let (op, p) = self.raw().unwrap();
        assert_eq!((op, p.first()), (0x2, Some(&RUN)));
        p
    }

    fn send(&mut self, op: u8, p: &[u8]) {
        let mut f = vec![0x80 | op];
        if p.len() < 126 {
            f.push(crate::num::to_u8(p.len()));
        } else {
            f.push(126);
            f.extend_from_slice(&crate::num::to_u16(p.len()).to_be_bytes());
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

fn refused_by_server(code: u16, link: Link) {
    let hub: &'static Hub = Box::leak(Box::new(Hub::new()));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    hub.sync(Some(test_want(port, LOC)), None);
    let mut peer = Peer::accept(&listener);
    peer.data().unwrap();
    wait_for(|| (hub.poll().link == Link::Connected).then_some(()));
    peer.send(0x8, &code.to_be_bytes());
    let (op, payload) = peer.data().unwrap();
    assert_eq!(op, 0x8);
    assert_eq!(u16::from_be_bytes([payload[0], payload[1]]), 1000);
    assert!(peer.data().is_none());
    wait_for(|| (hub.poll().link == link).then_some(()));
    listener.set_nonblocking(true).unwrap();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(4) {
        match listener.accept() {
            Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock),
            Ok(_) => panic!("reconnected after {code} in {:?}", start.elapsed()),
        }
        assert_eq!(hub.poll().link, link);
        std::thread::sleep(Duration::from_millis(100));
    }
    let start = Instant::now();
    assert!(hub.shutdown(Duration::from_secs(5)));
    assert!(start.elapsed() < Duration::from_secs(1));
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
    hub.poll().snaps.iter().any(|s| {
        s.entries
            .iter()
            .any(|(m, t)| *m == member && t.dealt == dmg)
    })
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
