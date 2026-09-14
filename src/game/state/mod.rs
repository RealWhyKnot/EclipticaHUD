use crate::game::event::{parse_msg, split_line};
use crate::game::run::{
    base_name, BossFight, KillSummary, Run, StageStats, TakenEntry, Tally, TargetEntry,
};
use std::collections::{HashSet, VecDeque};

mod apply;

#[derive(Default, Debug, Clone, PartialEq)]
pub enum Mode {
    #[default]
    Idle,
    Lobby,
    Intermission,
    Stage,
}

pub const DEFAULT_WINDOW: u64 = 10;
const WAVE_IDLE_SECS: u64 = 20;

#[derive(Default)]
pub struct GameState {
    bosses: HashSet<String>,
    pub mode: Mode,
    pub stage: String,
    pub progress: f32,
    pub class: String,
    pub stage_no: Option<u32>,
    pub boss: Option<String>,
    pub fight_start: u64,
    pub fight_dmg: u64,
    pub target: Option<String>,
    pub target_since: u64,
    pub history: VecDeque<TargetEntry>,
    pub targets_total: u64,
    pub taken: VecDeque<TakenEntry>,
    pub deaths_log: VecDeque<(u64, u64)>,
    pub fight_taken: u64,
    pub fight_hits: u32,
    pub fight_max_hit: u64,
    pub fight_attacks: Vec<Tally>,
    taken_hits: VecDeque<(u64, u64)>,
    pub last_kill: Option<KillSummary>,
    pub runs: Vec<Run>,
    pub level_tokens: Vec<(bool, u32)>,
    pending_tokens: Vec<(bool, u32)>,
    pub tokens_got: u32,
    last_save: Option<u64>,
    stage_boss_seen: bool,
    wave_last_activity: u64,
    pub location: Option<String>,
    pub world: Option<String>,
    pub window: u64,
    pub stage_stats: StageStats,
    last_death: Option<u64>,
    hits: VecDeque<(u64, u64)>,
    pending_kill: Option<KillSummary>,
    dead_seen: Option<(String, u64)>,
    dead_until: u64,
    last_ts: u64,
    seq: u64,
    pub changed: bool,
}

impl GameState {
    fn open_run(&mut self, ts: u64) -> &mut Run {
        if self.runs.last().is_none_or(|r| r.end_ts.is_some()) {
            self.last_kill = None;
            self.runs.push(Run {
                start_ts: ts,
                ..Default::default()
            });
        }
        self.runs.last_mut().unwrap()
    }

    fn open_fight(&mut self) -> Option<&mut BossFight> {
        self.runs
            .last_mut()
            .filter(|r| r.end_ts.is_none())
            .and_then(|r| r.fights.last_mut())
            .filter(|f| f.end_ts.is_none())
    }

    pub fn live_run(&self) -> Option<&Run> {
        self.runs.last().filter(|r| r.end_ts.is_none())
    }

    pub fn in_ecliptica(&self) -> bool {
        self.world
            .as_deref()
            .is_some_and(|w| w.starts_with("Ecliptica"))
    }

    pub fn pre_boss(&self) -> bool {
        self.mode == Mode::Stage && self.boss.is_none() && self.stage_stats.start_ts != 0
    }

    pub fn stage_dps(&self, now: u64) -> u64 {
        self.stage_stats.dmg / now.saturating_sub(self.stage_stats.start_ts).max(1)
    }

    pub fn stage_taken_rate(&self, now: u64) -> u64 {
        self.stage_stats.taken / now.saturating_sub(self.stage_stats.start_ts).max(1)
    }

    pub fn wave_idle(&self, now: u64) -> bool {
        self.pre_boss() && now.saturating_sub(self.wave_last_activity) >= WAVE_IDLE_SECS
    }

    pub fn tokens_missing(&self) -> Option<(u32, u32)> {
        self.tokens_shown().filter(|(got, total)| got < total)
    }

    pub fn tokens_shown(&self) -> Option<(u32, u32)> {
        if self.mode != Mode::Stage || self.level_tokens.is_empty() {
            return None;
        }
        let total = self.level_tokens.len() as u32;
        Some((self.tokens_got.min(total), total))
    }

    pub fn is_dead(&self, now: u64) -> bool {
        now < self.dead_until
    }

    pub fn feed(&mut self, raw: &str) -> Option<u64> {
        let line = split_line(raw)?;
        self.last_ts = self.last_ts.max(line.ts);
        if let Some(ev) = parse_msg(line.msg) {
            self.apply(line.ts, ev);
        }
        Some(line.ts)
    }

    pub fn last_kill_total(&self) -> Option<(String, u64, u64)> {
        let k = self.last_kill.as_ref()?;
        let run = self.live_run()?;
        let idx = run
            .fights
            .iter()
            .rposition(|f| f.name == k.boss && f.kill.is_some())?;
        let group = run.groups().into_iter().find(|g| g.contains(&idx))?;
        let (strike, other) = run.fights[group]
            .iter()
            .filter_map(|f| f.kill)
            .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
        Some((base_name(&k.boss).to_string(), strike, other))
    }

    pub fn win(&self) -> u64 {
        if self.window == 0 {
            DEFAULT_WINDOW
        } else {
            self.window
        }
    }

    fn windowed(deque: &mut VecDeque<(u64, u64)>, now: u64, window: u64) -> u64 {
        while deque.front().is_some_and(|h| h.0 + window <= now) {
            deque.pop_front();
        }
        let span = deque
            .front()
            .map_or(1, |h| (now.saturating_sub(h.0) + 1).clamp(1, window));
        deque.iter().map(|h| h.1).sum::<u64>() / span
    }

    pub fn rolling_dps(&mut self, now: u64) -> u64 {
        let window = self.win();
        Self::windowed(&mut self.hits, now, window)
    }

    pub fn fight_dps(&self, now: u64) -> u64 {
        if self.boss.is_none() {
            return 0;
        }
        self.fight_dmg / now.saturating_sub(self.fight_start).max(1)
    }

    pub fn rolling_taken(&mut self, now: u64) -> u64 {
        let window = self.win();
        Self::windowed(&mut self.taken_hits, now, window)
    }

    pub fn fight_taken_rate(&self, now: u64) -> u64 {
        if self.boss.is_none() {
            return 0;
        }
        self.fight_taken / now.saturating_sub(self.fight_start).max(1)
    }
}

#[cfg(test)]
mod tests;
