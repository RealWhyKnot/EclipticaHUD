use crate::game::event::{civil, parse_msg, Event};
use crate::history::{History, PartyRecord};
use std::collections::HashSet;
use std::fmt::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const MAX_TEXT: usize = 2 * 1024 * 1024;
pub const MAX_BODY: usize = 240 * 1024;
const MAX_STR: usize = 96;
const MAX_ROWS: usize = 82;
const MAX_PARTY: usize = 500;
const MAX_BATCHES: usize = 8;
const FIRST_WAIT: Duration = Duration::from_secs(90);
const EVERY: Duration = Duration::from_secs(600);
const PARTY: &str = "";
const MIN_TS: u64 = 86_400 * 366 * 1000;

pub static ON: AtomicBool = AtomicBool::new(false);
pub static FAILING: AtomicBool = AtomicBool::new(false);

pub struct Batch {
    pub text: String,
    pub cursors: Vec<(String, u64)>,
}

pub struct Scrub {
    long: Vec<String>,
    short: Vec<String>,
}

impl Scrub {
    pub fn new<'a>(names: impl IntoIterator<Item = &'a str>) -> Scrub {
        let mut long = Vec::new();
        let mut short = Vec::new();
        for name in names {
            let name = name.trim();
            if name.chars().count() >= 3 {
                long.push(name.to_string());
            } else if !name.is_empty() {
                short.push(name.to_string());
            }
        }
        long.sort_by_key(|n| std::cmp::Reverse(n.len()));
        long.dedup();
        Scrub { long, short }
    }

    pub fn clean(&self, s: &str) -> String {
        let mut out: String = s
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        for name in &self.long {
            if out.contains(name.as_str()) {
                out = out.replace(name.as_str(), "player");
            }
        }
        if self.short.iter().any(|n| out.trim() == n) {
            out = "player".to_string();
        }
        out = drop_ids(&out);
        out.chars()
            .take(MAX_STR)
            .collect::<String>()
            .trim()
            .to_string()
    }
}

fn drop_ids(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("usr_") {
        out.push_str(&rest[..i]);
        out.push_str("usr");
        rest = rest[i + 4..].trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '-');
    }
    out.push_str(rest);
    out
}

fn float(f: f32) -> Option<String> {
    if !f.is_finite() || f < 0.0 {
        return None;
    }
    let s = format!("{f:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    (s.len() <= 12).then(|| s.to_string())
}

fn world(location: &str) -> Option<String> {
    let id = location.split(':').next()?;
    let hex = id.strip_prefix("wrld_")?;
    let ok = !hex.is_empty()
        && hex.len() <= 64
        && hex.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-');
    ok.then(|| id.to_string())
}

fn fields(ev: &Event, scrub: &Scrub, me: Option<&str>) -> Option<(&'static str, String)> {
    let some = |s: String| (!s.is_empty()).then_some(s);
    let s = |v: &str| some(scrub.clean(v));
    let out = match ev {
        Event::BossFight { name } | Event::BossDead { name } => s(name)?,
        Event::EnemyName(name) | Event::RoomEnter(name) => s(name)?,
        Event::StrikeTotal(n) | Event::NonStrikeTotal(n) => n.to_string(),
        Event::EnemyRetire(n) => n.to_string(),
        Event::Dealt { n, strike } => format!("{n}\t{}", u8::from(*strike)),
        Event::DamageTaken { amount, source } => format!("{amount}\t{}", scrub.clean(source)),
        Event::Ownership { object, player } => {
            let who = if me == Some(player.as_str()) {
                "you"
            } else {
                "other"
            };
            format!("{}\t{who}", s(object)?)
        }
        Event::Stage {
            name,
            progress,
            class,
        } => format!("{}\t{}\t{}", s(name)?, float(*progress)?, s(class)?),
        Event::EnemySpawn { slot, kind } => format!("{slot}\t{kind}"),
        Event::TokenSpawn { rune, chance } => format!("{}\t{chance}", u8::from(*rune)),
        Event::RoomJoin(location) => return Some(("world", world(location)?)),
        Event::Intermission
        | Event::Lobby
        | Event::RoomLeft
        | Event::PlayerDead
        | Event::SessionSave => String::new(),
        Event::SelfName { .. } | Event::PlayerJoined { .. } | Event::PlayerLeft { .. } => {
            return None
        }
    };
    Some((ev.label(), out))
}

fn party_line(rec: &PartyRecord, scrub: &Scrub) -> Option<String> {
    let boss = scrub.clean(&rec.boss);
    if boss.is_empty() || rec.rows.is_empty() {
        return None;
    }
    let rows: Vec<String> = rec
        .rows
        .iter()
        .take(MAX_ROWS)
        .map(|r| format!("{}:{}:{}", r.dealt, r.taken, u8::from(r.you)))
        .collect();
    Some(format!(
        "0\tparty\t{boss}\t{}\t{}\t{}\t{}\t{}",
        float(rec.progress)?,
        rec.secs,
        u8::from(rec.down),
        rec.here,
        rows.join(",")
    ))
}

fn day(ts: u64) -> String {
    if ts < MIN_TS {
        return "0000-00-00".to_string();
    }
    let (y, m, d) = civil(ts);
    format!("{y:04}-{m:02}-{d:02}")
}

struct Out {
    text: String,
    prev: Option<u64>,
    first: Option<u64>,
    limit: usize,
}

impl Out {
    fn push(&mut self, ts: u64, tag: &str, rest: &str) {
        let dt = self.prev.map_or(0, |p| ts.saturating_sub(p));
        self.prev = Some(ts);
        self.first.get_or_insert(ts);
        let _ = write!(self.text, "{dt}\t{tag}");
        if !rest.is_empty() {
            self.text.push('\t');
            self.text.push_str(rest);
        }
        self.text.push('\n');
    }

    fn full(&self) -> bool {
        self.text.len() >= self.limit
    }
}

fn log_lines(db: &History, name: &str, from: u64, out: &mut Out, all: &mut HashSet<String>) -> u64 {
    let mut lines = Vec::new();
    db.replay(name, |ts, msg| lines.push((ts, parse_msg(msg))));
    let names: HashSet<String> = lines
        .iter()
        .filter_map(|(_, ev)| match ev {
            Some(Event::SelfName { name, .. })
            | Some(Event::PlayerJoined { name, .. })
            | Some(Event::PlayerLeft { name, .. }) => Some(name.clone()),
            _ => None,
        })
        .collect();
    all.extend(names.iter().cloned());
    let scrub = Scrub::new(names.iter().map(String::as_str));
    let mut me: Option<String> = None;
    let mut here: HashSet<String> = HashSet::new();
    let mut inside = false;
    for (n, (ts, ev)) in lines.iter().enumerate() {
        let n = n as u64;
        if n >= from && out.full() {
            return n;
        }
        let Some(ev) = ev else {
            continue;
        };
        let count = here.len();
        let was_inside = inside;
        match ev {
            Event::SelfName { name, .. } => me = Some(name.clone()),
            Event::PlayerJoined { usr, .. } => {
                here.insert(usr.clone());
            }
            Event::PlayerLeft { usr, .. } => {
                here.remove(usr);
            }
            Event::RoomJoin(_) => here.clear(),
            Event::RoomLeft => {
                here.clear();
                inside = false;
            }
            Event::RoomEnter(room) => inside = room.starts_with("Ecliptica"),
            _ => {}
        }
        if n < from {
            continue;
        }
        if !inside && !(was_inside && matches!(ev, Event::RoomLeft)) {
            if matches!(ev, Event::RoomEnter(_)) {
                out.push(*ts, "room_enter", "other");
            }
            continue;
        }
        if let Some((tag, rest)) = fields(ev, &scrub, me.as_deref()) {
            out.push(*ts, tag, &rest);
        }
        if here.len() != count && !here.is_empty() {
            out.push(*ts, "players", &here.len().to_string());
        }
    }
    lines.len() as u64
}

pub fn build(db: &History, version: &str, id: String, limit: usize) -> Option<Batch> {
    let cursors = db.usage_cursors();
    if !cursors.contains_key(PARTY) {
        return None;
    }
    let newest_known = cursors
        .keys()
        .filter(|k| k.as_str() != PARTY)
        .max()
        .cloned();
    let mut out = Out {
        text: String::new(),
        prev: None,
        first: None,
        limit,
    };
    let mut moved = Vec::new();
    let mut names = HashSet::new();
    for (name, end) in db.log_ends() {
        let from = match cursors.get(&name) {
            Some(&n) => n,
            None if newest_known.as_ref().is_some_and(|k| name < *k) => end,
            None => 0,
        };
        if from >= end {
            if !cursors.contains_key(&name) {
                moved.push((name, end));
            }
            continue;
        }
        if out.full() {
            break;
        }
        let to = log_lines(db, &name, from, &mut out, &mut names);
        moved.push((name, to));
    }
    let party_from = cursors[PARTY];
    let mut party_to = party_from;
    let mut party = String::new();
    if !out.full() {
        let scrub = Scrub::new(names.iter().map(String::as_str));
        for rec in db.party_after(party_from, MAX_PARTY) {
            if out.text.len() + party.len() >= limit {
                break;
            }
            party_to = rec.at;
            out.first.get_or_insert(rec.at);
            if let Some(line) = party_line(&rec, &scrub) {
                party.push_str(&line);
                party.push('\n');
            }
        }
    }
    if party_to != party_from {
        moved.push((PARTY.to_string(), party_to));
    }
    if moved.is_empty() {
        return None;
    }
    out.text.push_str(&party);
    let version: String = version
        .chars()
        .filter(char::is_ascii_graphic)
        .take(32)
        .collect();
    let version = if version.is_empty() {
        "dev".to_string()
    } else {
        version
    };
    let head = format!(
        "v1\t{version}\t{id}\t{}\n",
        day(out.first.unwrap_or_default())
    );
    Some(Batch {
        text: if out.first.is_some() {
            head + &out.text
        } else {
            String::new()
        },
        cursors: moved,
    })
}

pub fn compress(text: &str) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(text.as_bytes(), 6)
}

pub fn next(db: &History, version: &str, id: String) -> Option<(Batch, Vec<u8>)> {
    let mut limit = MAX_TEXT;
    loop {
        let batch = build(db, version, id.clone(), limit)?;
        let body = if batch.text.is_empty() {
            Vec::new()
        } else {
            compress(&batch.text)
        };
        if body.len() <= MAX_BODY || limit < 4096 {
            return Some((batch, body));
        }
        limit /= 2;
    }
}

pub fn batch_id() -> String {
    format!("{:016x}{:016x}", crate::relay::rand(), crate::relay::rand())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sent {
    Kept,
    Dropped,
    Retry,
}

pub fn outcome(status: Option<u32>) -> Sent {
    match status {
        Some(200..=299) => Sent::Kept,
        Some(400 | 413) => Sent::Dropped,
        _ => Sent::Retry,
    }
}

pub fn pass(
    db: &mut History,
    version: &str,
    on: &AtomicBool,
    pending: &mut Option<(Batch, Vec<u8>)>,
    mut post: impl FnMut(&[u8]) -> Option<u32>,
) {
    for _ in 0..MAX_BATCHES {
        if !on.load(Ordering::Relaxed) {
            *pending = None;
            return;
        }
        if pending.is_none() {
            *pending = next(db, version, batch_id());
        }
        let Some((batch, body)) = pending.as_ref() else {
            return;
        };
        let sent = if body.is_empty() {
            Sent::Kept
        } else {
            outcome(post(body))
        };
        if sent == Sent::Retry {
            return;
        }
        if !on.load(Ordering::Relaxed) || !db.usage_commit(&batch.cursors) {
            *pending = None;
            return;
        }
        *pending = None;
    }
}

pub fn spawn(path: Option<PathBuf>) {
    let Some(path) = path else {
        return;
    };
    let _ = std::thread::Builder::new()
        .name("usage".into())
        .spawn(move || {
            std::thread::sleep(FIRST_WAIT);
            let mut db = None;
            let mut pending = None;
            loop {
                if ON.load(Ordering::Relaxed) {
                    if db.is_none() {
                        db = History::open(&path);
                    }
                    let mut failed = db.is_none();
                    if let Some(db) = db.as_mut() {
                        pass(db, crate::update::VERSION, &ON, &mut pending, |body| {
                            let status = crate::relay::post_usage(body);
                            failed = outcome(status) == Sent::Retry;
                            status
                        });
                    }
                    FAILING.store(failed && ON.load(Ordering::Relaxed), Ordering::Relaxed);
                } else {
                    pending = None;
                    FAILING.store(false, Ordering::Relaxed);
                }
                std::thread::sleep(EVERY);
            }
        });
}

#[cfg(test)]
#[path = "usage_tests.rs"]
mod tests;
