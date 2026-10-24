use super::{GameState, Mode, TOKEN_ALERT_SECS};
use crate::game::event::Event;
use crate::game::names::player_summon;
use crate::game::run::{
    base_name, continues, final_phase, phase_num, tally, BossFight, KillSummary, RunEnd,
    StageStats, TakenEntry, TargetEntry,
};
use crate::game::source::describe_source;

// const DEATH_HOLD: u64 = 2;
const LOG_CAP: usize = 500;
pub const ROSTER_CAP: usize = 128;

impl GameState {
    pub fn apply(&mut self, ts: u64, ev: Event) {
        if matches!(ev, Event::WorldLine(_)) {
            return;
        }
        self.changed = true;
        self.seq += 1;
        match ev {
            Event::BossFight { name, .. } => self.boss_fight(ts, name),
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
                self.end_run(ts, RunEnd::Lobby);
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
            Event::SessionSave(_) => {
                if self.mode == Mode::Stage
                    && !self.level_tokens.is_empty()
                    && !self.stage_boss_seen
                {
                    self.tokens_got += 1;
                    self.last_save = Some(ts);
                }
            }
            Event::RoomJoin(location) => {
                if self.location.as_ref() != Some(&location) {
                    self.clear_roster();
                }
                self.location = Some(location);
            }
            Event::SelfName { name, usr } => {
                self.me = Some(name);
                self.me_usr = Some(usr);
            }
            Event::PlayerJoined { name, usr } => self.player_joined(name, usr),
            Event::PlayerLeft { usr, .. } => {
                if let Some(i) = self.roster.iter().position(|(u, _)| *u == usr) {
                    self.roster.remove(i);
                    self.roster_seq += 1;
                }
            }
            Event::EnemySpawn { slot, kind } => self.enemy_spawn(slot, kind),
            Event::EnemyRetire(slot) => self.enemy_retire(ts, slot),
            Event::EnemyName(name) => self.enemy_name(name),
            // Death logging is gone: the game stopped writing its death line reliably (a downed
            // player can go without it entirely), and nothing else in the log marks a death.
            // Event::PlayerDead => self.player_dead(ts),
            Event::PlayerDead => {}
            Event::WorldLine(_) => {}
        }
    }

    fn boss_fight(&mut self, ts: u64, name: String) {
        if matches!(self.mode, Mode::Lobby | Mode::Intermission) {
            return;
        }
        self.bosses.insert(name.clone());
        let last = self
            .runs
            .last()
            .filter(|r| r.end_ts.is_none())
            .and_then(|r| r.fights.last());
        let echo = self.stage_boss_seen
            && last.is_some_and(|f| {
                base_name(&f.name) == base_name(&name) && phase_num(&name) <= phase_num(&f.name)
            });
        let transition = last.is_some_and(|f| continues(f, &name));
        if self.boss.as_deref() == Some(&name) || echo {
            return;
        }
        let stage_deaths = self.stage_stats.deaths;
        self.close_stage(self.last_clear.unwrap_or(ts));
        self.boss = Some(name.clone());
        if !self.stage_boss_seen {
            self.stage_boss_seen = true;
            if self.last_save.is_some() {
                self.tokens_got = self.tokens_got.saturating_sub(1);
            }
        }
        if !transition {
            self.cursed = stage_deaths >= 2;
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
            max_hit: 0,
            attacks: Vec::new(),
            deaths: 0,
            kill: None,
            lost: false,
            series: Vec::new(),
            taken_series: Vec::new(),
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
        self.hits.push_back((ts, n));
        if self.boss.is_some() {
            self.fight_dmg += n;
            if let Some(f) = self.open_fight() {
                f.dmg += n;
                f.add_dmg(ts, n);
            }
        } else if self.pre_boss() {
            self.stage_stats.dmg += n;
            self.stage_stats.add_dmg(ts, n);
        }
    }

    fn damage_taken(&mut self, ts: u64, amount: u64, source: String) {
        if amount == 0 {
            return;
        }
        self.taken_hits.push_back((ts, amount));
        if self.pre_boss() {
            let s = &mut self.stage_stats;
            s.taken += amount;
            s.add_taken(ts, amount);
            s.hits += 1;
            s.max_hit = s.max_hit.max(amount);
            let (who, attack) = describe_source(&source);
            tally(&mut s.attacks, &who, &attack, amount);
        } else if self.boss.is_some() {
            self.fight_taken += amount;
            self.fight_hits += 1;
            self.fight_max_hit = self.fight_max_hit.max(amount);
            let (who, attack) = describe_source(&source);
            tally(&mut self.fight_attacks, &who, &attack, amount);
            if let Some(f) = self.open_fight() {
                f.taken += amount;
                f.hits += 1;
                f.max_hit = f.max_hit.max(amount);
                tally(&mut f.attacks, &who, &attack, amount);
                f.add_taken(ts, amount);
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
            self.stage_boss_seen = false;
            self.pool_known = std::mem::take(&mut self.pool_reset_seen);
            self.stage_stats = StageStats {
                name: name.clone(),
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

    // Death logging is gone, see Event::PlayerDead in apply.
    // fn player_dead(&mut self, ts: u64) {
    //     let new = self
    //         .last_death
    //         .is_none_or(|_| self.alive_since.is_some_and(|a| a < ts));
    //     self.last_death = Some(ts);
    //     self.alive_since = None;
    //     if new {
    //         self.death_final = self.cursed;
    //         self.cursed = true;
    //         self.deaths_total += 1;
    //         if let Some(r) = self.runs.last_mut().filter(|r| r.end_ts.is_none()) {
    //             r.deaths += 1;
    //         }
    //         if self.pre_boss() {
    //             self.stage_stats.deaths += 1;
    //         }
    //         if let Some(f) = self.open_fight() {
    //             f.deaths += 1;
    //         }
    //         self.deaths_log.push_back((ts, self.seq));
    //         if self.deaths_log.len() > LOG_CAP {
    //             self.deaths_log.pop_front();
    //         }
    //     }
    //     self.dead_until = ts + DEATH_HOLD;
    // }

    fn record_kill(&mut self, k: KillSummary) {
        let Some(f) = self
            .runs
            .last_mut()
            .filter(|r| r.end_ts.is_none())
            .and_then(|r| {
                r.fights
                    .iter_mut()
                    .rev()
                    .find(|f| f.name == k.boss && f.kill.is_none())
            })
        else {
            return;
        };
        f.kill = Some((k.strike, k.non_strike));
        f.end_ts.get_or_insert(k.ts);
        self.last_kill = Some(k);
    }

    fn close_stage(&mut self, ts: u64) {
        let end = self.clear_ts.take().unwrap_or(ts);
        self.last_clear = None;
        self.alive.clear();
        if self.stage_stats.start_ts == 0 {
            return;
        }
        let mut done = std::mem::take(&mut self.stage_stats);
        done.end_ts = Some(end);
        if let Some(r) = self.runs.last_mut().filter(|r| r.end_ts.is_none()) {
            r.stages.push(done);
        }
    }

    fn end_run(&mut self, ts: u64, how: RunEnd) {
        let in_fight = self.mode == Mode::Stage && self.stage_boss_seen;
        self.close_stage(ts);
        if let Some(r) = self.runs.last_mut().filter(|r| r.end_ts.is_none()) {
            let mut end = how;
            let mut end_ts = ts;
            if let Some(f) = r.fights.last_mut() {
                let running = f.end_ts.is_none_or(|e| e == ts);
                let kill_ts = *f.end_ts.get_or_insert(ts);
                if how == RunEnd::Lobby && in_fight && running {
                    f.lost = true;
                    end = RunEnd::Lost;
                } else if f.kill.is_some() && final_phase(&f.name) {
                    end = RunEnd::Won;
                    end_ts = kill_ts;
                }
            }
            r.end = Some(end);
            r.end_ts = Some(end_ts);
            r.hits = std::mem::take(&mut self.taken);
            r.targets = std::mem::take(&mut self.history);
            r.death_log = std::mem::take(&mut self.deaths_log);
        }
        self.boss = None;
        self.target = None;
        self.pending_kill = None;
        self.dead_until = 0;
        self.cursed = false;
        self.death_final = false;
        self.last_death = None;
        self.alive_since = None;
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

    fn player_joined(&mut self, name: String, usr: String) {
        let full = self.roster.len() >= ROSTER_CAP;
        match self.roster.iter_mut().find(|(u, _)| *u == usr) {
            Some((_, n)) if *n == name => return,
            Some((_, n)) => *n = name,
            None if full => return,
            None => self.roster.push((usr, name)),
        }
        self.roster_seq += 1;
    }

    fn clear_roster(&mut self) {
        if !self.roster.is_empty() {
            self.roster.clear();
            self.roster_seq += 1;
        }
    }

    fn leave_world(&mut self, ts: u64) {
        self.clear_roster();
        self.end_run(ts, RunEnd::Left);
        self.pool_reset_seen = false;
        self.mode = Mode::Idle;
        self.location = None;
        self.world = None;
    }

    pub fn log_rotated(&mut self) {
        self.leave_world(self.last_ts);
        self.changed = true;
    }

    fn enemy_spawn(&mut self, slot: u32, kind: u32) {
        self.last_spawn = None;
        if !self.pre_boss() || !self.pool_known || self.summon_kinds.contains(&kind) {
            return;
        }
        self.alive.insert(slot);
        self.last_spawn = Some((slot, kind, self.clear_ts.take()));
    }

    fn enemy_name(&mut self, name: String) {
        let Some((slot, kind, before)) = self.last_spawn.take() else {
            return;
        };
        if player_summon(&name) {
            self.summon_kinds.insert(kind);
            if self.alive.remove(&slot) && self.alive.is_empty() {
                self.clear_ts = before;
            }
        }
    }

    fn enemy_retire(&mut self, ts: u64, slot: u32) {
        self.last_spawn = None;
        if !self.pre_boss() {
            if matches!(self.mode, Mode::Idle | Mode::Lobby | Mode::Intermission) {
                self.pool_reset_seen = true;
            }
            return;
        }
        if self.alive.remove(&slot) && self.alive.is_empty() {
            self.clear_ts = Some(ts);
            self.last_clear = Some(ts);
            self.token_clear = Some(ts);
        }
    }

    pub fn poll_token_alert(&mut self, now: u64) {
        let Some(at) = self.token_clear else {
            return;
        };
        if now < at + TOKEN_ALERT_SECS || self.last_spawn.is_some() {
            return;
        }
        self.token_clear = None;
        if self.stage_clear() == Some(at) && self.tokens_missing().is_some() {
            self.token_alerts += 1;
            self.changed = true;
        }
    }

    fn reset_stage_tokens(&mut self) {
        self.tokens_got = 0;
        self.last_save = None;
        self.stage_boss_seen = false;
    }
}
