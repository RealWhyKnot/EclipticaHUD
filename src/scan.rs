use crate::game::event::{fmt_clock, parse_msg, split_line};
use crate::game::names::{boss_name, stage_name};
use crate::game::run::{base_name, phase_num, Run, RunEnd, StageStats};
use crate::game::source::describe_source;
use crate::game::state::{GameState, Mode};
use crate::vrchat::log::{all_logs, log_dir, read_lines};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub fn scan(path: Option<String>) {
    let paths: Vec<PathBuf> = match path {
        Some(p) => vec![p.into()],
        None => log_dir().map(|d| all_logs(&d)).unwrap_or_default(),
    };
    if paths.is_empty() {
        eprintln!("no VRChat log found");
        std::process::exit(1);
    }
    let mut gs = GameState::default();
    let mut seen = Seen::default();
    let mut counts = BTreeMap::new();
    let mut last = 0;
    for (i, path) in paths.iter().enumerate() {
        println!("=== {}", path.display());
        let read = read_lines(path, |raw| {
            let Some(line) = split_line(raw) else {
                return;
            };
            if let Some(ev) = parse_msg(line.msg) {
                *counts.entry(ev.label()).or_insert(0u64) += 1;
            }
            gs.feed(raw);
            last = line.ts;
            seen.observe(&mut gs, line.ts);
        });
        if let Err(e) = read {
            eprintln!("cannot read {}: {e}", path.display());
            continue;
        }
        if i + 1 < paths.len() {
            gs.log_rotated();
            seen.observe(&mut gs, last);
        }
    }
    println!("=== events");
    for (label, n) in counts {
        println!("{label}: {n}");
    }
    println!("=== runs");
    for (i, r) in gs.runs.iter().enumerate() {
        println!("{}", run_line(i, r, r.end_ts.unwrap_or(last)));
    }
}

#[derive(Default)]
struct Seen {
    world: Option<String>,
    mode: Mode,
    runs: usize,
    run: Option<RunSeen>,
    stage: String,
    stage_start: u64,
    clear: Option<u64>,
    tokens: Option<(u32, u32)>,
    alerts: u64,
    targets: u64,
    taken_seq: u64,
    peak: u64,
}

#[derive(Default)]
struct RunSeen {
    idx: usize,
    stages: usize,
    fights: Vec<(bool, bool)>,
    deaths: u32,
    over: bool,
}

impl Seen {
    fn observe(&mut self, gs: &mut GameState, ts: u64) {
        if !std::mem::take(&mut gs.changed) {
            return;
        }
        let t = fmt_clock(ts);
        if gs.boss.is_some() {
            self.peak = self.peak.max(gs.rolling_dps(ts));
        }
        if gs.world != self.world {
            println!("{t}  world    {}", gs.world.as_deref().unwrap_or("left"));
            self.world = gs.world.clone();
        }
        if gs.runs.len() > self.runs {
            self.runs = gs.runs.len();
            self.run = Some(RunSeen {
                idx: self.runs - 1,
                ..Default::default()
            });
            println!("{t}  run {}   start", self.runs);
        }
        if gs.stage_stats.start_ts != self.stage_start {
            self.stage_start = gs.stage_stats.start_ts;
            if self.stage_start != 0 {
                self.stage = gs.stage.clone();
                let tokens = gs
                    .tokens_shown()
                    .map_or("no token lines".to_string(), |(_, n)| format!("{n} tokens"));
                println!(
                    "{t}  stage {}  {}  {}  {tokens}",
                    gs.stage_no.unwrap_or(0),
                    stage_name(&gs.stage),
                    gs.class
                );
            }
            self.tokens = gs.tokens_shown();
            self.clear = None;
        }
        if let Some(rs) = self.run.as_mut() {
            let r = &gs.runs[rs.idx];
            for s in &r.stages[rs.stages..] {
                println!(
                    "{t}  cleared  {}  {}",
                    stage_name(&self.stage),
                    stage_line(s)
                );
            }
            rs.stages = r.stages.len();
            while rs.fights.len() < r.fights.len() {
                let f = &r.fights[rs.fights.len()];
                match phase_num(&f.name) {
                    1 => println!("{t}  boss     {}", boss_name(&f.name)),
                    n => println!("{t}  phase {n}  {}", boss_name(base_name(&f.name))),
                }
                rs.fights.push((false, false));
                self.peak = 0;
            }
            for (f, (ended, killed)) in r.fights.iter().zip(rs.fights.iter_mut()) {
                if let Some(end) = f.end_ts.filter(|_| !*ended) {
                    *ended = true;
                    let secs = f.secs(end);
                    println!(
                        "{t}  down     {}  {secs}s  dmg {}  DPS {}  peak {}  taken {} in {} hits  deaths {}",
                        f.name,
                        f.dmg,
                        f.dmg / secs.max(1),
                        self.peak,
                        f.taken,
                        f.hits,
                        f.deaths
                    );
                }
                if let Some((strike, other)) = f.kill.filter(|_| !*killed) {
                    *killed = true;
                    println!("{t}  kill     {}  {strike} strike + {other} other", f.name);
                }
            }
            if r.deaths > rs.deaths {
                rs.deaths = r.deaths;
                let place = gs.boss.as_deref().unwrap_or(stage_name(&gs.stage));
                println!("{t}  death    #{}  {place}", r.deaths);
            }
            if let Some(end) = r.end_ts.filter(|_| !rs.over) {
                rs.over = true;
                println!("{t}  {}", run_line(rs.idx, r, end));
                for g in (0..r.groups().len()).filter_map(|i| r.group(i)) {
                    println!(
                        "            {}  {}  {}s  dmg {}  DPS {}  phases {}",
                        boss_name(g.name),
                        g.result(),
                        g.duration(end),
                        g.dmg,
                        g.dps(end),
                        g.n_phases
                    );
                }
            }
        }
        let clear = gs.stage_clear();
        if clear != self.clear {
            match clear {
                Some(c) => println!(
                    "{t}  clear    +{}s  dmg {}  DPS {}  taken {} in {} hits",
                    gs.stage_secs(c),
                    gs.stage_stats.dmg,
                    gs.stage_dps(c),
                    gs.stage_stats.taken,
                    gs.stage_stats.hits
                ),
                None if gs.pre_boss() => println!("{t}  spawn    enemies back, clearing resumed"),
                None => {}
            }
            self.clear = clear;
        }
        let tokens = gs.tokens_shown();
        if tokens != self.tokens {
            if let Some((got, total)) = tokens.filter(|_| self.tokens.is_some()) {
                println!("{t}  token    {got}/{total}");
            }
            self.tokens = tokens;
        }
        if gs.token_alerts != self.alerts {
            self.alerts = gs.token_alerts;
            let (got, total) = gs.tokens_shown().unwrap_or((0, 0));
            println!("{t}  ALERT    enemies clear, tokens {got}/{total}");
        }
        if gs.mode != self.mode {
            self.mode = gs.mode.clone();
            match self.mode {
                Mode::Intermission => println!("{t}  intermission"),
                Mode::Lobby => println!("{t}  lobby"),
                _ => {}
            }
        }
        if gs.targets_total > self.targets {
            self.targets = gs.targets_total;
            if let Some(h) = gs.history.back() {
                println!("{t}  target   {}  <-  {}", h.player, h.boss);
            }
        }
        if let Some(hit) = gs.taken.back().filter(|h| h.seq != self.taken_seq) {
            self.taken_seq = hit.seq;
            let (who, attack) = describe_source(&hit.source);
            println!("{t}  hit      {:>4}  {who}  {attack}", hit.amount);
        }
    }
}

fn stage_line(s: &StageStats) -> String {
    let secs = s.secs(s.start_ts);
    format!(
        "{secs}s  dmg {}  DPS {}  taken {} in {} hits  deaths {}",
        s.dmg,
        s.dmg / secs.max(1),
        s.taken,
        s.hits,
        s.deaths
    )
}

fn run_line(i: usize, r: &Run, now: u64) -> String {
    let how = match r.end {
        Some(RunEnd::Lost) => format!(
            "LOST at {}",
            r.fights
                .last()
                .map_or("?", |f| boss_name(base_name(&f.name)))
        ),
        Some(RunEnd::Won) => "WON".to_string(),
        Some(RunEnd::Lobby) => "ENDED in lobby".to_string(),
        Some(RunEnd::Left) => "LEFT".to_string(),
        None => "LIVE".to_string(),
    };
    format!(
        "run {:>2}  {}  {how}  last stage {} ({})  {}s  stages {}  bosses {}  dmg {}  DPS {}  deaths {}",
        i + 1,
        fmt_clock(r.start_ts),
        r.stage_no.unwrap_or(0),
        stage_name(&r.stage),
        now.saturating_sub(r.start_ts),
        r.stages.len(),
        r.groups().len(),
        r.dmg(),
        r.dmg() / r.active_secs(now).max(1),
        r.deaths
    )
}
