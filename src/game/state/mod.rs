use crate::game::event::{parse_msg, split_line, Event};
use crate::game::run::{
    base_name, continues, phase_num, tally, BossFight, KillSummary, Run, StageStats, TakenEntry,
    Tally, TargetEntry, KILL_DEDUPE_SECS,
};
use crate::game::source::describe_source;
use std::collections::{HashSet, VecDeque};

#[derive(Default, Debug, Clone, PartialEq)]
pub enum Mode {
    #[default]
    Idle,
    Lobby,
    Intermission,
    Stage,
}

pub const DEFAULT_WINDOW: u64 = 10;
const DEATH_HOLD: u64 = 3;
const WIPE_SECS: u64 = 3;
const LOG_CAP: usize = 500;
const BOSS_SAVE_LEAD: u64 = 8;
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

    fn close_stage(&mut self, ts: u64) {
        if self.stage_stats.start_ts == 0 {
            return;
        }
        let mut done = std::mem::take(&mut self.stage_stats);
        done.end_ts = Some(ts);
        if let Some(r) = self.runs.last_mut().filter(|r| r.end_ts.is_none()) {
            r.stages.push(done);
        }
    }

    fn end_run(&mut self, ts: u64) {
        self.close_stage(ts);
        let wiped = self
            .last_death
            .is_some_and(|d| ts.saturating_sub(d) <= WIPE_SECS);
        if let Some(r) = self.runs.last_mut().filter(|r| r.end_ts.is_none()) {
            if let Some(f) = r.fights.last_mut() {
                let recent = f.end_ts.is_none_or(|e| ts.saturating_sub(e) <= WIPE_SECS);
                f.end_ts.get_or_insert(ts);
                f.lost = wiped && recent;
            }
            r.won = !wiped
                && r.fights
                    .last()
                    .is_some_and(|f| base_name(&f.name) == "JimBringer" && f.kill.is_some());
            r.lost = wiped;
            r.end_ts = Some(ts);
            r.hits = std::mem::take(&mut self.taken);
            r.targets = std::mem::take(&mut self.history);
            r.death_log = std::mem::take(&mut self.deaths_log);
        }
        self.boss = None;
        self.target = None;
        self.pending_kill = None;
        self.dead_until = 0;
        self.last_death = None;
        self.fight_start = 0;
        self.fight_dmg = 0;
        self.fight_taken = 0;
        self.fight_hits = 0;
        self.fight_max_hit = 0;
        self.fight_attacks.clear();
        self.hits.clear();
        self.taken_hits.clear();
        self.taken.clear();
        self.history.clear();
        self.deaths_log.clear();
        self.bosses.clear();
        self.level_tokens.clear();
        self.pending_tokens.clear();
        self.reset_stage_tokens();
        self.stage.clear();
        self.class.clear();
        self.progress = 0.0;
        self.stage_no = None;
    }

    fn leave_world(&mut self, ts: u64) {
        self.end_run(ts);
        self.mode = Mode::Idle;
        self.location = None;
        self.world = None;
    }

    pub fn log_rotated(&mut self) {
        self.leave_world(self.last_ts);
        self.changed = true;
    }

    fn touch_wave(&mut self, ts: u64) {
        if self.mode == Mode::Stage {
            self.wave_last_activity = ts;
        }
    }

    pub fn wave_idle(&self, now: u64) -> bool {
        self.pre_boss() && now.saturating_sub(self.wave_last_activity) >= WAVE_IDLE_SECS
    }

    pub fn tokens_missing(&self) -> Option<(u32, u32)> {
        self.tokens_shown().filter(|(got, total)| got < total)
    }

    fn reset_stage_tokens(&mut self) {
        self.tokens_got = 0;
        self.last_save = None;
        self.stage_boss_seen = false;
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
    pub fn apply(&mut self, ts: u64, ev: Event) {
        self.changed = true;
        self.seq += 1;
        let seq = self.seq;
        match ev {
            Event::BossFight { name } => {
                if self.mode == Mode::Lobby {
                    return;
                }
                self.bosses.insert(name.clone());
                let last = self
                    .runs
                    .last()
                    .filter(|r| r.end_ts.is_none())
                    .and_then(|r| r.fights.last());
                let echo = last.is_some_and(|f| {
                    base_name(&f.name) == base_name(&name)
                        && phase_num(&name) <= phase_num(&f.name)
                        && match f.end_ts {
                            None => true,
                            Some(e) => f.kill.is_some() && ts.saturating_sub(e) <= KILL_DEDUPE_SECS,
                        }
                });
                let transition = last.is_some_and(|f| continues(f, &name, ts));
                if self.boss.as_deref() != Some(&name) && !echo {
                    self.close_stage(ts);
                    self.boss = Some(name.clone());
                    if !self.stage_boss_seen {
                        self.stage_boss_seen = true;
                        if self
                            .last_save
                            .is_some_and(|s| ts.saturating_sub(s) <= BOSS_SAVE_LEAD)
                        {
                            self.tokens_got = self.tokens_got.saturating_sub(1);
                        }
                    }
                    if !transition {
                        self.fight_start = ts;
                        self.fight_dmg = 0;
                        self.fight_taken = 0;
                        self.fight_hits = 0;
                        self.fight_max_hit = 0;
                        self.fight_attacks.clear();
                    }
                    if let Some(f) = self.open_fight() {
                        f.end_ts = Some(ts);
                    }
                    self.open_run(ts).fights.push(BossFight {
                        name,
                        start_ts: ts,
                        end_ts: None,
                        dmg: 0,
                        taken: 0,
                        hits: 0,
                        attacks: Vec::new(),
                        deaths: 0,
                        kill: None,
                        lost: false,
                    });
                }
            }
            Event::BossDead { name } => {
                if let Some(k) = self.pending_kill.take() {
                    self.record_kill(k);
                }
                self.pending_kill = Some(KillSummary {
                    ts,
                    boss: name.clone(),
                    strike: 0,
                    non_strike: 0,
                });
                if self.boss.as_deref() == Some(&name) {
                    self.boss = None;
                    self.target = None;
                    if let Some(f) = self.open_fight().filter(|f| f.name == name) {
                        f.end_ts = Some(ts);
                    }
                }
            }
            Event::StrikeTotal(n) => {
                if let Some(k) = self.pending_kill.as_mut() {
                    k.strike = n;
                }
            }
            Event::NonStrikeTotal(n) => {
                if let Some(mut k) = self.pending_kill.take() {
                    k.non_strike = n;
                    self.record_kill(k);
                }
            }
            Event::Dealt { n, .. } => {
                self.touch_wave(ts);
                self.hits.push_back((ts, n));
                if self.boss.is_some() {
                    self.fight_dmg += n;
                    if let Some(f) = self.open_fight() {
                        f.dmg += n;
                    }
                } else if self.pre_boss() {
                    self.stage_stats.dmg += n;
                }
            }
            Event::DamageTaken { amount, source } => {
                self.touch_wave(ts);
                self.taken_hits.push_back((ts, amount));
                if self.pre_boss() {
                    let s = &mut self.stage_stats;
                    s.taken += amount;
                    s.hits += 1;
                    s.max_hit = s.max_hit.max(amount);
                    let (who, attack) = describe_source(&source, amount);
                    tally(&mut s.attacks, &who, &attack, amount);
                } else if self.boss.is_some() {
                    self.fight_taken += amount;
                    self.fight_hits += 1;
                    self.fight_max_hit = self.fight_max_hit.max(amount);
                    let (who, attack) = describe_source(&source, amount);
                    tally(&mut self.fight_attacks, &who, &attack, amount);
                    if let Some(f) = self.open_fight() {
                        f.taken += amount;
                        f.hits += 1;
                        tally(&mut f.attacks, &who, &attack, amount);
                    }
                }
                self.taken.push_back(TakenEntry {
                    ts,
                    seq,
                    amount,
                    source,
                });
                if self.taken.len() > LOG_CAP {
                    self.taken.pop_front();
                }
            }
            Event::Ownership { object, player } => {
                self.touch_wave(ts);
                if self.bosses.contains(&object) {
                    self.target = Some(player.clone());
                    self.target_since = ts;
                    self.targets_total += 1;
                    self.history.push_back(TargetEntry {
                        ts,
                        seq,
                        player,
                        boss: object,
                    });
                    if self.history.len() > LOG_CAP {
                        self.history.pop_front();
                    }
                }
            }
            Event::Stage {
                name,
                progress,
                class,
            } => {
                self.mode = Mode::Stage;
                self.wave_last_activity = ts;
                if !self.pending_tokens.is_empty() {
                    self.level_tokens = std::mem::take(&mut self.pending_tokens);
                    self.reset_stage_tokens();
                } else if self.stage != name {
                    self.level_tokens.clear();
                    self.reset_stage_tokens();
                }
                let fresh =
                    self.stage_stats.start_ts == 0 || self.stage != name || self.boss.is_some();
                if fresh {
                    self.close_stage(ts);
                    self.stage_stats = StageStats {
                        start_ts: ts,
                        ..Default::default()
                    };
                }
                self.stage = name.clone();
                self.progress = progress;
                if !class.is_empty() {
                    self.class = class;
                }
                let class = self.class.clone();
                let run = self.open_run(ts);
                run.stage = name;
                run.class = class;
                if fresh {
                    run.stage_no = Some(run.stages.len() as u32 + 1);
                }
                self.stage_no = run.stage_no;
            }
            Event::Intermission => {
                self.close_stage(ts);
                self.mode = Mode::Intermission;
                self.boss = None;
                self.target = None;
                if let Some(f) = self.open_fight() {
                    f.end_ts = Some(ts);
                }
            }
            Event::Lobby => {
                self.end_run(ts);
                self.mode = Mode::Lobby;
            }
            Event::RoomLeft => {
                self.leave_world(ts);
            }
            Event::RoomEnter(name) => {
                if !name.starts_with("Ecliptica") {
                    self.leave_world(ts);
                }
                self.world = Some(name);
            }
            Event::TokenSpawn { rune, chance } => {
                self.pending_tokens.push((rune, chance));
            }
            Event::SessionSave => {
                if self.mode == Mode::Stage
                    && !self.level_tokens.is_empty()
                    && !self.stage_boss_seen
                {
                    self.tokens_got += 1;
                    self.last_save = Some(ts);
                }
            }
            Event::RoomJoin(location) => {
                self.location = Some(location);
            }
            Event::EnemyActivity => self.touch_wave(ts),
            Event::PlayerDead => {
                self.last_death = Some(ts);
                if ts >= self.dead_until {
                    if let Some(r) = self.runs.last_mut().filter(|r| r.end_ts.is_none()) {
                        r.deaths += 1;
                    }
                    if self.pre_boss() {
                        self.stage_stats.deaths += 1;
                    }
                    if let Some(f) = self.open_fight() {
                        f.deaths += 1;
                    }
                    self.deaths_log.push_back((ts, seq));
                    if self.deaths_log.len() > 500 {
                        self.deaths_log.pop_front();
                    }
                }
                self.dead_until = ts + DEATH_HOLD;
            }
        }
    }

    fn record_kill(&mut self, k: KillSummary) {
        let chained = self.dead_seen.as_ref().is_some_and(|(boss, ts)| {
            *boss == k.boss && k.ts.saturating_sub(*ts) <= KILL_DEDUPE_SECS
        });
        self.dead_seen = Some((k.boss.clone(), k.ts));
        let dupe = chained
            && self.last_kill.as_ref().is_some_and(|p| {
                p.boss == k.boss && p.strike + p.non_strike >= k.strike + k.non_strike
            });
        if dupe {
            return;
        }
        if let Some(f) = self
            .runs
            .last_mut()
            .filter(|r| r.end_ts.is_none())
            .and_then(|r| {
                r.fights
                    .iter_mut()
                    .rev()
                    .find(|f| f.name == k.boss && f.kill.is_none())
            })
        {
            f.kill = Some((k.strike, k.non_strike));
            f.end_ts.get_or_insert(k.ts);
        }
        self.last_kill = Some(k);
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
mod tests {
    use super::*;
    use crate::game::event::fmt_clock;
    use crate::game::run::merge_tallies;

    const P: &str = "2026.09.07 09:12:28 Debug      -  ";

    #[test]
    fn boss_gating() {
        let mut gs = GameState::default();
        gs.feed(&format!("{P}ownership of Neko1 transferred to Alice"));
        assert!(gs.target.is_none());
        gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1"
        ));
        gs.feed(&format!("{P}ownership of Neko1 transferred to Alice"));
        assert!(gs.target.is_none());
        gs.feed(&format!("{P}ownership of Kakarot transferred to Alice"));
        assert_eq!(gs.target.as_deref(), Some("Alice"));
        assert_eq!(gs.history.len(), 1);
    }

    #[test]
    fn kill_dedupe() {
        let mut gs = GameState::default();
        let kill = |gs: &mut GameState, t: &str, s: u64| {
            gs.feed(&format!(
                "2026.09.07 {t} Debug      -  Boss Kakarot dead, personal damage dealt: "
            ));
            gs.feed(&format!("2026.09.07 {t} Debug      -  STRIKE DMG: {s}"));
            gs.feed(&format!("2026.09.07 {t} Debug      -  NON-STRIKE DMG: 0"));
        };
        kill(&mut gs, "09:24:12", 4793);
        kill(&mut gs, "09:24:18", 0);
        let k = gs.last_kill.as_ref().unwrap();
        assert_eq!(k.strike, 4793);
        kill(&mut gs, "09:26:00", 900);
        assert_eq!(gs.last_kill.as_ref().unwrap().strike, 900);
    }

    #[test]
    fn kill_echo_chain_outlives_window() {
        let mut gs = GameState::default();
        let kill = |gs: &mut GameState, secs: u64, s: u64| {
            let t = fmt_clock(3600 + secs);
            gs.feed(&format!(
                "2026.09.08 {t} Debug      -  Boss Gravetender dead, personal damage dealt: "
            ));
            gs.feed(&format!("2026.09.08 {t} Debug      -  STRIKE DMG: {s}"));
            gs.feed(&format!("2026.09.08 {t} Debug      -  NON-STRIKE DMG: 0"));
        };
        kill(&mut gs, 0, 9173);
        let first_ts = gs.last_kill.as_ref().unwrap().ts;
        let mut t = 13;
        while t <= 110 {
            kill(&mut gs, t, 0);
            t += 3;
        }
        let k = gs.last_kill.as_ref().unwrap();
        assert_eq!(k.strike, 9173);
        assert_eq!(k.ts, first_ts);
        kill(&mut gs, 380, 0);
        assert_eq!(gs.last_kill.as_ref().unwrap().strike, 0);
    }

    fn feed_at(gs: &mut GameState, t: &str, msg: &str) -> u64 {
        gs.feed(&format!("2026.09.08 {t} Debug      -  {msg}"))
            .unwrap()
    }

    #[test]
    fn run_lifecycle() {
        let mut gs = GameState::default();
        feed_at(
            &mut gs,
            "08:18:01",
            "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer",
        );
        assert_eq!(gs.runs.len(), 1);
        assert!(gs.runs[0].end_ts.is_none());
        feed_at(
            &mut gs,
            "08:19:54",
            "ECLIPTICA - now fighting boss: DarkMouth(Clone) on phase: 0",
        );
        feed_at(&mut gs, "08:19:55", "Dealing 100 STRIKE damage");
        feed_at(
            &mut gs,
            "08:20:10",
            "Boss DarkMouth dead, personal damage dealt: ",
        );
        feed_at(&mut gs, "08:20:10", "STRIKE DMG: 4100");
        feed_at(&mut gs, "08:20:10", "NON-STRIKE DMG: 55");
        feed_at(&mut gs, "08:20:49", "ECLIPTICA - now in lobby");
        let run = &gs.runs[0];
        assert!(run.end_ts.is_some());
        assert!(run.fights.last().filter(|f| f.lost).is_none());
        assert_eq!(run.stage, "Hall of Beginnings");
        assert_eq!(run.class, "Spellhammer");
        assert_eq!(run.fights.len(), 1);
        let f = &run.fights[0];
        assert_eq!(f.dmg, 100);
        assert_eq!(f.kill, Some((4100, 55)));
        assert!(f.end_ts.is_some());
        feed_at(
            &mut gs,
            "08:21:25",
            "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Nekomancer",
        );
        assert_eq!(gs.runs.len(), 2);
        assert_eq!(gs.runs[1].class, "Nekomancer");
    }

    #[test]
    fn room_left_ends_run() {
        let mut gs = GameState::default();
        feed_at(
            &mut gs,
            "08:29:14",
            "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Twinmage",
        );
        feed_at(
            &mut gs,
            "08:30:00",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0",
        );
        feed_at(&mut gs, "08:30:52", "[Behaviour] OnLeftRoom");
        assert_eq!(gs.mode, Mode::Idle);
        assert!(gs.boss.is_none());
        let run = &gs.runs[0];
        assert!(run.end_ts.is_some());
        assert!(run.fights[0].end_ts.is_some());
        assert_eq!(run.fights[0].kill, None);
    }

    #[test]
    fn rotation_ends_run() {
        let mut gs = GameState::default();
        feed_at(
            &mut gs,
            "08:29:14",
            "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Twinmage",
        );
        feed_at(
            &mut gs,
            "08:29:20",
            "damage has been taken: 5, from source: x",
        );
        let last = gs.feed(
            "2026.09.08 08:30:00 Debug      -  ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0",
        ).unwrap();
        gs.log_rotated();
        assert_eq!(gs.mode, Mode::Idle);
        assert!(gs.boss.is_none());
        assert!(gs.taken.is_empty());
        let run = &gs.runs[0];
        assert_eq!(run.end_ts, Some(last));
        assert_eq!(run.fights[0].end_ts, Some(last));
    }

    #[test]
    fn boss_flap_after_kill_ignored() {
        let mut gs = GameState::default();
        feed_at(
            &mut gs,
            "10:55:34",
            "ECLIPTICA - now fighting boss: AntKing(Clone) on phase: 0.9562449",
        );
        feed_at(
            &mut gs,
            "10:58:36",
            "Boss AntKing dead, personal damage dealt: ",
        );
        feed_at(&mut gs, "10:58:36", "STRIKE DMG: 23681");
        feed_at(&mut gs, "10:58:36", "NON-STRIKE DMG: 0");
        feed_at(
            &mut gs,
            "10:58:36",
            "ECLIPTICA - now fighting boss: AntKingPhase2(Clone) on phase: 0.9562449",
        );
        feed_at(
            &mut gs,
            "10:58:36",
            "ECLIPTICA - now fighting boss: AntKing(Clone) on phase: 0.9562449",
        );
        feed_at(
            &mut gs,
            "10:58:36",
            "ECLIPTICA - now fighting boss: AntKingPhase2(Clone) on phase: 0.9562449",
        );
        assert_eq!(gs.boss.as_deref(), Some("AntKingPhase2"));
        let run = &gs.runs[0];
        assert_eq!(run.fights.len(), 2);
        assert_eq!(run.fights[0].kill, Some((23681, 0)));
        assert_eq!(run.groups(), vec![0..2]);
    }

    #[test]
    fn stray_phase1_before_kill_triple_ignored() {
        let mut gs = GameState::default();
        feed_at(
            &mut gs,
            "05:38:15",
            "ECLIPTICA - now fighting boss: Bravera(Clone) on phase: 0.9",
        );
        feed_at(
            &mut gs,
            "05:42:52",
            "ECLIPTICA - now fighting boss: BraveraPhase2(Clone) on phase: 0.9",
        );
        feed_at(
            &mut gs,
            "05:42:53",
            "ECLIPTICA - now fighting boss: Bravera(Clone) on phase: 0.9",
        );
        feed_at(
            &mut gs,
            "05:42:53",
            "Boss Bravera dead, personal damage dealt: ",
        );
        feed_at(&mut gs, "05:42:53", "STRIKE DMG: 18620");
        feed_at(&mut gs, "05:42:53", "NON-STRIKE DMG: 0");
        assert_eq!(gs.boss.as_deref(), Some("BraveraPhase2"));
        let run = &gs.runs[0];
        assert_eq!(run.fights.len(), 2);
        assert_eq!(run.fights[0].kill, Some((18620, 0)));
        assert_eq!(run.groups(), vec![0..2]);
    }

    #[test]
    fn boss_flap_after_lobby_ignored() {
        let mut gs = GameState::default();
        feed_at(
            &mut gs,
            "10:34:13",
            "ECLIPTICA - now in stage: Stage_Bringer on phase: 1 as class: Spellhammer",
        );
        feed_at(
            &mut gs,
            "10:56:27",
            "ECLIPTICA - now fighting boss: JimBringerPhase3(Clone) on phase: 1",
        );
        feed_at(
            &mut gs,
            "11:12:46",
            "Boss JimBringerPhase3 dead, personal damage dealt: ",
        );
        feed_at(&mut gs, "11:12:46", "STRIKE DMG: 21059");
        feed_at(&mut gs, "11:12:46", "NON-STRIKE DMG: 0");
        feed_at(&mut gs, "11:12:46", "ECLIPTICA - now in lobby");
        feed_at(
            &mut gs,
            "11:12:46",
            "ECLIPTICA - now fighting boss: JimBringerPhase3(Clone) on phase: 0",
        );
        assert_eq!(gs.runs.len(), 1);
        assert!(gs.boss.is_none());
        assert_eq!(gs.mode, Mode::Lobby);
    }

    #[test]
    fn token_spawns_per_level() {
        let mut gs = GameState::default();
        for _ in 0..3 {
            feed_at(&mut gs, "08:18:01", "spawn token, False, 0");
        }
        feed_at(
            &mut gs,
            "08:18:01",
            "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer",
        );
        assert_eq!(gs.level_tokens, vec![(false, 0); 3]);
        feed_at(
            &mut gs,
            "08:19:00",
            "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer",
        );
        assert_eq!(gs.level_tokens.len(), 3);
        feed_at(&mut gs, "08:46:50", "spawn token, True, 35");
        feed_at(&mut gs, "08:46:50", "spawn token, True, 55");
        feed_at(
            &mut gs,
            "08:46:50",
            "ECLIPTICA - now in stage: Stage_GMFuncFlat on phase: 0.06 as class: Spellhammer",
        );
        assert_eq!(gs.level_tokens, vec![(true, 35), (true, 55)]);
        feed_at(&mut gs, "08:50:00", "ECLIPTICA - now in lobby");
        assert!(gs.level_tokens.is_empty());
    }

    const HALL: &str =
        "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer";
    const SAVE: &str = "ECLIPTICA saving SESSION ID 2505";

    fn spawn_level(gs: &mut GameState, t: &str, stage: &str) {
        for _ in 0..3 {
            feed_at(gs, t, "spawn token, False, 0");
        }
        feed_at(gs, t, stage);
    }

    #[test]
    fn tokens_count_session_saves_until_boss() {
        let mut gs = GameState::default();
        assert_eq!(gs.tokens_shown(), None);
        spawn_level(&mut gs, "04:02:02", HALL);
        assert_eq!(gs.tokens_shown(), Some((0, 3)));
        feed_at(&mut gs, "04:02:26", SAVE);
        assert_eq!(gs.tokens_shown(), Some((1, 3)));
        feed_at(&mut gs, "04:02:30", SAVE);
        feed_at(&mut gs, "04:03:24", SAVE);
        assert_eq!(gs.tokens_shown(), Some((3, 3)));
        feed_at(&mut gs, "04:03:53", SAVE);
        assert_eq!(gs.tokens_shown(), Some((3, 3)));
        feed_at(
            &mut gs,
            "04:03:57",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0",
        );
        assert_eq!(gs.tokens_got, 3);
        feed_at(
            &mut gs,
            "04:06:56",
            "Boss Kakarot dead, personal damage dealt: ",
        );
        feed_at(&mut gs, "04:06:58", SAVE);
        assert_eq!(gs.tokens_shown(), Some((3, 3)));
        feed_at(&mut gs, "04:07:04", "ECLIPTICA - now in intermission");
        feed_at(&mut gs, "04:07:27", SAVE);
        assert_eq!(gs.tokens_got, 3);
        assert_eq!(gs.tokens_shown(), None);
        spawn_level(
            &mut gs,
            "04:07:56",
            "ECLIPTICA - now in stage: Stage_GMFuncFlat on phase: 0.06 as class: Spellhammer",
        );
        assert_eq!(gs.tokens_shown(), Some((0, 3)));
    }

    #[test]
    fn skipped_token_and_boss_save() {
        let mut gs = GameState::default();
        spawn_level(&mut gs, "04:28:53", HALL);
        feed_at(&mut gs, "04:29:16", SAVE);
        feed_at(&mut gs, "04:29:19", SAVE);
        feed_at(&mut gs, "04:31:38", SAVE);
        assert_eq!(gs.tokens_shown(), Some((3, 3)));
        feed_at(
            &mut gs,
            "04:31:42",
            "ECLIPTICA - now fighting boss: FlyLord(Clone) on phase: 0.12",
        );
        assert_eq!(gs.tokens_shown(), Some((2, 3)));
        feed_at(
            &mut gs,
            "04:34:35",
            "ECLIPTICA - now fighting boss: FlyLordPhase2(Clone) on phase: 0.12",
        );
        assert_eq!(gs.tokens_shown(), Some((2, 3)));
    }

    #[test]
    fn token_left_long_before_boss_is_kept() {
        let mut gs = GameState::default();
        spawn_level(&mut gs, "05:00:00", HALL);
        feed_at(&mut gs, "05:00:10", SAVE);
        feed_at(
            &mut gs,
            "05:03:00",
            "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
        );
        assert_eq!(gs.tokens_shown(), Some((1, 3)));
    }

    #[test]
    fn location_follows_room_lines() {
        let mut gs = GameState::default();
        feed_at(
            &mut gs,
            "03:59:35",
            "[Behaviour] Joining wrld_0fb88df3-2057-4c2f-8e06-e948864378fd:87887~hidden(usr_4e64b21b-fbd0-4c12-8b55-c8c500b517b1)~region(use)",
        );
        assert!(gs
            .location
            .as_deref()
            .is_some_and(|l| l.ends_with("~region(use)")));
        feed_at(&mut gs, "04:40:00", "[Behaviour] OnLeftRoom");
        assert_eq!(gs.location, None);
    }

    #[test]
    fn stage_counter() {
        let mut gs = GameState::default();
        feed_at(&mut gs, "09:10:01", HALL);
        assert_eq!(gs.stage_no, Some(1));
        assert_eq!(gs.runs.len(), 1);
        assert_eq!(gs.runs[0].stage_no, Some(1));
        feed_at(
            &mut gs,
            "09:12:00",
            "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
        );
        kill_at(&mut gs, "09:15:00", "Nan", 500);
        feed_at(&mut gs, "09:15:10", "ECLIPTICA - now in intermission");
        assert_eq!(gs.stage_no, Some(1));
        const HALLOW: &str =
            "ECLIPTICA - now in stage: Stage_Hallow on phase: 0.1 as class: Spellhammer";
        feed_at(&mut gs, "09:20:00", HALLOW);
        assert_eq!(gs.stage_no, Some(2));
        assert_eq!(gs.runs[0].stage_no, Some(2));
        feed_at(&mut gs, "09:20:30", HALLOW);
        assert_eq!(gs.runs[0].stage_no, Some(2));
        feed_at(&mut gs, "09:30:00", "ECLIPTICA - now in lobby");
        assert!(gs.stage_no.is_none());
        assert_eq!(gs.runs[0].stage_no, Some(2));
        gs.log_rotated();
        assert!(gs.stage_no.is_none());
    }

    #[test]
    fn wave_idle_after_twenty_quiet_seconds() {
        let mut gs = GameState::default();
        spawn_level(&mut gs, "14:13:32", HALL);
        let t = feed_at(
            &mut gs,
            "14:13:40",
            "Initializing Enemy POOL ID1 as ENEMY ID 8",
        );
        assert!(!gs.wave_idle(t + 19));
        assert!(gs.wave_idle(t + 20));
        feed_at(&mut gs, "14:13:45", SAVE);
        assert_eq!(gs.tokens_missing(), Some((1, 3)));
        let t = feed_at(&mut gs, "14:14:10", "Dealing 40 NON-STRIKE damage");
        assert_eq!(gs.stage_stats.dmg, 40);
        assert!(!gs.wave_idle(t + 19));
        assert!(gs.wave_idle(t + 20));
        let t = feed_at(
            &mut gs,
            "14:14:40",
            "damage has been taken: 7, from source: machinegunShooter1",
        );
        assert!(!gs.wave_idle(t + 19));
        let t = feed_at(
            &mut gs,
            "14:14:50",
            "ownership of Crab transferred to WhyKnot",
        );
        assert!(!gs.wave_idle(t + 19));
        assert!(gs.wave_idle(t + 20));
        feed_at(&mut gs, "14:15:20", SAVE);
        feed_at(&mut gs, "14:15:21", SAVE);
        assert_eq!(gs.tokens_missing(), None);
        assert_eq!(gs.tokens_shown(), Some((3, 3)));
        let t = feed_at(
            &mut gs,
            "14:16:26",
            "ECLIPTICA - now fighting boss: NX-Obsidian(Clone) on phase: 0.34",
        );
        assert!(!gs.wave_idle(t + 60));
        feed_at(&mut gs, "14:16:30", "Dealing 25 NON-STRIKE damage");
        feed_at(&mut gs, "14:16:31", "Dealing 30 STRIKE damage");
        assert_eq!(gs.fight_dmg, 55);
        assert_eq!(gs.runs[0].fights[0].dmg, 55);
        kill_at(&mut gs, "14:18:26", "NX-Obsidian", 6625);
        assert!(!gs.wave_idle(t + 180));
        feed_at(&mut gs, "14:18:35", "ECLIPTICA - now in intermission");
        assert!(!gs.wave_idle(t + 200));
        spawn_level(
            &mut gs,
            "14:20:00",
            "ECLIPTICA - now in stage: Stage_ProtoColony on phase: 0.4 as class: Spellhammer",
        );
        let t = gs.stage_stats.start_ts;
        assert_eq!(gs.tokens_missing(), Some((0, 3)));
        assert!(!gs.wave_idle(t + 19));
        assert!(gs.wave_idle(t + 20));
    }

    #[test]
    fn jim_kill_then_lobby_is_a_win() {
        let mut gs = GameState::default();
        feed_at(&mut gs, "09:10:01", HALL);
        feed_at(
            &mut gs,
            "09:12:00",
            "ECLIPTICA - now fighting boss: JimBringer(Clone) on phase: 1",
        );
        feed_at(
            &mut gs,
            "09:14:00",
            "ECLIPTICA - now fighting boss: JimBringerPhase2(Clone) on phase: 1",
        );
        kill_at(&mut gs, "09:16:00", "JimBringerPhase2", 900);
        feed_at(&mut gs, "09:16:01", LOBBY);
        assert!(gs.runs[0].won);
        assert!(!gs.runs[0].lost);
        let mut gs = GameState::default();
        feed_at(&mut gs, "09:10:01", HALL);
        feed_at(
            &mut gs,
            "09:12:00",
            "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
        );
        kill_at(&mut gs, "09:16:00", "Nan", 900);
        feed_at(&mut gs, "09:16:01", LOBBY);
        assert!(!gs.runs[0].won);
    }

    #[test]
    fn boss_without_stage_still_records() {
        let mut gs = GameState::default();
        feed_at(
            &mut gs,
            "09:00:00",
            "ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0",
        );
        assert_eq!(gs.runs.len(), 1);
        assert_eq!(gs.runs[0].fights[0].name, "Yuki");
    }

    #[test]
    fn dps_windows() {
        let mut gs = GameState::default();
        let t0 = split_line(&format!("{P}Dealing 100 STRIKE damage"))
            .unwrap()
            .ts;
        gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1"
        ));
        gs.feed(&format!("{P}Dealing 100 STRIKE damage"));
        gs.feed(&format!("{P}Dealing 50 STRIKE damage"));
        assert_eq!(gs.fight_dmg, 150);
        assert_eq!(gs.rolling_dps(t0), 150);
        assert_eq!(gs.rolling_dps(t0 + 2), 50);
        assert_eq!(gs.rolling_dps(t0 + 9), 15);
        assert_eq!(gs.rolling_dps(t0 + 10), 0);
        assert_eq!(gs.rolling_dps(t0 + 60), 0);
        let mut short = GameState {
            window: 3,
            ..Default::default()
        };
        short.feed(&format!("{P}Dealing 90 STRIKE damage"));
        assert_eq!(short.rolling_dps(t0 + 2), 30);
        assert_eq!(short.rolling_dps(t0 + 3), 0);
        assert_eq!(gs.fight_dps(t0 + 10), 15);
        gs.feed(&format!("{P}Boss Kakarot dead, personal damage dealt: "));
        assert!(gs.boss.is_none());
        assert_eq!(gs.fight_dps(t0 + 10), 0);
    }

    #[test]
    fn boss_fight_resets() {
        let mut gs = GameState::default();
        gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0.6"
        ));
        gs.feed(&format!("{P}Dealing 100 STRIKE damage"));
        gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.6"
        ));
        assert_eq!(gs.fight_dmg, 0);
        assert_eq!(gs.boss.as_deref(), Some("Kakarot"));
    }

    #[test]
    fn phase_transition_keeps_live_stats() {
        let mut gs = GameState::default();
        let t0 = feed_at(
            &mut gs,
            "12:21:33",
            "ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0.6",
        );
        feed_at(&mut gs, "12:21:40", "Dealing 100 STRIKE damage");
        feed_at(
            &mut gs,
            "12:22:45",
            "ECLIPTICA - now fighting boss: YukiPhase2(Clone) on phase: 0.6",
        );
        feed_at(&mut gs, "12:22:50", "Dealing 50 STRIKE damage");
        assert_eq!(gs.fight_dmg, 150);
        assert_eq!(gs.fight_start, t0);
        assert_eq!(gs.boss.as_deref(), Some("YukiPhase2"));
        let run = &gs.runs[0];
        assert_eq!(run.fights.len(), 2);
        assert_eq!(run.groups(), vec![0..2]);
        assert_eq!(run.fights[0].dmg, 100);
        assert_eq!(run.fights[1].dmg, 50);
    }

    #[test]
    fn jimbringer_three_phase_ordering() {
        let mut gs = GameState::default();
        let t0 = feed_at(
            &mut gs,
            "10:34:45",
            "ECLIPTICA - now fighting boss: JimBringer(Clone) on phase: 1",
        );
        feed_at(&mut gs, "10:40:00", "Dealing 500 STRIKE damage");
        feed_at(
            &mut gs,
            "10:46:56",
            "Boss JimBringer dead, personal damage dealt: ",
        );
        feed_at(&mut gs, "10:46:56", "STRIKE DMG: 1000");
        feed_at(&mut gs, "10:46:56", "NON-STRIKE DMG: 0");
        feed_at(
            &mut gs,
            "10:46:57",
            "ECLIPTICA - now fighting boss: JimBringerPhase2(Clone) on phase: 1",
        );
        feed_at(
            &mut gs,
            "10:46:57",
            "ECLIPTICA - now fighting boss: JimBringerPhase2(Clone) on phase: 1",
        );
        feed_at(&mut gs, "10:50:00", "Dealing 700 STRIKE damage");
        feed_at(
            &mut gs,
            "10:56:27",
            "ECLIPTICA - now fighting boss: JimBringerPhase3(Clone) on phase: 1",
        );
        feed_at(
            &mut gs,
            "10:56:27",
            "Boss JimBringerPhase2 dead, personal damage dealt: ",
        );
        feed_at(&mut gs, "10:56:27", "STRIKE DMG: 2000");
        feed_at(&mut gs, "10:56:27", "NON-STRIKE DMG: 0");
        feed_at(&mut gs, "10:57:00", "Dealing 300 STRIKE damage");
        assert_eq!(gs.boss.as_deref(), Some("JimBringerPhase3"));
        assert_eq!(gs.fight_dmg, 1500);
        assert_eq!(gs.fight_start, t0);
        let run = &gs.runs[0];
        assert_eq!(run.fights.len(), 3);
        assert_eq!(run.groups(), vec![0..3]);
        assert_eq!(run.fights[0].kill, Some((1000, 0)));
        assert_eq!(run.fights[1].kill, Some((2000, 0)));
        assert_eq!(run.fights[2].kill, None);
        assert_eq!(run.fights[1].dmg, 700);
        assert_eq!(run.fights[2].dmg, 300);
    }

    #[test]
    fn taken_aggregates_current_fight() {
        let mut gs = GameState::default();
        let t0 = feed_at(
            &mut gs,
            "12:00:00",
            "damage has been taken: 5, from source: attack_Spit",
        );
        assert_eq!(gs.rolling_taken(t0), 5);
        assert_eq!(gs.fight_taken, 0);
        assert_eq!(gs.fight_hits, 0);
        feed_at(
            &mut gs,
            "12:00:10",
            "ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0",
        );
        feed_at(
            &mut gs,
            "12:00:11",
            "damage has been taken: 12, from source: (Yuki) frostBeam",
        );
        feed_at(
            &mut gs,
            "12:00:12",
            "damage has been taken: 8, from source: (Yuki) frostBeam",
        );
        feed_at(
            &mut gs,
            "12:00:13",
            "damage has been taken: 30, from source: attack_Spit (1)",
        );
        let t = feed_at(
            &mut gs,
            "12:00:14",
            "damage has been taken: 10, from source: attack_Spit (2)",
        );
        assert_eq!(gs.rolling_taken(t), 15);
        assert_eq!(gs.fight_taken, 60);
        assert_eq!(gs.fight_hits, 4);
        assert_eq!(gs.fight_max_hit, 30);
        assert_eq!(gs.fight_taken_rate(t + 6), 6);
        let tl = |who: &str, attack: &str, total: u64, hits: u32| Tally {
            who: who.into(),
            attack: attack.into(),
            total,
            hits,
        };
        let expected = vec![tl("Yuki", "Frost Beam", 20, 2), tl("enemy", "Spit", 40, 2)];
        assert_eq!(gs.fight_attacks, expected);
        assert_eq!(gs.runs[0].fights[0].taken, 60);
        assert_eq!(gs.runs[0].fights[0].hits, 4);
        assert_eq!(gs.runs[0].fights[0].attacks, expected);
        feed_at(
            &mut gs,
            "12:01:00",
            "ECLIPTICA - now fighting boss: YukiPhase2(Clone) on phase: 0",
        );
        feed_at(
            &mut gs,
            "12:01:01",
            "damage has been taken: 1, from source: (Yuki) frostBeam",
        );
        assert_eq!(gs.fight_taken, 61);
        assert_eq!(gs.fight_hits, 5);
        assert_eq!(gs.runs[0].fights[1].taken, 1);
        assert_eq!(
            gs.runs[0].fights[1].attacks,
            vec![tl("Yuki", "Frost Beam", 1, 1)]
        );
        let merged = merge_tallies(gs.runs[0].fights.iter().map(|f| f.attacks.as_slice()));
        assert_eq!(
            merged,
            vec![tl("Yuki", "Frost Beam", 21, 3), tl("enemy", "Spit", 40, 2)]
        );
        feed_at(
            &mut gs,
            "12:05:00",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0",
        );
        assert_eq!(gs.fight_taken, 0);
        assert_eq!(gs.fight_hits, 0);
        assert_eq!(gs.fight_max_hit, 0);
        assert!(gs.fight_attacks.is_empty());
        assert_eq!(gs.fight_taken_rate(t + 6), 0);
        assert_eq!(gs.taken.len(), 6);
        let seqs: Vec<u64> = gs.taken.iter().map(|e| e.seq).collect();
        assert!(seqs.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn taken_and_history_cap() {
        let mut gs = GameState::default();
        feed_at(
            &mut gs,
            "12:00:00",
            "ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0",
        );
        for _ in 0..600 {
            feed_at(
                &mut gs,
                "12:00:01",
                "damage has been taken: 1, from source: ",
            );
            feed_at(
                &mut gs,
                "12:00:01",
                "ownership of Yuki transferred to Alice",
            );
        }
        assert_eq!(gs.taken.len(), 500);
        assert_eq!(gs.history.len(), 500);
        assert_eq!(gs.fight_hits, 600);
        gs.log_rotated();
        assert!(gs.taken.is_empty());
        assert!(gs.fight_attacks.is_empty());
    }

    #[test]
    fn death_clusters_count_once() {
        let mut gs = GameState::default();
        feed_at(
            &mut gs,
            "08:18:01",
            "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer",
        );
        feed_at(
            &mut gs,
            "08:19:00",
            "ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0",
        );
        for t in ["08:20:42", "08:20:42", "08:20:43", "08:20:44", "08:20:46"] {
            feed_at(&mut gs, t, "Local controller dead, switching off.");
        }
        let last = feed_at(&mut gs, "08:20:48", "Local controller dead, switching off.");
        assert_eq!(gs.runs[0].deaths, 1);
        assert_eq!(gs.runs[0].fights[0].deaths, 1);
        assert!(gs.is_dead(last));
        assert!(gs.is_dead(last + 2));
        assert!(!gs.is_dead(last + 3));
        feed_at(&mut gs, "08:25:55", "Local controller dead, switching off.");
        assert_eq!(gs.runs[0].deaths, 2);
        assert_eq!(gs.deaths_log.len(), 2);
        assert!(gs.deaths_log[0].1 < gs.deaths_log[1].1);
        assert!(gs.runs[0].fights.last().filter(|f| f.lost).is_none());
        feed_at(&mut gs, "08:25:57", "ECLIPTICA - now in lobby");
        assert!(gs.runs[0].lost);
        assert_eq!(
            gs.runs[0]
                .fights
                .last()
                .filter(|f| f.lost)
                .map(|f| f.name.as_str()),
            Some("Yuki")
        );
        assert_eq!(gs.runs[0].death_log.len(), 2);
        assert!(gs.deaths_log.is_empty());
    }

    const DEAD: &str = "Local controller dead, switching off.";
    const LOBBY: &str = "ECLIPTICA - now in lobby";

    fn kill_at(gs: &mut GameState, t: &str, boss: &str, strike: u64) {
        feed_at(gs, t, &format!("Boss {boss} dead, personal damage dealt: "));
        feed_at(gs, t, &format!("STRIKE DMG: {strike}"));
        feed_at(gs, t, "NON-STRIKE DMG: 0");
    }

    #[test]
    fn wipe_with_kill_triple_marks_run_lost() {
        let mut gs = GameState::default();
        feed_at(&mut gs, "04:12:36", HALL);
        feed_at(
            &mut gs,
            "05:40:45",
            "ECLIPTICA - now fighting boss: MephielPhase2(Clone) on phase: 0.8",
        );
        for t in [
            "05:46:03", "05:46:04", "05:46:05", "05:46:06", "05:46:07", "05:46:08",
        ] {
            feed_at(&mut gs, t, DEAD);
        }
        kill_at(&mut gs, "05:46:08", "MephielPhase2", 5113);
        feed_at(&mut gs, "05:46:08", LOBBY);
        kill_at(&mut gs, "05:46:08", "MephielPhase2", 0);
        let run = &gs.runs[0];
        assert!(run.lost);
        assert_eq!(run.end_ts, run.fights[0].end_ts);
        assert_eq!(run.fights[0].kill, Some((5113, 0)));
        assert!(run.fights[0].lost);
        assert_eq!(
            run.fights
                .last()
                .filter(|f| f.lost)
                .map(|f| f.name.as_str()),
            Some("MephielPhase2")
        );
        assert_eq!(gs.mode, Mode::Lobby);
        assert_eq!(gs.last_kill.as_ref().map(|k| k.strike), Some(5113));
        feed_at(&mut gs, "05:50:00", HALL);
        assert_eq!(gs.runs.len(), 2);
        assert!(gs.last_kill.is_none());
    }

    #[test]
    fn kill_long_after_death_is_a_win() {
        let mut gs = GameState::default();
        feed_at(&mut gs, "04:00:00", HALL);
        feed_at(
            &mut gs,
            "04:05:00",
            "ECLIPTICA - now fighting boss: Melon(Clone) on phase: 0.9",
        );
        feed_at(&mut gs, "04:06:00", DEAD);
        kill_at(&mut gs, "04:21:15", "Melon", 8738);
        feed_at(&mut gs, "04:21:15", LOBBY);
        assert!(!gs.runs[0].lost);
        assert!(gs.runs[0].fights.last().filter(|f| f.lost).is_none());
        assert_eq!(gs.runs[0].fights[0].kill, Some((8738, 0)));
    }

    #[test]
    fn death_after_boss_kill_loses_run_not_fight() {
        let mut gs = GameState::default();
        feed_at(&mut gs, "04:00:00", HALL);
        feed_at(
            &mut gs,
            "04:05:00",
            "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
        );
        kill_at(&mut gs, "04:08:00", "Nan", 1000);
        feed_at(&mut gs, "04:08:05", "ECLIPTICA - now in intermission");
        feed_at(&mut gs, "04:12:00", DEAD);
        feed_at(&mut gs, "04:12:01", LOBBY);
        assert!(gs.runs[0].lost);
        assert!(gs.runs[0].fights.last().filter(|f| f.lost).is_none());
        assert!(!gs.runs[0].fights[0].lost);
    }

    #[test]
    fn end_run_clears_live_state() {
        let mut gs = GameState::default();
        spawn_level(&mut gs, "04:00:01", HALL);
        feed_at(&mut gs, "04:00:20", SAVE);
        feed_at(
            &mut gs,
            "04:05:00",
            "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
        );
        feed_at(&mut gs, "04:05:01", "ownership of Nan transferred to Alice");
        feed_at(&mut gs, "04:05:02", "Dealing 100 STRIKE damage");
        feed_at(
            &mut gs,
            "04:05:03",
            "damage has been taken: 40, from source: (Nan) slam",
        );
        feed_at(&mut gs, "04:05:04", DEAD);
        kill_at(&mut gs, "04:08:00", "Nan", 1000);
        assert!(gs.last_kill.is_some());
        feed_at(&mut gs, "04:08:30", LOBBY);
        assert_eq!(gs.mode, Mode::Lobby);
        assert!(gs.boss.is_none() && gs.target.is_none());
        assert_eq!(gs.fight_dmg, 0);
        assert_eq!(gs.fight_taken, 0);
        assert_eq!(gs.fight_hits, 0);
        assert_eq!(gs.fight_max_hit, 0);
        assert!(gs.fight_attacks.is_empty());
        assert!(gs.taken.is_empty() && gs.history.is_empty() && gs.deaths_log.is_empty());
        assert!(gs.stage.is_empty() && gs.class.is_empty());
        assert_eq!(gs.progress, 0.0);
        assert!(gs.stage_no.is_none());
        assert!(gs.level_tokens.is_empty());
        assert_eq!(gs.tokens_got, 0);
        assert!(!gs.is_dead(gs.last_ts));
        assert_eq!(gs.rolling_dps(gs.last_ts), 0);
        let run = &gs.runs[0];
        assert_eq!(run.hits.len(), 1);
        assert_eq!(run.targets.len(), 1);
        assert_eq!(run.death_log.len(), 1);
        assert_eq!(run.stage_no, Some(1));
        feed_at(&mut gs, "04:09:00", "ownership of Nan transferred to Bob");
        assert!(gs.target.is_none());
        assert!(gs.history.is_empty());
    }

    #[test]
    fn other_world_and_quit_end_the_run() {
        let mut gs = GameState::default();
        feed_at(
            &mut gs,
            "13:24:21",
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        );
        assert!(gs.in_ecliptica());
        feed_at(&mut gs, "13:25:04", HALL);
        feed_at(&mut gs, "13:26:04", "[Behaviour] OnLeftRoom");
        assert!(!gs.in_ecliptica());
        assert!(gs.runs[0].end_ts.is_some());
        assert!(!gs.runs[0].lost);
        feed_at(&mut gs, "13:26:05", "[Behaviour] Entering Room: Sky Dream");
        assert!(!gs.in_ecliptica());
        assert_eq!(gs.mode, Mode::Idle);
        feed_at(
            &mut gs,
            "13:30:00",
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        );
        feed_at(&mut gs, "13:31:00", HALL);
        feed_at(&mut gs, "13:31:10", "[Behaviour] Entering Room: Sky Dream");
        assert_eq!(gs.runs.len(), 2);
        assert!(gs.runs[1].end_ts.is_some());
        assert_eq!(gs.mode, Mode::Idle);
        feed_at(
            &mut gs,
            "13:40:00",
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        );
        feed_at(&mut gs, "13:41:00", HALL);
        feed_at(
            &mut gs,
            "13:42:00",
            "VRCApplication: HandleApplicationQuit at 6480.239",
        );
        assert_eq!(gs.runs.len(), 3);
        assert!(gs.runs[2].end_ts.is_some());
        assert!(gs.world.is_none());
        assert_eq!(gs.mode, Mode::Idle);
    }

    #[test]
    fn empty_class_keeps_previous() {
        let mut gs = GameState::default();
        feed_at(&mut gs, "09:10:01", HALL);
        feed_at(
            &mut gs,
            "09:20:00",
            "ECLIPTICA - now in stage: Stage_ProtoColony on phase: 0.1220348 as class: ",
        );
        assert_eq!(gs.class, "Spellhammer");
        assert_eq!(gs.runs[0].class, "Spellhammer");
        assert_eq!(gs.stage, "ProtoColony");
    }

    #[test]
    fn refight_after_unfinished_fight_is_kept() {
        let mut gs = GameState::default();
        feed_at(&mut gs, "09:10:01", HALL);
        feed_at(
            &mut gs,
            "09:12:00",
            "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
        );
        feed_at(&mut gs, "09:12:30", "ECLIPTICA - now in intermission");
        feed_at(
            &mut gs,
            "09:12:40",
            "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
        );
        assert_eq!(gs.boss.as_deref(), Some("Nan"));
        assert_eq!(gs.runs[0].fights.len(), 2);
        kill_at(&mut gs, "09:15:00", "Nan", 500);
        feed_at(
            &mut gs,
            "09:15:03",
            "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
        );
        assert!(gs.boss.is_none());
        assert_eq!(gs.runs[0].fights.len(), 2);
    }

    #[test]
    fn boss_dead_without_totals_still_records_kill() {
        let mut gs = GameState::default();
        feed_at(&mut gs, "09:10:01", HALL);
        feed_at(
            &mut gs,
            "09:12:00",
            "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
        );
        feed_at(
            &mut gs,
            "09:13:00",
            "Boss Nan dead, personal damage dealt: ",
        );
        feed_at(&mut gs, "09:13:00", "STRIKE DMG: 700");
        feed_at(
            &mut gs,
            "09:13:30",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0",
        );
        kill_at(&mut gs, "09:14:00", "Kakarot", 900);
        assert_eq!(gs.runs[0].fights[0].kill, Some((700, 0)));
        assert_eq!(gs.runs[0].fights[1].kill, Some((900, 0)));
    }

    #[test]
    fn stage_stats_before_the_boss() {
        let mut gs = GameState::default();
        feed_at(&mut gs, "09:10:01", HALL);
        assert!(gs.pre_boss());
        feed_at(&mut gs, "09:10:05", "Dealing 40 STRIKE damage");
        feed_at(
            &mut gs,
            "09:10:06",
            "damage has been taken: 7, from source: machinegunShooter1",
        );
        feed_at(
            &mut gs,
            "09:10:07",
            "damage has been taken: 9, from source: machinegunShooter1",
        );
        feed_at(&mut gs, "09:10:08", DEAD);
        feed_at(
            &mut gs,
            "09:10:30",
            "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0.05 as class: Spellhammer",
        );
        feed_at(&mut gs, "09:10:31", "Dealing 60 STRIKE damage");
        let s = &gs.stage_stats;
        assert_eq!(
            (s.dmg, s.taken, s.hits, s.max_hit, s.deaths),
            (100, 16, 2, 9, 1)
        );
        assert_eq!(s.attacks.len(), 1);
        assert_eq!(gs.fight_dmg, 0);
        let t = feed_at(&mut gs, "09:10:41", "Dealing 0 STRIKE damage");
        assert_eq!(gs.stage_dps(t), 100 / 40);
        feed_at(
            &mut gs,
            "09:11:01",
            "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0.05",
        );
        assert!(!gs.pre_boss());
        assert_eq!(gs.stage_stats.start_ts, 0);
        let run = gs.live_run().unwrap();
        assert_eq!(run.stages.len(), 1);
        assert_eq!(run.stages[0].end_ts, Some(t + 20));
        assert_eq!(run.stages[0].dmg, 100);
        feed_at(&mut gs, "09:11:05", "Dealing 500 STRIKE damage");
        feed_at(
            &mut gs,
            "09:11:06",
            "damage has been taken: 30, from source: (Nan) slam",
        );
        assert_eq!(gs.fight_dmg, 500);
        kill_at(&mut gs, "09:12:00", "Nan", 500);
        feed_at(&mut gs, "09:12:05", "ECLIPTICA - now in intermission");
        let run = gs.live_run().unwrap();
        assert_eq!(run.dmg(), 600);
        assert_eq!(run.taken(), 46);
        assert_eq!(run.hit_count(), 3);
        assert_eq!(run.max_hit(), 30);
        assert_eq!(run.kills(), 1);
        assert_eq!(run.attacks().len(), 2);
        assert_eq!(run.active_secs(0), 60 + 59);
        feed_at(
            &mut gs,
            "09:13:00",
            "ECLIPTICA - now in stage: Stage_GMFuncFlat on phase: 0.1 as class: Spellhammer",
        );
        assert_eq!(gs.stage, "GMFuncFlat");
        assert_eq!(gs.stage_stats.dmg, 0);
        feed_at(&mut gs, "09:13:10", "Dealing 5 STRIKE damage");
        feed_at(&mut gs, "09:14:00", LOBBY);
        assert_eq!(gs.runs[0].stages.len(), 2);
        assert_eq!(gs.runs[0].stages[1].dmg, 5);
        assert_eq!(gs.stage_stats.start_ts, 0);
    }

    #[test]
    fn last_kill_sums_the_phases() {
        let mut gs = GameState::default();
        feed_at(&mut gs, "09:10:01", HALL);
        assert_eq!(gs.last_kill_total(), None);
        feed_at(
            &mut gs,
            "09:12:00",
            "ECLIPTICA - now fighting boss: Mephiel(Clone) on phase: 0.8",
        );
        kill_at(&mut gs, "09:15:00", "Mephiel", 6388);
        assert_eq!(gs.last_kill_total(), Some(("Mephiel".into(), 6388, 0)));
        feed_at(
            &mut gs,
            "09:15:02",
            "ECLIPTICA - now fighting boss: MephielPhase2(Clone) on phase: 0.8",
        );
        feed_at(
            &mut gs,
            "09:18:00",
            "Boss MephielPhase2 dead, personal damage dealt: ",
        );
        feed_at(&mut gs, "09:18:00", "STRIKE DMG: 5113");
        feed_at(&mut gs, "09:18:00", "NON-STRIKE DMG: 7");
        assert_eq!(gs.last_kill_total(), Some(("Mephiel".into(), 11501, 7)));
        feed_at(&mut gs, "09:18:05", "ECLIPTICA - now in intermission");
        assert_eq!(gs.last_kill_total(), Some(("Mephiel".into(), 11501, 7)));
        feed_at(
            &mut gs,
            "09:20:00",
            "ECLIPTICA - now in stage: Stage_GMFuncFlat on phase: 0.9 as class: Spellhammer",
        );
        feed_at(
            &mut gs,
            "09:22:00",
            "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0.9",
        );
        kill_at(&mut gs, "09:23:00", "Nan", 100);
        assert_eq!(gs.last_kill_total(), Some(("Nan".into(), 100, 0)));
    }

    #[test]
    fn gap_after_a_kill_is_not_pre_boss() {
        let mut gs = GameState::default();
        feed_at(&mut gs, "09:10:01", HALL);
        assert!(gs.pre_boss());
        feed_at(
            &mut gs,
            "09:11:01",
            "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0.05",
        );
        kill_at(&mut gs, "09:12:00", "Nan", 100);
        assert!(gs.boss.is_none());
        assert_eq!(gs.mode, Mode::Stage);
        assert!(!gs.pre_boss());
        feed_at(&mut gs, "09:12:04", "Dealing 9 STRIKE damage");
        assert_eq!(gs.stage_stats.dmg, 0);
        feed_at(&mut gs, "09:12:10", "ECLIPTICA - now in intermission");
        assert_eq!(gs.live_run().unwrap().stages.len(), 1);
    }

    #[test]
    fn intermission_closes_the_fight() {
        let mut gs = GameState::default();
        feed_at(&mut gs, "09:10:01", HALL);
        feed_at(
            &mut gs,
            "09:12:00",
            "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
        );
        let t = feed_at(&mut gs, "09:12:30", "ECLIPTICA - now in intermission");
        assert_eq!(gs.runs[0].fights[0].end_ts, Some(t));
    }
}
