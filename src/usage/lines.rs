use super::scrub::Scrub;
use super::{MAX_PACK, MAX_ROWS, MIN_TS, UNIX_DAYS, hash};
use crate::game::event::{Event, civil, parse_msg};
use crate::game::run::{Run, RunEnd};
use crate::game::state::GameState;
use crate::history::{History, PartyRecord};
use std::collections::HashSet;
use std::fmt::Write;

pub(super) fn float(f: f32) -> Option<String> {
    if !f.is_finite() || f < 0.0 {
        return None;
    }
    let s = format!("{f:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    (s.len() <= 12).then(|| s.to_string())
}

pub(super) fn world(location: &str, salt: &str) -> Option<String> {
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

pub(super) fn party_line(seq: u64, rec: &PartyRecord, scrub: &mut Scrub) -> Option<String> {
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

pub(super) fn day(ts: u64) -> String {
    if ts < MIN_TS {
        return "0000-00-00".to_string();
    }
    let (y, m, d) = civil(ts);
    format!("{y:04}-{m:02}-{d:02}")
}

pub(super) fn unix(ts: u64) -> u64 {
    ts.saturating_sub(UNIX_DAYS * 86_400)
}

pub(super) struct Out {
    pub(super) text: String,
    pub(super) prev: Option<u64>,
    pub(super) first: Option<u64>,
    pub(super) limit: usize,
    pub(super) retire: Option<(u64, Vec<u32>)>,
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

    pub(super) fn full(&self) -> bool {
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

pub(super) fn log_lines(
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
        let n = crate::num::to_u64(n);
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
        if !(inside || was_inside && matches!(ev, Event::RoomLeft)) {
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
    let end = crate::num::to_u64(lines.len());
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
