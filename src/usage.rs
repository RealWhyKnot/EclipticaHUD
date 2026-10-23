use crate::game::event::{civil, parse_msg, Event};
use crate::game::run::{Run, RunEnd};
use crate::game::state::GameState;
use crate::history::{History, PartyRecord};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const MAX_TEXT: usize = 2 * 1024 * 1024;
pub const MAX_BODY: usize = 240 * 1024;
const MAX_STR: usize = 1000;
const MAX_ROWS: usize = 82;
const MAX_PACK: usize = 200;
const MAX_PARTY: usize = 500;
const MAX_BATCHES: usize = 8;
const FIRST_WAIT: Duration = Duration::from_secs(90);
const EVERY: Duration = Duration::from_secs(600);
const PARTY: &str = "#party";
const PARTY_AT: &str = "";
const MIN_TS: u64 = 86_400 * 366 * 1000;
const UNIX_DAYS: u64 = 719_590;

pub static ON: AtomicBool = AtomicBool::new(false);
pub static FAILING: AtomicBool = AtomicBool::new(false);

pub struct Batch {
    pub text: String,
    pub cursors: Vec<(String, u64)>,
}

pub struct Scrub {
    me: HashSet<String>,
    names: Vec<String>,
    tags: HashMap<String, String>,
    pub salt: String,
}

fn edge(c: Option<char>) -> bool {
    c.is_none_or(|c| !c.is_alphanumeric())
}

fn find_word(s: &str, name: &str, mut from: usize) -> Option<usize> {
    let open = name.chars().next().is_some_and(char::is_alphanumeric);
    let close = name.chars().next_back().is_some_and(char::is_alphanumeric);
    while let Some(i) = s.get(from..)?.find(name) {
        let at = from + i;
        let end = at + name.len();
        if (!open || edge(s[..at].chars().next_back())) && (!close || edge(s[end..].chars().next()))
        {
            return Some(at);
        }
        from = at + s[at..].chars().next().map_or(1, char::len_utf8);
    }
    None
}

impl Scrub {
    pub fn new(me: &[String], others: &[String]) -> Scrub {
        let mut names: Vec<String> = me
            .iter()
            .chain(others)
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .collect();
        names.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        names.dedup();
        Scrub {
            me: me.iter().map(|n| n.trim().to_string()).collect(),
            names,
            tags: HashMap::new(),
            salt: String::new(),
        }
    }

    pub fn reset(&mut self) {
        self.tags.clear();
    }

    pub fn who(&mut self, name: &str) -> String {
        let name = name.trim();
        if self.me.contains(name) {
            return "you".to_string();
        }
        let next = format!("P{}", self.tags.len() + 1);
        self.tags.entry(name.to_string()).or_insert(next).clone()
    }

    pub fn clean(&mut self, s: &str) -> String {
        let mut out: String = s
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let hits: Vec<String> = self
            .names
            .iter()
            .filter(|n| out.contains(n.as_str()))
            .cloned()
            .collect();
        for name in hits {
            let mut tag = None;
            let mut from = 0;
            while let Some(at) = find_word(&out, &name, from) {
                let tag = tag.get_or_insert_with(|| self.who(&name)).clone();
                out.replace_range(at..at + name.len(), &tag);
                from = at + tag.len();
            }
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

fn world(location: &str, salt: &str) -> Option<String> {
    let mut parts = location.splitn(2, ':');
    let id = parts.next()?;
    let hex = id.strip_prefix("wrld_")?;
    let ok = !hex.is_empty()
        && hex.len() <= 64
        && hex.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-');
    if !ok {
        return None;
    }
    let instance = parts.next().and_then(|rest| rest.split('~').next());
    Some(match instance {
        Some(n) if !n.is_empty() && n.len() <= 16 && n.bytes().all(|b| b.is_ascii_digit()) => {
            format!("{id}\t{n}")
        }
        Some(n) if !n.is_empty() => format!("{id}\tx{}", hash(&[salt, n], 1)),
        _ => format!("{id}\t"),
    })
}

fn fields(ev: &Event, scrub: &mut Scrub) -> Option<(&'static str, String)> {
    let out = match ev {
        Event::BossFight { name, phase } => {
            format!("{}\t{}", scrub.clean(name), scrub.clean(phase))
        }
        Event::BossDead { name } | Event::EnemyName(name) | Event::RoomEnter(name) => {
            scrub.clean(name)
        }
        Event::StrikeTotal(n) | Event::NonStrikeTotal(n) => n.to_string(),
        Event::Dealt { n, strike } => format!("{n}\t{}", u8::from(*strike)),
        Event::DamageTaken { amount, source } => format!("{amount}\t{}", scrub.clean(source)),
        Event::Ownership { object, player } => {
            format!("{}\t{}", scrub.clean(object), scrub.who(player))
        }
        Event::Stage {
            name,
            progress,
            class,
        } => format!(
            "{}\t{}\t{}",
            scrub.clean(name),
            float(*progress).unwrap_or_default(),
            scrub.clean(class)
        ),
        Event::EnemySpawn { slot, kind } => format!("{slot}\t{kind}"),
        Event::TokenSpawn { rune, chance } => format!("{}\t{chance}", u8::from(*rune)),
        Event::RoomJoin(location) => return Some(("world", world(location, &scrub.salt)?)),
        Event::SessionSave(id) => scrub.clean(id),
        Event::WorldLine(text) => scrub.clean(text),
        Event::PlayerJoined { name, .. } => return Some(("player_join", scrub.who(name))),
        Event::PlayerLeft { name, .. } => return Some(("player_left", scrub.who(name))),
        Event::Intermission | Event::Lobby | Event::RoomLeft | Event::PlayerDead => String::new(),
        Event::EnemyRetire(_) | Event::SelfName { .. } => return None,
    };
    Some((ev.label(), out))
}

fn party_line(seq: u64, rec: &PartyRecord, scrub: &mut Scrub) -> Option<String> {
    if rec.rows.is_empty() {
        return None;
    }
    scrub.reset();
    let rows: Vec<String> = rec
        .rows
        .iter()
        .take(MAX_ROWS)
        .map(|r| {
            let who = if r.you {
                "you".to_string()
            } else {
                scrub.who(&r.name)
            };
            format!("{}:{}:{who}", r.dealt, r.taken)
        })
        .collect();
    Some(format!(
        "0\tparty\t{seq}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        unix(rec.at),
        scrub.clean(&rec.boss),
        float(rec.progress).unwrap_or_default(),
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

fn unix(ts: u64) -> u64 {
    ts.saturating_sub(UNIX_DAYS * 86_400)
}

fn sha256(data: &[u8]) -> Option<[u8; 32]> {
    use windows_sys::Win32::Security::Cryptography::{BCryptHash, BCRYPT_SHA256_ALG_HANDLE};
    let mut out = [0u8; 32];
    let status = unsafe {
        BCryptHash(
            BCRYPT_SHA256_ALG_HANDLE,
            std::ptr::null(),
            0,
            data.as_ptr(),
            u32::try_from(data.len()).ok()?,
            out.as_mut_ptr(),
            32,
        )
    };
    (status == 0).then_some(out)
}

fn hash(parts: &[&str], n: usize) -> String {
    let mut data = Vec::new();
    for p in parts {
        data.extend_from_slice(&(p.len() as u64).to_le_bytes());
        data.extend_from_slice(p.as_bytes());
    }
    let digest = sha256(&data).unwrap_or_else(|| {
        let mut h = DefaultHasher::new();
        parts.hash(&mut h);
        let mut out = [0u8; 32];
        for (i, chunk) in out.chunks_mut(8).enumerate() {
            let mut h = h.clone();
            i.hash(&mut h);
            chunk.copy_from_slice(&h.finish().to_le_bytes());
        }
        out
    });
    let mut out = String::new();
    for b in &digest[..(n * 8).min(32)] {
        let _ = write!(out, "{b:02x}");
    }
    out
}

pub fn utc_offset_at(ts: u64) -> i64 {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::Time::TzSpecificLocalTimeToSystemTime;
    if ts < MIN_TS {
        return 0;
    }
    let (y, m, d) = civil(ts);
    let secs = ts % 86_400;
    let local = SYSTEMTIME {
        wYear: y as u16,
        wMonth: m as u16,
        wDayOfWeek: 0,
        wDay: d as u16,
        wHour: (secs / 3600) as u16,
        wMinute: (secs / 60 % 60) as u16,
        wSecond: (secs % 60) as u16,
        wMilliseconds: 0,
    };
    let mut utc = SYSTEMTIME::default();
    if unsafe { TzSpecificLocalTimeToSystemTime(std::ptr::null(), &local, &mut utc) } == 0 {
        return 0;
    }
    let minutes = |t: &SYSTEMTIME| i64::from(t.wHour) * 60 + i64::from(t.wMinute);
    let date = |t: &SYSTEMTIME| (t.wYear, t.wMonth, t.wDay);
    let day = match date(&local).cmp(&date(&utc)) {
        std::cmp::Ordering::Less => -1440,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1440,
    };
    minutes(&local) - minutes(&utc) + day
}

struct Out {
    text: String,
    prev: Option<u64>,
    first: Option<u64>,
    limit: usize,
    retire: Option<(u64, Vec<u32>)>,
}

impl Out {
    fn line(&mut self, ts: u64, tag: &str, rest: &str) {
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

    fn push(&mut self, ts: u64, tag: &str, rest: &str) {
        self.flush();
        self.line(ts, tag, rest);
    }

    fn retire(&mut self, ts: u64, slot: u32) {
        match self.retire.as_mut() {
            Some((at, slots)) if *at == ts && slots.len() < MAX_PACK => slots.push(slot),
            _ => {
                self.flush();
                self.retire = Some((ts, vec![slot]));
            }
        }
    }

    fn flush(&mut self) {
        if let Some((ts, slots)) = self.retire.take() {
            let list: Vec<String> = slots.iter().map(u32::to_string).collect();
            self.line(ts, "enemy_retire", &list.join(","));
        }
    }

    fn full(&self) -> bool {
        self.text.len() >= self.limit
    }
}

fn end_name(end: Option<RunEnd>) -> &'static str {
    match end {
        Some(RunEnd::Won) => "won",
        Some(RunEnd::Lost) => "lost",
        Some(RunEnd::Lobby) => "lobby",
        Some(RunEnd::Left) => "left",
        None => "open",
    }
}

fn run_lines(run: &Run, ts: u64, out: &mut Out, scrub: &mut Scrub) {
    let off = |t: u64| t.saturating_sub(run.start_ts);
    let end = run.end_ts.unwrap_or(ts);
    for (i, s) in run.stages.iter().enumerate() {
        let rest = format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            scrub.clean(&s.name),
            i + 1,
            off(s.start_ts),
            s.secs(end),
            s.dmg,
            s.taken,
            s.hits,
            s.max_hit,
            s.deaths
        );
        out.push(ts, "stage_end", &rest);
    }
    for (g, range) in run.groups().into_iter().enumerate() {
        for f in &run.fights[range] {
            let result = match (f.lost, f.kill, f.end_ts) {
                (true, _, _) => "lost",
                (_, Some(_), _) => "killed",
                (_, None, Some(_)) => "unfinished",
                (_, None, None) => "open",
            };
            let (strike, non) = f.kill.map_or((String::new(), String::new()), |(a, b)| {
                (a.to_string(), b.to_string())
            });
            let name = scrub.clean(&f.name);
            let rest = format!(
                "{name}\t{}\t{result}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{strike}\t{non}",
                g + 1,
                off(f.start_ts),
                f.secs(end),
                f.dmg,
                f.taken,
                f.hits,
                f.max_hit,
                f.deaths
            );
            out.push(ts, "fight_end", &rest);
            for t in &f.attacks {
                let rest = format!(
                    "{name}\t{}\t{}\t{}\t{}",
                    scrub.clean(&t.who),
                    scrub.clean(&t.attack),
                    t.total,
                    t.hits
                );
                out.push(ts, "fight_attack", &rest);
            }
        }
    }
    let rest = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        end_name(run.end),
        end.saturating_sub(run.start_ts),
        run.stages.len(),
        run.fights.len(),
        run.groups().len(),
        run.kills(),
        run.deaths,
        scrub.clean(&run.class),
        scrub.clean(&run.stage),
        run.dmg(),
        run.taken(),
        run.hit_count(),
        run.max_hit()
    );
    out.push(ts, "run_end", &rest);
}

struct Derived {
    gs: GameState,
    deaths: u64,
    runs: usize,
    cut: Option<usize>,
}

impl Derived {
    fn new() -> Derived {
        Derived {
            gs: GameState::default(),
            deaths: 0,
            runs: 0,
            cut: None,
        }
    }

    fn open(&mut self) {
        self.cut.get_or_insert(self.gs.runs.len());
    }

    fn emit(&mut self, ts: u64, out: &mut Out, scrub: &mut Scrub) {
        while self.deaths < self.gs.deaths_total {
            self.deaths += 1;
            out.push(ts, "death", "");
        }
        while let Some(run) = self.gs.runs.get(self.runs).filter(|r| r.end_ts.is_some()) {
            if self.cut.is_some_and(|cut| self.runs >= cut) {
                run_lines(run, ts, out, scrub);
            }
            self.runs += 1;
        }
    }
}

fn sink() -> Out {
    Out {
        text: String::new(),
        prev: None,
        first: None,
        limit: usize::MAX,
        retire: None,
    }
}

fn log_lines(
    db: &History,
    name: &str,
    (from, floor): (u64, u64),
    closed: bool,
    salt: &str,
    out: &mut Out,
    scrub: &mut Scrub,
) -> u64 {
    let mut lines = Vec::new();
    db.replay(name, |ts, msg| lines.push((ts, msg.to_string())));
    scrub.reset();
    let mut skip = sink();
    let mut derived = Derived::new();
    let mut here: HashSet<String> = HashSet::new();
    let mut inside = false;
    let mut marked = false;
    for (n, (ts, msg)) in lines.iter().enumerate() {
        let n = n as u64;
        let ts = *ts;
        let live = n >= from;
        if live && out.full() {
            out.flush();
            return n;
        }
        if live && !marked {
            marked = true;
            out.push(ts, "log", &format!("{}\t{from}", hash(&[salt, name], 1)));
        }
        skip.text.clear();
        let out: &mut Out = if live { &mut *out } else { &mut skip };
        if n >= floor {
            derived.open();
        }
        derived.gs.feed_msg(ts, msg);
        derived.emit(ts, out, scrub);
        let Some(ev) = parse_msg(msg) else {
            continue;
        };
        let count = here.len();
        let was_inside = inside;
        match &ev {
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
        if !inside && !(was_inside && matches!(ev, Event::RoomLeft)) {
            if matches!(ev, Event::RoomEnter(_)) {
                out.push(ts, "room_enter", "other");
            }
            continue;
        }
        if let Event::EnemyRetire(slot) = ev {
            out.retire(ts, slot);
        } else if let Some((tag, rest)) = fields(&ev, scrub) {
            out.push(ts, tag, &rest);
        }
        if here.len() != count && !here.is_empty() {
            out.push(ts, "players", &here.len().to_string());
        }
    }
    let end = lines.len() as u64;
    if closed {
        let ts = lines.last().map_or(0, |l| l.0);
        if from <= end && !out.full() {
            if !marked {
                out.push(ts, "log", &format!("{}\t{from}", hash(&[salt, name], 1)));
            }
            derived.open();
            derived.gs.log_rotated();
            derived.emit(ts, out, scrub);
            out.push(ts, "log_end", &end.to_string());
            out.flush();
            return end + 1;
        }
        out.flush();
        return end;
    }
    out.flush();
    end
}

pub fn build(db: &History, version: &str, limit: usize) -> Option<Batch> {
    let salt = db.salt();
    db.snapshot(|| build_from(db, version, limit, salt))
}

fn build_from(db: &History, version: &str, limit: usize, salt: String) -> Option<Batch> {
    let cursors = db.usage_cursors();
    let floors = db.usage_floors();
    if !cursors.contains_key(PARTY) {
        return None;
    }
    let newest_known = cursors
        .keys()
        .filter(|k| k.as_str() != PARTY && k.as_str() != PARTY_AT)
        .max()
        .cloned();
    let ends = db.log_ends();
    let newest = ends.last().map(|(n, _)| n.clone());
    let (me, others) = db.known_names();
    let mut scrub = Scrub::new(&me, &others);
    scrub.salt.clone_from(&salt);
    let mut out = Out {
        text: String::new(),
        prev: None,
        first: None,
        limit,
        retire: None,
    };
    let mut moved = Vec::new();
    for (name, end) in ends {
        let closed = newest.as_ref() != Some(&name);
        let end = end + u64::from(closed);
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
        let floor = floors.get(&name).copied().unwrap_or(0);
        let to = log_lines(
            db,
            &name,
            (from, floor),
            closed,
            &salt,
            &mut out,
            &mut scrub,
        );
        moved.push((name, to));
    }
    let party_from = cursors[PARTY];
    let mut party_to = party_from;
    let mut party_at = 0;
    let mut party = String::new();
    if !out.full() {
        for (seq, rec) in db.party_saved_after(party_from, MAX_PARTY) {
            if out.text.len() + party.len() >= limit {
                break;
            }
            party_to = seq;
            party_at = party_at.max(rec.at);
            out.first.get_or_insert(rec.at);
            if let Some(line) = party_line(seq, &rec, &mut scrub) {
                party.push_str(&line);
                party.push('\n');
            }
        }
    }
    if party_to != party_from {
        moved.push((PARTY.to_string(), party_to));
        moved.push((PARTY_AT.to_string(), party_at));
    }
    if moved.is_empty() {
        return None;
    }
    out.text.push_str(&party);
    let Some(first) = out.first else {
        return Some(Batch {
            text: String::new(),
            cursors: moved,
        });
    };
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
    let id = hash(&[&salt, &out.text], 2);
    let head = format!(
        "v2\t{version}\t{id}\t{}\t{}\t{}\n",
        day(first),
        unix(first),
        utc_offset_at(first)
    );
    Some(Batch {
        text: head + &out.text,
        cursors: moved,
    })
}

pub fn compress(text: &str) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(text.as_bytes(), 6)
}

pub fn next(db: &History, version: &str) -> Option<(Batch, Vec<u8>)> {
    let mut limit = MAX_TEXT;
    loop {
        let batch = build(db, version, limit)?;
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
            *pending = next(db, version);
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
