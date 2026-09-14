use super::{GameState, Mode};
use crate::game::event::Event;
use crate::game::run::{
    base_name, continues, phase_num, tally, BossFight, KillSummary, StageStats, TakenEntry,
    TargetEntry, KILL_DEDUPE_SECS,
};
use crate::game::source::describe_source;

const DEATH_HOLD: u64 = 3;
const WIPE_SECS: u64 = 3;
const LOG_CAP: usize = 500;
const BOSS_SAVE_LEAD: u64 = 8;

impl GameState {
    pub fn apply(&mut self, ts: u64, ev: Event) {
        self.changed = true;
        self.seq += 1;
        match ev {
            Event::BossFight { name } => self.boss_fight(ts, name),
            Event::BossDead { name } => self.boss_dead(ts, name),
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
            Event::Dealt { n, .. } => self.dealt(ts, n),
            Event::DamageTaken { amount, source } => self.damage_taken(ts, amount, source),
            Event::Ownership { object, player } => self.ownership(ts, object, player),
            Event::Stage {
                name,
                progress,
                class,
            } => self.stage(ts, name, progress, class),
            Event::Intermission => self.intermission(ts),
            Event::Lobby => {
                self.end_run(ts);
                self.mode = Mode::Lobby;
            }
            Event::RoomLeft => self.leave_world(ts),
            Event::RoomEnter(name) => {
                if !name.starts_with("Ecliptica") {
                    self.leave_world(ts);
                }
                self.world = Some(name);
            }
            Event::TokenSpawn { rune, chance } => self.pending_tokens.push((rune, chance)),
            Event::SessionSave => {
                if self.mode == Mode::Stage
                    && !self.level_tokens.is_empty()
                    && !self.stage_boss_seen
                {
                    self.tokens_got += 1;
                    self.last_save = Some(ts);
                }
            }
            Event::RoomJoin(location) => self.location = Some(location),
            Event::EnemyActivity => self.touch_wave(ts),
            Event::PlayerDead => self.player_dead(ts),
        }
    }

    fn boss_fight(&mut self, ts: u64, name: String) {
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
        if self.boss.as_deref() == Some(&name) || echo {
            return;
        }
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

    fn boss_dead(&mut self, ts: u64, name: String) {
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

    fn dealt(&mut self, ts: u64, n: u64) {
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

    fn damage_taken(&mut self, ts: u64, amount: u64, source: String) {
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
            seq: self.seq,
            amount,
            source,
        });
        if self.taken.len() > LOG_CAP {
            self.taken.pop_front();
        }
    }

    fn ownership(&mut self, ts: u64, object: String, player: String) {
        self.touch_wave(ts);
        if !self.bosses.contains(&object) {
            return;
        }
        self.target = Some(player.clone());
        self.target_since = ts;
        self.targets_total += 1;
        self.history.push_back(TargetEntry {
            ts,
            seq: self.seq,
            player,
            boss: object,
        });
        if self.history.len() > LOG_CAP {
            self.history.pop_front();
        }
    }

    fn stage(&mut self, ts: u64, name: String, progress: f32, class: String) {
        self.mode = Mode::Stage;
        self.wave_last_activity = ts;
        if !self.pending_tokens.is_empty() {
            self.level_tokens = std::mem::take(&mut self.pending_tokens);
            self.reset_stage_tokens();
        } else if self.stage != name {
            self.level_tokens.clear();
            self.reset_stage_tokens();
        }
        let fresh = self.stage_stats.start_ts == 0 || self.stage != name || self.boss.is_some();
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

    fn intermission(&mut self, ts: u64) {
        self.close_stage(ts);
        self.mode = Mode::Intermission;
        self.boss = None;
        self.target = None;
        if let Some(f) = self.open_fight() {
            f.end_ts = Some(ts);
        }
    }

    fn player_dead(&mut self, ts: u64) {
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
            self.deaths_log.push_back((ts, self.seq));
            if self.deaths_log.len() > LOG_CAP {
                self.deaths_log.pop_front();
            }
        }
        self.dead_until = ts + DEATH_HOLD;
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

    fn reset_stage_tokens(&mut self) {
        self.tokens_got = 0;
        self.last_save = None;
        self.stage_boss_seen = false;
    }
}
