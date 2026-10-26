mod client;
pub mod party;
pub mod proto;
pub mod sha256;

use crate::game::state::Summary;
use proto::{Claim, Member, Msg, Room, Totals};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const DEFAULT_URL: &str = "wss://ecliptica.whyknot.dev/ws";
const SNAP_QUEUE: usize = 8;
const RUN_QUEUE: usize = 8;
const SEND_GAP_MS: u64 = 1000;
const RUN_GAP_MS: u64 = 5000;
const MAX_BACKOFF_S: u64 = 60;
const MAX_STEP: u32 = 5;
const STABLE_SECS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Want {
    pub url: String,
    pub location: String,
    pub usr: String,
}

impl Want {
    pub fn room(&self) -> Room {
        proto::room_hash(&self.location)
    }

    pub fn member(&self) -> Member {
        proto::member_hash(&self.location, &self.usr)
    }

    pub fn claim(&self) -> Claim {
        proto::claim_hash(key(), &self.location)
    }
}

fn key() -> &'static [u8; 32] {
    static KEY: OnceLock<[u8; 32]> = OnceLock::new();
    KEY.get_or_init(|| {
        if cfg!(test) {
            [0x42; 32]
        } else {
            let path = std::env::var_os("APPDATA")
                .map(|b| PathBuf::from(b).join("EclipticaHUD").join("relay.key"));
            load_or_make_key(path.as_deref(), fresh_key)
        }
    })
}

pub fn load_or_make_key(path: Option<&Path>, fresh: impl FnOnce() -> [u8; 32]) -> [u8; 32] {
    if let Some(k) = path
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| key_from_hex(&t))
    {
        return k;
    }
    let k = fresh();
    if let Some(p) = path {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, sha256::hex(&k));
    }
    k
}

pub fn key_from_hex(text: &str) -> Option<[u8; 32]> {
    let t = text.trim().as_bytes();
    if t.len() != 64 || !t.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, pair) in t.chunks(2).enumerate() {
        let s = std::str::from_utf8(pair).ok()?;
        out[i] = u8::from_str_radix(s, 16).ok()?;
    }
    Some(out)
}

fn fresh_key() -> [u8; 32] {
    use windows_sys::Win32::Security::Cryptography::{
        BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
    };
    let mut k = [0u8; 32];
    let ok = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            k.as_mut_ptr(),
            k.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    } >= 0;
    if ok {
        k
    } else {
        let seed: Vec<u8> = (0..8).flat_map(|_| rand().to_le_bytes()).collect();
        sha256::sha256(&[&seed])
    }
}

pub fn want(on: bool, in_world: bool, location: Option<&str>, usr: Option<&str>) -> Option<Want> {
    if !on || !in_world {
        return None;
    }
    let location = location.filter(|l| l.starts_with("wrld_"))?;
    let usr = usr.filter(|u| u.starts_with("usr_"))?;
    Some(Want {
        url: relay_url().to_string(),
        location: location.to_string(),
        usr: usr.to_string(),
    })
}

fn relay_url() -> &'static str {
    static URL: OnceLock<String> = OnceLock::new();
    URL.get_or_init(|| url_from(std::env::var("EHUD_RELAY_URL").ok().as_deref()))
}

pub fn url_from(env: Option<&str>) -> String {
    env.map(str::trim)
        .filter(|u| Url::parse(u).is_some())
        .unwrap_or(DEFAULT_URL)
        .to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    pub secure: bool,
    pub host: String,
    pub port: u16,
    pub path: String,
}

impl Url {
    pub fn parse(s: &str) -> Option<Url> {
        let (secure, rest) = if let Some(r) = s.strip_prefix("wss://") {
            (true, r)
        } else {
            (false, s.strip_prefix("ws://")?)
        };
        let (auth, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let default = if secure { 443 } else { 80 };
        let (host, port) = if let Some(v6) = auth.strip_prefix('[') {
            let (h, tail) = v6.split_once(']')?;
            let port = match tail.strip_prefix(':') {
                Some(p) => p.parse().ok()?,
                None if tail.is_empty() => default,
                None => return None,
            };
            (h, port)
        } else {
            match auth.rsplit_once(':') {
                Some((h, p)) => (h, p.parse().ok()?),
                None => (auth, default),
            }
        };
        let ok_host = !host.is_empty()
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'));
        let ok_path = path.chars().all(|c| c.is_ascii_graphic());
        let ok_scheme = secure || loopback(host);
        (ok_host && ok_path && ok_scheme && port != 0).then(|| Url {
            secure,
            host: host.to_string(),
            port,
            path: path.to_string(),
        })
    }
}

fn loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

#[derive(Debug, Default)]
pub struct Backoff {
    fails: u32,
}

impl Backoff {
    pub fn next_ms(&mut self, rand: u64) -> u64 {
        let base = (2u64 << self.fails.min(MAX_STEP)).min(MAX_BACKOFF_S) * 1000;
        self.fails = self.fails.saturating_add(1);
        base + rand % (base / 4 + 1)
    }

    pub fn max_ms(&mut self, rand: u64) -> u64 {
        self.fails = self.fails.max(MAX_STEP);
        self.next_ms(rand)
    }

    pub fn reset(&mut self) {
        self.fails = 0;
    }

    pub fn after_session(&mut self, lived: Duration) {
        if lived.as_secs() >= STABLE_SECS {
            self.reset();
        }
    }
}

#[derive(Debug, Default)]
pub struct Throttle {
    sent: Option<Totals>,
    at: Option<u64>,
}

impl Throttle {
    pub fn due(&mut self, now_ms: u64, dmg: Option<Totals>) -> Option<Msg> {
        let v = dmg.map(Totals::capped);
        if self.sent == v || self.at.is_some_and(|t| now_ms < t + SEND_GAP_MS) {
            return None;
        }
        self.sent = v;
        self.at = Some(now_ms);
        Some(v.map_or(Msg::End, Msg::Dmg))
    }

    pub fn wait_ms(&self, now_ms: u64, dmg: Option<Totals>) -> u64 {
        let pending = self.sent != dmg.map(Totals::capped);
        match (pending, self.at) {
            (true, Some(t)) => (t + SEND_GAP_MS).saturating_sub(now_ms).max(1),
            (true, None) => 1,
            (false, _) => SEND_GAP_MS,
        }
    }
}

#[derive(Debug, Default)]
pub struct RunThrottle {
    sent: Option<Option<Summary>>,
    at: Option<u64>,
}

impl RunThrottle {
    pub fn due(&mut self, now_ms: u64, run: Option<&Summary>) -> bool {
        if self.sent.as_ref().is_some_and(|s| s.as_ref() == run)
            || self.at.is_some_and(|t| now_ms < t + RUN_GAP_MS)
        {
            return false;
        }
        self.sent = Some(run.cloned());
        self.at = Some(now_ms);
        true
    }

    pub fn wait_ms(&self, now_ms: u64, run: Option<&Summary>) -> u64 {
        let pending = self.sent.as_ref().is_none_or(|s| s.as_ref() != run);
        match (pending, self.at) {
            (true, Some(t)) => (t + RUN_GAP_MS).saturating_sub(now_ms).max(1),
            (true, None) => 1,
            (false, _) => RUN_GAP_MS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OwnRun {
    pub summary: Summary,
    pub now: u64,
    pub at: Instant,
}

impl OwnRun {
    pub fn encode(&self) -> Vec<u8> {
        let now = self.now + self.at.elapsed().as_secs();
        proto::encode_run(&self.summary, now)
    }
}

#[derive(Debug, Clone)]
pub struct PeerRun {
    pub at: Instant,
    pub room: Room,
    pub held: u32,
    pub payload: Vec<u8>,
}

impl PeerRun {
    pub fn summary(&self, now: u64) -> Option<Summary> {
        let held = self.held as u64 + self.at.elapsed().as_secs();
        proto::decode_run(&self.payload, now.saturating_sub(held))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Link {
    #[default]
    Off,
    Connecting,
    Connected,
    Retrying,
    Full,
    Taken,
}

#[derive(Debug, Clone)]
pub struct Snap {
    pub at: Instant,
    pub room: Room,
    pub entries: Vec<(Member, Totals)>,
}

#[derive(Default)]
struct Shared {
    want: Option<Want>,
    dmg: Option<Totals>,
    running: bool,
    link: Link,
    session: u64,
    snaps: VecDeque<Snap>,
    run: Option<OwnRun>,
    runs: VecDeque<PeerRun>,
}

impl Shared {
    fn end_session(&mut self) {
        self.session += 1;
        self.snaps.clear();
        self.runs.clear();
    }
}

pub struct Hub {
    m: Mutex<Shared>,
    cv: Condvar,
}

pub static HUB: Hub = Hub::new();

#[derive(Default)]
pub struct Poll {
    pub link: Link,
    pub session: u64,
    pub snaps: Vec<Snap>,
    pub runs: Vec<PeerRun>,
}

pub trait Relay: Sync {
    fn sync(&'static self, want: Option<Want>, dmg: Option<Totals>);
    fn poll(&self) -> Poll;
    fn share_run(&self, _run: Option<Summary>, _now: u64) {}
}

impl Relay for Hub {
    fn sync(&'static self, want: Option<Want>, dmg: Option<Totals>) {
        Hub::sync(self, want, dmg);
    }

    fn share_run(&self, run: Option<Summary>, now: u64) {
        Hub::share_run(self, run, now);
    }

    fn poll(&self) -> Poll {
        Hub::poll(self)
    }
}

impl Hub {
    pub const fn new() -> Self {
        Hub {
            m: Mutex::new(Shared {
                want: None,
                dmg: None,
                running: false,
                link: Link::Off,
                session: 0,
                snaps: VecDeque::new(),
                run: None,
                runs: VecDeque::new(),
            }),
            cv: Condvar::new(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Shared> {
        self.m.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn sync(&'static self, want: Option<Want>, dmg: Option<Totals>) {
        let mut s = self.lock();
        let changed = s.want != want || s.dmg != dmg;
        s.want = want;
        s.dmg = dmg;
        if s.want.is_some() && !s.running {
            let spawned = std::thread::Builder::new()
                .name("ehud-relay".into())
                .spawn(move || client::run(self));
            if spawned.is_ok() {
                s.running = true;
                s.link = Link::Connecting;
            } else {
                s.link = Link::Retrying;
            }
        }
        if changed {
            self.cv.notify_all();
        }
    }

    pub fn share_run(&self, run: Option<Summary>, now: u64) {
        let mut s = self.lock();
        if s.run.as_ref().map(|r| &r.summary) == run.as_ref() {
            return;
        }
        s.run = run.map(|summary| OwnRun {
            summary,
            now,
            at: Instant::now(),
        });
        self.cv.notify_all();
    }

    pub fn poll(&self) -> Poll {
        let mut s = self.lock();
        Poll {
            link: s.link,
            session: s.session,
            snaps: s.snaps.drain(..).collect(),
            runs: s.runs.drain(..).collect(),
        }
    }

    pub fn shutdown(&'static self, wait: Duration) -> bool {
        self.sync(None, None);
        let deadline = Instant::now() + wait;
        let mut s = self.lock();
        while s.running {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            s = self
                .cv
                .wait_timeout(s, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        true
    }

    fn push_snap(&self, session: u64, snap: Snap) {
        let mut s = self.lock();
        if s.session != session || s.link != Link::Connected {
            return;
        }
        if s.snaps.len() >= SNAP_QUEUE {
            s.snaps.pop_front();
        }
        s.snaps.push_back(snap);
    }

    fn push_run(&self, session: u64, run: PeerRun) {
        let mut s = self.lock();
        if s.session != session || s.link != Link::Connected {
            return;
        }
        if s.runs.len() >= RUN_QUEUE {
            s.runs.pop_front();
        }
        s.runs.push_back(run);
    }

    fn set_link(&self, link: Link) -> u64 {
        let mut s = self.lock();
        if link == Link::Connected || s.link == Link::Connected {
            s.end_session();
        }
        s.link = link;
        s.session
    }
}

pub fn usage_url(relay: &str) -> Option<Url> {
    let url = Url::parse(relay)?;
    Some(Url {
        path: "/usage".to_string(),
        ..url
    })
}

pub fn post_usage(body: &[u8]) -> Option<u32> {
    client::post(&usage_url(relay_url())?, body)
}

pub fn post_board(body: &[u8]) -> Option<u32> {
    let url = usage_url(relay_url())?;
    client::post(
        &Url {
            path: "/board".to_string(),
            ..url
        },
        body,
    )
}

pub fn rand() -> u64 {
    use std::hash::BuildHasher;
    std::collections::hash_map::RandomState::new().hash_one(Instant::now())
}

#[cfg(test)]
pub(crate) mod tests;
