use crate::game::event::civil;
use crate::history::History;
use std::fmt::Write;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

mod lines;
mod scrub;

use lines::{Out, day, log_lines, party_line, unix};
use scrub::Scrub;

pub(crate) const MAX_TEXT: usize = 2 * 1024 * 1024;
pub(crate) const MAX_BODY: usize = 240 * 1024;
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

pub(crate) static ON: AtomicBool = AtomicBool::new(false);
pub(crate) static FAILING: AtomicBool = AtomicBool::new(false);

pub(crate) struct Batch {
    pub text: String,
    pub cursors: Vec<(String, u64)>,
}

pub(crate) fn utc_unix(ts: u64) -> u64 {
    (unix(ts).cast_signed() - utc_offset_at(ts) * 60)
        .max(0)
        .cast_unsigned()
}

fn sha256(data: &[u8]) -> Option<[u8; 32]> {
    use windows_sys::Win32::Security::Cryptography::{BCRYPT_SHA256_ALG_HANDLE, BCryptHash};
    let mut out = [0u8; 32];
    // SAFETY: the input and output pointers and lengths describe `data` and `out`, which outlive the call.
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

pub(crate) fn hash(parts: &[&str], n: usize) -> String {
    let mut data = Vec::new();
    for p in parts {
        data.extend_from_slice(&crate::num::to_u64(p.len()).to_le_bytes());
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

pub(crate) fn utc_offset_at(ts: u64) -> i64 {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::Time::TzSpecificLocalTimeToSystemTime;
    if ts < MIN_TS {
        return 0;
    }
    let (y, m, d) = civil(ts);
    let secs = ts % 86_400;
    let local = SYSTEMTIME {
        wYear: crate::num::to_u16(y),
        wMonth: crate::num::to_u16(m),
        wDayOfWeek: 0,
        wDay: crate::num::to_u16(d),
        wHour: crate::num::to_u16(secs / 3600),
        wMinute: crate::num::to_u16(secs / 60 % 60),
        wSecond: crate::num::to_u16(secs % 60),
        wMilliseconds: 0,
    };
    let mut utc = SYSTEMTIME::default();
    // SAFETY: both pointers are live SYSTEMTIME values, and a null time zone means the current one.
    if unsafe { TzSpecificLocalTimeToSystemTime(std::ptr::null(), &raw const local, &raw mut utc) }
        == 0
    {
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

pub(crate) fn build(db: &History, version: &str, limit: usize) -> Option<Batch> {
    let salt = db.salt();
    db.snapshot(|| build_from(db, version, limit, &salt))
}

fn build_from(db: &History, version: &str, limit: usize, salt: &str) -> Option<Batch> {
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
    salt.clone_into(&mut scrub.salt);
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
        let to = log_lines(db, &name, (from, floor), closed, salt, &mut out, &mut scrub);
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
    Some(sealed(version, salt, &out, moved))
}

fn sealed(version: &str, salt: &str, out: &Out, moved: Vec<(String, u64)>) -> Batch {
    let Some(first) = out.first else {
        return Batch {
            text: String::new(),
            cursors: moved,
        };
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
    let id = hash(&[salt, &out.text], 2);
    let head = format!(
        "v2\t{version}\t{id}\t{}\t{}\t{}\n",
        day(first),
        unix(first),
        utc_offset_at(first)
    );
    Batch {
        text: head + &out.text,
        cursors: moved,
    }
}

pub(crate) fn compress(text: &str) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(text.as_bytes(), 6)
}

pub(crate) fn next(db: &History, version: &str) -> Option<(Batch, Vec<u8>)> {
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
pub(crate) enum Sent {
    Kept,
    Dropped,
    Retry,
}

pub(crate) fn outcome(status: Option<u32>) -> Sent {
    match status {
        Some(200..=299) => Sent::Kept,
        Some(400 | 413) => Sent::Dropped,
        _ => Sent::Retry,
    }
}

pub(crate) fn pass(
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

pub(crate) fn spawn(path: Option<PathBuf>) {
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
mod tests;
