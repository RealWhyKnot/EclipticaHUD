use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    BossFight { name: String },
    BossDead { name: String },
    StrikeTotal(u64),
    NonStrikeTotal(u64),
    DealtStrike(u64),
    DamageTaken { amount: u64, source: String },
    Ownership { object: String, player: String },
    Stage { name: String, progress: f32, class: String },
    Intermission,
    Lobby,
}

pub struct Line<'a> {
    pub ts: u64,
    pub msg: &'a str,
}

pub fn split_line(raw: &str) -> Option<Line<'_>> {
    let b = raw.as_bytes();
    if b.len() < 34 || b[4] != b'.' || b[7] != b'.' || b[10] != b' ' {
        return None;
    }
    let ts = parse_ts(&raw[..19])?;
    let rest = &raw[19..];
    let sep = rest.find("-  ")?;
    Some(Line { ts, msg: &rest[sep + 3..] })
}

fn parse_ts(s: &str) -> Option<u64> {
    let num = |r: &str| r.parse::<u64>().ok();
    let y = num(s.get(0..4)?)?;
    let mo = num(s.get(5..7)?)?;
    let d = num(s.get(8..10)?)?;
    let h = num(s.get(11..13)?)?;
    let mi = num(s.get(14..16)?)?;
    let sec = num(s.get(17..19)?)?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let (y, mo) = if mo <= 2 { (y - 1, mo + 12) } else { (y, mo) };
    let era_days = 365 * y + y / 4 - y / 100 + y / 400 + (153 * (mo + 1)) / 5 + d - 1;
    Some(era_days * 86400 + h * 3600 + mi * 60 + sec)
}

pub fn parse_msg(msg: &str) -> Option<Event> {
    if let Some(rest) = msg.strip_prefix("ownership of ") {
        let i = rest.find(" transferred to ")?;
        let player = rest[i + 16..].trim_end();
        if player.is_empty() {
            return None;
        }
        return Some(Event::Ownership { object: rest[..i].to_string(), player: player.to_string() });
    }
    if let Some(rest) = msg.strip_prefix("Dealing ") {
        let n = rest.strip_suffix(" STRIKE damage")?.parse().ok()?;
        return Some(Event::DealtStrike(n));
    }
    if let Some(rest) = msg.strip_prefix("damage has been taken: ") {
        let i = rest.find(", from source:")?;
        let amount = rest[..i].parse().ok()?;
        return Some(Event::DamageTaken { amount, source: rest[i + 14..].trim().to_string() });
    }
    if let Some(rest) = msg.strip_prefix("ECLIPTICA - now ") {
        if let Some(rest) = rest.strip_prefix("fighting boss: ") {
            let i = rest.find("(Clone)")?;
            return Some(Event::BossFight { name: rest[..i].to_string() });
        }
        if let Some(rest) = rest.strip_prefix("in stage: ") {
            let ip = rest.find(" on phase: ")?;
            let ic = rest.find(" as class: ")?;
            let name = rest[..ip].strip_prefix("Stage_").unwrap_or(&rest[..ip]);
            return Some(Event::Stage {
                name: name.to_string(),
                progress: rest[ip + 11..ic].parse().ok()?,
                class: rest[ic + 11..].trim().to_string(),
            });
        }
        if rest.starts_with("in intermission") {
            return Some(Event::Intermission);
        }
        if rest.starts_with("in lobby") {
            return Some(Event::Lobby);
        }
        return None;
    }
    if let Some(rest) = msg.strip_prefix("Boss ") {
        let name = rest.strip_suffix("dead, personal damage dealt: ")?.trim_end();
        return Some(Event::BossDead { name: name.to_string() });
    }
    if let Some(rest) = msg.strip_prefix("STRIKE DMG: ") {
        return Some(Event::StrikeTotal(rest.trim().parse().ok()?));
    }
    if let Some(rest) = msg.strip_prefix("NON-STRIKE DMG: ") {
        return Some(Event::NonStrikeTotal(rest.trim().parse().ok()?));
    }
    None
}

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
    hits: VecDeque<(u64, u64)>,
    pending_kill: Option<KillSummary>,
    pub changed: bool,
}

impl GameState {
    pub fn apply(&mut self, ts: u64, ev: Event) {
        self.changed = true;
        match ev {
            Event::BossFight { name } => {
                self.bosses.insert(name.clone());
                if self.boss.as_deref() != Some(&name) {
                    self.boss = Some(name);
                    self.fight_start = ts;
                    self.fight_dmg = 0;
                }
            }
            Event::BossDead { name } => {
                self.pending_kill = Some(KillSummary { ts, boss: name.clone(), strike: 0, non_strike: 0 });
                if self.boss.as_deref() == Some(&name) {
                    self.boss = None;
                    self.target = None;
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
                    let dupe = self.last_kill.as_ref().is_some_and(|p| {
                        p.boss == k.boss
                            && k.ts.saturating_sub(p.ts) <= KILL_DEDUPE_SECS
                            && p.strike + p.non_strike >= k.strike + k.non_strike
                    });
                    if !dupe {
                        self.last_kill = Some(k);
                    }
                }
            }
            Event::DealtStrike(n) => {
                self.hits.push_back((ts, n));
                if self.boss.is_some() {
                    self.fight_dmg += n;
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
                    self.history.push_back(TargetEntry { ts, player, boss: object });
                    if self.history.len() > 100 {
                        self.history.pop_front();
                    }
                }
            }
            Event::Stage { name, progress, class } => {
                self.mode = Mode::Stage;
                self.stage = name;
                self.progress = progress;
                self.class = class;
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
            }
        }
    }

    pub fn feed(&mut self, raw: &str) -> Option<u64> {
        let line = split_line(raw)?;
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
    fn ownership_unicode() {
        let ev = parse_msg("ownership of ManalyteBig transferred to Be\u{430}rHands").unwrap();
        assert_eq!(
            ev,
            Event::Ownership { object: "ManalyteBig".into(), player: "Be\u{430}rHands".into() }
        );
        let ev = parse_msg("ownership of BigWolf transferred to \u{1d04}\u{29c}\u{1d07}\u{1d05}\u{1d1c}\u{493} \u{6c17}\u{307e}\u{3050}\u{308c}").unwrap();
        match ev {
            Event::Ownership { player, .. } => assert_eq!(player.chars().count(), 11),
            _ => panic!(),
        }
    }

    #[test]
    fn boss_lines() {
        assert_eq!(
            parse_msg("ECLIPTICA - now fighting boss: ObisidusPhase2(Clone) on phase: 0.7223684"),
            Some(Event::BossFight { name: "ObisidusPhase2".into() })
        );
        assert_eq!(
            parse_msg("Boss Kakarot dead, personal damage dealt: "),
            Some(Event::BossDead { name: "Kakarot".into() })
        );
        assert_eq!(parse_msg("STRIKE DMG: 4793"), Some(Event::StrikeTotal(4793)));
        assert_eq!(parse_msg("NON-STRIKE DMG: 0"), Some(Event::NonStrikeTotal(0)));
    }

    #[test]
    fn damage_lines() {
        assert_eq!(parse_msg("Dealing 140 STRIKE damage"), Some(Event::DealtStrike(140)));
        assert_eq!(
            parse_msg("damage has been taken: 12, from source: (Khepri) attack_Claws2"),
            Some(Event::DamageTaken { amount: 12, source: "(Khepri) attack_Claws2".into() })
        );
        assert_eq!(
            parse_msg("damage has been taken: 2, from source: "),
            Some(Event::DamageTaken { amount: 2, source: String::new() })
        );
        assert_eq!(
            parse_msg("damage has been taken: 2, from source:"),
            Some(Event::DamageTaken { amount: 2, source: String::new() })
        );
    }

    #[test]
    fn stage_line() {
        let ev = parse_msg(
            "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer",
        )
        .unwrap();
        assert_eq!(
            ev,
            Event::Stage { name: "Hall of Beginnings".into(), progress: 0.0, class: "Spellhammer".into() }
        );
        assert_eq!(parse_msg("ECLIPTICA - now in intermission"), Some(Event::Intermission));
        assert_eq!(parse_msg("ECLIPTICA - now in lobby"), Some(Event::Lobby));
    }

    #[test]
    fn non_events() {
        assert_eq!(parse_msg("Retiring Enemy POOL ID19"), None);
        assert_eq!(parse_msg("2.5"), None);
        assert_eq!(parse_msg("ECLIPTICA saving SESSION ID 19854"), None);
        assert_eq!(parse_msg("Backup Active, swapping..."), None);
    }

    #[test]
    fn line_split() {
        let l = split_line("2026.09.07 09:03:54 Debug      -  Dealing 90 STRIKE damage").unwrap();
        assert_eq!(l.msg, "Dealing 90 STRIKE damage");
        let l2 = split_line("2026.09.07 09:03:55 Warning    -  ENEMY Kakarot INTRO SLAP.").unwrap();
        assert_eq!(l2.ts, l.ts + 1);
        assert!(split_line("  at UnityEngine.EventSystems.ExecuteEvents.Execute").is_none());
        assert!(split_line("").is_none());
    }

    #[test]
    fn boss_gating() {
        let mut gs = GameState::default();
        gs.feed(&format!("{P}ownership of Neko1 transferred to Alice"));
        assert!(gs.target.is_none());
        gs.feed(&format!("{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1"));
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
            gs.feed(&format!("2026.09.07 {t} Debug      -  Boss Kakarot dead, personal damage dealt: "));
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
    fn dps_windows() {
        let mut gs = GameState::default();
        let t0 = split_line(&format!("{P}Dealing 100 STRIKE damage")).unwrap().ts;
        gs.feed(&format!("{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1"));
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
        gs.feed(&format!("{P}ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0.6"));
        gs.feed(&format!("{P}Dealing 100 STRIKE damage"));
        gs.feed(&format!("{P}ECLIPTICA - now fighting boss: YukiPhase2(Clone) on phase: 0.6"));
        assert_eq!(gs.fight_dmg, 0);
        assert_eq!(gs.boss.as_deref(), Some("YukiPhase2"));
    }
}
