use crate::game::event::{Event, parse_msg};
use crate::game::names::boss_name;
use crate::game::run::base_name;
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

const MAGIC: &str = "eboard 2";
const MAX_TEXT: usize = 40;
const MAX_MSG: usize = 1000;
const MAX_LINES: usize = 4000;
const MAX_BYTES: usize = 200 * 1024;
const MAX_QUEUE: usize = 900;
const FRESH: u64 = 60;
const EVERY: Duration = Duration::from_secs(1);
const BACKOFF: [u64; 4] = [1, 2, 4, 8];

const STREAM: &[&str] = &[
    "ECLIPTICA",
    "ENEMY ",
    "Enemy ",
    "Boss ",
    "Tracking boss",
    "Backup Active",
    "No backup active",
    "Advancing Stage",
    "fading out music",
    "Local controller",
    "STRIKE DMG",
    "NON-STRIKE DMG",
    "Dealing ",
    "damage has been taken",
    "spawn token",
    "Initializing Enemy",
    "Retiring Enemy",
    "[Behaviour] OnLeftRoom",
    "VRCApplication: HandleApplicationQuit",
];

pub(crate) const OK: u8 = 0;
pub(crate) const FAILING: u8 = 1;
pub(crate) const TAKEN: u8 = 2;

pub(crate) static ON: AtomicBool = AtomicBool::new(false);
pub(crate) static STATUS: AtomicU8 = AtomicU8::new(OK);
pub(crate) static DELETING: AtomicBool = AtomicBool::new(false);

pub(crate) fn streams(msg: &str) -> bool {
    STREAM.iter().any(|p| msg.starts_with(p))
}

fn clean(msg: &str) -> String {
    msg.trim_end()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_MSG)
        .collect()
}

fn short(s: &str) -> String {
    s.trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_TEXT)
        .collect::<String>()
        .trim()
        .to_string()
}

pub(crate) fn label(msg: &str) -> Option<(String, String)> {
    let Some(Event::BossFight { name, .. }) = parse_msg(msg) else {
        return None;
    };
    let base = base_name(&name);
    let ok = !base.is_empty()
        && base.len() <= MAX_TEXT
        && base.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    let shown = short(&boss_name(base));
    (ok && !shown.is_empty()).then(|| (base.to_string(), shown))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Header {
    pub name: String,
    pub party: u32,
    pub room: String,
    pub share_name: bool,
}

impl Header {
    pub(crate) fn ok(&self) -> bool {
        !self.name.is_empty() && !self.name.to_ascii_lowercase().contains("usr_")
    }
}

pub(crate) fn body(
    key: &str,
    stream: &str,
    seq: u64,
    now_ms: u64,
    h: &Header,
    lines: &[(u64, String)],
) -> String {
    let mut out = format!(
        "{MAGIC}\nkey {key}\nname {}\nstream {stream}\nseq {seq}\nnow {now_ms}\nparty {}\nroom {}\n",
        short(&h.name),
        h.party.max(1),
        h.room
    );
    if h.share_name {
        out.push_str("share name\n");
    }
    let labels: BTreeMap<String, String> = lines.iter().filter_map(|(_, m)| label(m)).collect();
    for (base, shown) in labels {
        let _ = writeln!(out, "label {base}\t{shown}");
    }
    for (ts, m) in lines {
        let _ = writeln!(out, "{ts}\t{m}");
    }
    out
}

pub(crate) fn forget_body(key: &str) -> String {
    format!("{MAGIC}\nkey {key}\nforget\n")
}

pub(crate) fn valid_key(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

pub(crate) fn random_hex(bytes: usize) -> String {
    use windows_sys::Win32::Security::Cryptography::{
        BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
    };
    let mut buf = vec![0u8; bytes];
    // SAFETY: the buffer pointer and length describe `buf`, which outlives the call.
    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            buf.as_mut_ptr(),
            crate::num::to_u32(buf.len()),
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status != 0 {
        for chunk in buf.chunks_mut(8) {
            let r = crate::relay::rand().to_le_bytes();
            chunk.copy_from_slice(&r[..chunk.len()]);
        }
    }
    crate::relay::sha256::hex(&buf)
}

pub(crate) fn new_key() -> String {
    random_hex(32)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reply {
    Done,
    Taken,
    Refused,
    Retry,
}

pub(crate) fn reply(status: Option<u32>) -> Reply {
    match status {
        Some(200..=299) => Reply::Done,
        Some(409) => Reply::Taken,
        Some(400..=499) => Reply::Refused,
        _ => Reply::Retry,
    }
}

pub(crate) struct Outbox {
    stream: String,
    seq: u64,
    lines: Vec<(u64, String)>,
    flushed: Option<Instant>,
    queue: VecDeque<String>,
    forget: bool,
    fails: usize,
}

impl Default for Outbox {
    fn default() -> Outbox {
        Outbox {
            stream: random_hex(8),
            seq: 0,
            lines: Vec::new(),
            flushed: None,
            queue: VecDeque::new(),
            forget: false,
            fails: 0,
        }
    }
}

pub(crate) enum Job {
    Forget,
    Send(String),
}

impl Outbox {
    pub(crate) fn restart(&mut self) {
        self.stream = random_hex(8);
        self.seq = 0;
        self.lines.clear();
        self.queue.clear();
    }

    pub(crate) fn push(&mut self, lines: impl IntoIterator<Item = (u64, String)>) {
        self.lines.extend(lines);
    }

    pub(crate) fn flush(
        &mut self,
        key: &str,
        header: impl FnOnce() -> Header,
        now_ms: u64,
        at: Instant,
    ) {
        if self.lines.is_empty() || self.flushed.is_some_and(|t| at.duration_since(t) < EVERY) {
            return;
        }
        self.flushed = Some(at);
        let h = header();
        if !h.ok() {
            self.lines.clear();
            return;
        }
        while !self.lines.is_empty() {
            let mut bytes = 0;
            let n = self
                .lines
                .iter()
                .take(MAX_LINES)
                .take_while(|(_, m)| {
                    bytes += m.len() + 24;
                    bytes <= MAX_BYTES
                })
                .count()
                .max(1);
            let part: Vec<(u64, String)> = self.lines.drain(..n).collect();
            self.queue
                .push_back(body(key, &self.stream, self.seq, now_ms, &h, &part));
            self.seq += 1;
        }
        if self.queue.len() > MAX_QUEUE {
            self.restart();
        }
    }

    pub(crate) fn forget(&mut self) {
        self.restart();
        self.forget = true;
    }

    pub(crate) fn job(&self, on: bool) -> Option<Job> {
        if self.forget {
            return Some(Job::Forget);
        }
        if !on {
            return None;
        }
        self.queue.front().cloned().map(Job::Send)
    }

    pub(crate) fn done(&mut self, job: &Job, r: Reply) -> (u8, Option<Duration>) {
        if r == Reply::Retry {
            let wait = BACKOFF[self.fails.min(BACKOFF.len() - 1)];
            self.fails += 1;
            return (FAILING, Some(Duration::from_secs(wait)));
        }
        self.fails = 0;
        match job {
            Job::Forget => self.forget = false,
            Job::Send(text) => {
                if self.queue.front() == Some(text) {
                    self.queue.pop_front();
                }
            }
        }
        (if r == Reply::Taken { TAKEN } else { OK }, None)
    }
}

static OUTBOX: Mutex<Option<Outbox>> = Mutex::new(None);
static KEY: Mutex<String> = Mutex::new(String::new());
static WAKE: Condvar = Condvar::new();

fn with<T>(f: impl FnOnce(&mut Outbox) -> T) -> T {
    let mut g = OUTBOX
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let out = f(g.get_or_insert_with(Outbox::default));
    drop(g);
    WAKE.notify_all();
    out
}

fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| crate::num::to_u64(d.as_millis()))
}

pub(crate) fn fresh(utc: u64, now: u64) -> bool {
    utc + FRESH >= now
}

pub(crate) fn feed(kept: &[(u64, String)], header: impl FnOnce() -> Header) {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    let now_ms = unix_ms();
    let now = now_ms / 1000;
    let lines: Vec<(u64, String)> = kept
        .iter()
        .filter(|(_, m)| streams(m))
        .map(|(ts, m)| (crate::usage::utc_unix(*ts), clean(m)))
        .filter(|(utc, _)| fresh(*utc, now))
        .collect();
    let key = KEY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let mut g = OUTBOX
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let o = g.get_or_insert_with(Outbox::default);
    o.push(lines);
    let had = o.queue.len();
    o.flush(&key, header, now_ms, Instant::now());
    let grew = o.queue.len() != had;
    drop(g);
    if grew {
        WAKE.notify_all();
    }
}

pub(crate) fn forget() {
    DELETING.store(true, Ordering::Relaxed);
    with(Outbox::forget);
}

pub(crate) fn spawn(key: String, forgot: impl Fn() + Send + 'static) {
    KEY.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone_from(&key);
    let _ = std::thread::Builder::new()
        .name("board".into())
        .spawn(move || {
            let mut g = OUTBOX
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut until = Some(Instant::now());
            loop {
                g = match until {
                    Some(t) => {
                        WAKE.wait_timeout(g, t.saturating_duration_since(Instant::now()))
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .0
                    }
                    None => WAKE
                        .wait(g)
                        .unwrap_or_else(std::sync::PoisonError::into_inner),
                };
                if until.is_some_and(|t| Instant::now() < t) {
                    continue;
                }
                let o = g.get_or_insert_with(Outbox::default);
                let on = ON.load(Ordering::Relaxed);
                if !on && !o.forget {
                    o.restart();
                }
                let Some(job) = o.job(on) else {
                    until = None;
                    if !on {
                        STATUS.store(OK, Ordering::Relaxed);
                    }
                    continue;
                };
                let text = match &job {
                    Job::Forget => forget_body(&key),
                    Job::Send(text) => text.clone(),
                };
                drop(g);
                let r = reply(crate::relay::post_board(text.as_bytes()));
                g = OUTBOX
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let o = g.get_or_insert_with(Outbox::default);
                let (status, retry) = o.done(&job, r);
                let gone = matches!(job, Job::Forget) && !o.forget;
                STATUS.store(status, Ordering::Relaxed);
                until = Some(Instant::now() + retry.unwrap_or(Duration::ZERO));
                if gone {
                    DELETING.store(false, Ordering::Relaxed);
                    drop(g);
                    forgot();
                    g = OUTBOX
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
            }
        });
}

#[cfg(test)]
#[path = "board_tests.rs"]
mod tests;
