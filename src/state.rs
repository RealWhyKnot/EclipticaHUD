use crate::parse::{parse_msg, split_line, Event};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone)]
pub struct TargetEntry {
    pub ts: u64,
    pub player: String,
    pub boss: String,
}

#[derive(Debug, Clone)]
pub struct TakenEntry {
    pub ts: u64,
    pub amount: u64,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct KillSummary {
    pub ts: u64,
    pub boss: String,
    pub strike: u64,
    pub non_strike: u64,
}

#[derive(Debug, Clone)]
pub struct BossFight {
    pub name: String,
    pub start_ts: u64,
    pub end_ts: Option<u64>,
    pub dmg: u64,
    pub kill: Option<(u64, u64)>,
}

#[derive(Default, Debug, Clone)]
pub struct Run {
    pub start_ts: u64,
    pub end_ts: Option<u64>,
    pub stage: String,
    pub class: String,
    pub fights: Vec<BossFight>,
    pub deaths: u32,
}

impl Run {
    pub fn groups(&self) -> Vec<std::ops::Range<usize>> {
        let mut out: Vec<std::ops::Range<usize>> = Vec::new();
        for (i, f) in self.fights.iter().enumerate() {
            let chained = out.last().is_some_and(|g| {
                let prev = &self.fights[g.end - 1];
                base_name(&prev.name) == base_name(&f.name)
                    && phase_num(&f.name) > phase_num(&prev.name)
            });
            match out.last_mut() {
                Some(g) if chained => g.end = i + 1,
                _ => out.push(i..i + 1),
            }
        }
        out
    }
}

pub fn base_name(name: &str) -> &str {
    let Some(pos) = name.rfind("Phase") else {
        return name;
    };
    let digits = &name[pos + 5..];
    if pos > 0 && !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        &name[..pos]
    } else {
        name
    }
}

pub fn phase_num(name: &str) -> u32 {
    if base_name(name) == name {
        return 1;
    }
    let pos = name.rfind("Phase").unwrap();
    name[pos + 5..].parse().unwrap_or(1)
}

#[derive(Default, Debug, Clone, PartialEq)]
pub enum Mode {
    #[default]
    Idle,
    Lobby,
    Intermission,
    Stage,
}

const DPS_WINDOW: u64 = 10;
const KILL_DEDUPE_SECS: u64 = 30;
const DEATH_HOLD: u64 = 3;

#[derive(Default)]
pub struct GameState {
    bosses: HashSet<String>,
    pub mode: Mode,
    pub stage: String,
    pub progress: f32,
    pub class: String,
    pub boss: Option<String>,
    pub fight_start: u64,
    pub fight_dmg: u64,
    pub target: Option<String>,
    pub target_since: u64,
    pub history: VecDeque<TargetEntry>,
    pub targets_total: u64,
    pub taken: VecDeque<TakenEntry>,
    pub last_kill: Option<KillSummary>,
    pub runs: Vec<Run>,
    hits: VecDeque<(u64, u64)>,
    pending_kill: Option<KillSummary>,
    dead_seen: Option<(String, u64)>,
    dead_until: u64,
    last_ts: u64,
    pub changed: bool,
}

impl GameState {
    fn open_run(&mut self, ts: u64) -> &mut Run {
        if self.runs.last().is_none_or(|r| r.end_ts.is_some()) {
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

    fn close_run(&mut self, ts: u64) {
        if let Some(f) = self.open_fight() {
            f.end_ts = Some(ts);
        }
        if let Some(r) = self.runs.last_mut() {
            if r.end_ts.is_none() {
                r.end_ts = Some(ts);
            }
        }
    }

    pub fn log_rotated(&mut self) {
        self.close_run(self.last_ts);
        self.mode = Mode::Idle;
        self.boss = None;
        self.target = None;
        self.stage.clear();
        self.class.clear();
        self.progress = 0.0;
        self.pending_kill = None;
        self.dead_seen = None;
        self.dead_until = 0;
        self.taken.clear();
        self.history.clear();
        self.changed = true;
    }

    pub fn is_dead(&self, now: u64) -> bool {
        now < self.dead_until
    }
    pub fn apply(&mut self, ts: u64, ev: Event) {
        self.changed = true;
        match ev {
            Event::BossFight { name } => {
                self.bosses.insert(name.clone());
                let just_ended = self.runs.last().is_some_and(|r| {
                    r.fights.iter().rev().any(|f| {
                        f.name == name
                            && f.end_ts
                                .is_some_and(|e| ts.saturating_sub(e) <= KILL_DEDUPE_SECS)
                    })
                });
                let open_run = self.runs.last().filter(|r| r.end_ts.is_none());
                let transition = open_run.and_then(|r| r.fights.last()).is_some_and(|prev| {
                    base_name(&prev.name) == base_name(&name)
                        && phase_num(&name) > phase_num(&prev.name)
                        && prev
                            .end_ts
                            .is_none_or(|e| ts.saturating_sub(e) <= KILL_DEDUPE_SECS)
                });
                if self.boss.as_deref() != Some(&name) && !just_ended {
                    self.boss = Some(name.clone());
                    if !transition {
                        self.fight_start = ts;
                        self.fight_dmg = 0;
                    }
                    if let Some(f) = self.open_fight() {
                        f.end_ts = Some(ts);
                    }
                    self.open_run(ts).fights.push(BossFight {
                        name,
                        start_ts: ts,
                        end_ts: None,
                        dmg: 0,
                        kill: None,
                    });
                }
            }
            Event::BossDead { name } => {
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
                    let chained = self.dead_seen.as_ref().is_some_and(|(boss, ts)| {
                        *boss == k.boss && k.ts.saturating_sub(*ts) <= KILL_DEDUPE_SECS
                    });
                    self.dead_seen = Some((k.boss.clone(), k.ts));
                    let dupe = chained
                        && self.last_kill.as_ref().is_some_and(|p| {
                            p.boss == k.boss && p.strike + p.non_strike >= k.strike + k.non_strike
                        });
                    if !dupe {
                        if let Some(f) = self.runs.last_mut().and_then(|r| {
                            r.fights
                                .iter_mut()
                                .rev()
                                .find(|f| f.name == k.boss && f.kill.is_none())
                        }) {
                            f.kill = Some((k.strike, k.non_strike));
                            f.end_ts.get_or_insert(k.ts);
                        }
                        self.last_kill = Some(k);
                    }
                }
            }
            Event::DealtStrike(n) => {
                self.hits.push_back((ts, n));
                if self.boss.is_some() {
                    self.fight_dmg += n;
                    if let Some(f) = self.open_fight() {
                        f.dmg += n;
                    }
                }
            }
            Event::DamageTaken { amount, source } => {
                self.taken.push_back(TakenEntry { ts, amount, source });
                if self.taken.len() > 20 {
                    self.taken.pop_front();
                }
            }
            Event::Ownership { object, player } => {
                if self.bosses.contains(&object) {
                    self.target = Some(player.clone());
                    self.target_since = ts;
                    self.targets_total += 1;
                    self.history.push_back(TargetEntry {
                        ts,
                        player,
                        boss: object,
                    });
                    if self.history.len() > 100 {
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
                self.stage = name.clone();
                self.progress = progress;
                self.class = class.clone();
                let run = self.open_run(ts);
                run.stage = name;
                run.class = class;
            }
            Event::Intermission => {
                self.mode = Mode::Intermission;
                self.boss = None;
                self.target = None;
            }
            Event::Lobby => {
                self.mode = Mode::Lobby;
                self.boss = None;
                self.target = None;
                self.progress = 0.0;
                self.close_run(ts);
            }
            Event::RoomLeft => {
                self.mode = Mode::Idle;
                self.boss = None;
                self.target = None;
                self.stage.clear();
                self.class.clear();
                self.progress = 0.0;
                self.close_run(ts);
            }
            Event::PlayerDead => {
                if ts >= self.dead_until {
                    if let Some(r) = self.runs.last_mut().filter(|r| r.end_ts.is_none()) {
                        r.deaths += 1;
                    }
                }
                self.dead_until = ts + DEATH_HOLD;
            }
        }
    }

    pub fn feed(&mut self, raw: &str) -> Option<u64> {
        let line = split_line(raw)?;
        self.last_ts = self.last_ts.max(line.ts);
        if let Some(ev) = parse_msg(line.msg) {
            self.apply(line.ts, ev);
        }
        Some(line.ts)
    }

    pub fn rolling_dps(&mut self, now: u64) -> u64 {
        while self.hits.front().is_some_and(|h| h.0 + DPS_WINDOW < now) {
            self.hits.pop_front();
        }
        self.hits.iter().map(|h| h.1).sum::<u64>() / DPS_WINDOW
    }

    pub fn fight_dps(&self, now: u64) -> u64 {
        if self.boss.is_none() {
            return 0;
        }
        self.fight_dmg / now.saturating_sub(self.fight_start).max(1)
    }
}

pub fn fmt_clock(ts: u64) -> String {
    format!("{:02}:{:02}:{:02}", ts / 3600 % 24, ts / 60 % 60, ts % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn phase_helpers() {
        assert_eq!(base_name("Yuki"), "Yuki");
        assert_eq!(base_name("YukiPhase2"), "Yuki");
        assert_eq!(base_name("ManalyteAncientPhase2"), "ManalyteAncient");
        assert_eq!(base_name("M41D"), "M41D");
        assert_eq!(base_name("NX-Obsidian"), "NX-Obsidian");
        assert_eq!(base_name("Phase2"), "Phase2");
        assert_eq!(base_name("PhaseShifter"), "PhaseShifter");
        assert_eq!(phase_num("Yuki"), 1);
        assert_eq!(phase_num("YukiPhase2"), 2);
        assert_eq!(phase_num("AntKingPhase3"), 3);
    }

    #[test]
    fn groups_merge_phases_split_rekills() {
        let fight = |name: &str| BossFight {
            name: name.into(),
            start_ts: 0,
            end_ts: None,
            dmg: 0,
            kill: None,
        };
        let run = Run {
            fights: vec![
                fight("Yuki"),
                fight("YukiPhase2"),
                fight("Kakarot"),
                fight("Kakarot"),
                fight("Nan"),
            ],
            ..Default::default()
        };
        assert_eq!(run.groups(), vec![0..2, 2..3, 3..4, 4..5]);
        assert!(Run::default().groups().is_empty());
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
        assert_eq!(gs.rolling_dps(t0), 15);
        assert_eq!(gs.rolling_dps(t0 + 60), 0);
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
    fn death_clusters_count_once() {
        let mut gs = GameState::default();
        feed_at(
            &mut gs,
            "08:18:01",
            "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer",
        );
        for t in ["08:20:42", "08:20:42", "08:20:43", "08:20:44", "08:20:46"] {
            feed_at(&mut gs, t, "Local controller dead, switching off.");
        }
        let last = feed_at(&mut gs, "08:20:48", "Local controller dead, switching off.");
        assert_eq!(gs.runs[0].deaths, 1);
        assert!(gs.is_dead(last));
        assert!(gs.is_dead(last + 2));
        assert!(!gs.is_dead(last + 3));
        feed_at(&mut gs, "08:25:55", "Local controller dead, switching off.");
        assert_eq!(gs.runs[0].deaths, 2);
    }
}
