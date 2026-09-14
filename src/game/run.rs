use std::collections::VecDeque;

pub(crate) const KILL_DEDUPE_SECS: u64 = 30;

#[derive(Debug, Clone)]
pub struct TargetEntry {
    pub ts: u64,
    pub seq: u64,
    pub player: String,
    pub boss: String,
}

#[derive(Debug, Clone)]
pub struct TakenEntry {
    pub ts: u64,
    pub seq: u64,
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

#[derive(Debug, Clone, PartialEq)]
pub struct Tally {
    pub who: String,
    pub attack: String,
    pub total: u64,
    pub hits: u32,
}

pub fn tally(list: &mut Vec<Tally>, who: &str, attack: &str, amount: u64) {
    match list.iter_mut().find(|t| t.who == who && t.attack == attack) {
        Some(t) => {
            t.total += amount;
            t.hits += 1;
        }
        None => list.push(Tally {
            who: who.to_string(),
            attack: attack.to_string(),
            total: amount,
            hits: 1,
        }),
    }
}

pub fn merge_tallies<'a>(groups: impl Iterator<Item = &'a [Tally]>) -> Vec<Tally> {
    let mut out: Vec<Tally> = Vec::new();
    for g in groups {
        for t in g {
            match out
                .iter_mut()
                .find(|o| o.who == t.who && o.attack == t.attack)
            {
                Some(o) => {
                    o.total += t.total;
                    o.hits += t.hits;
                }
                None => out.push(t.clone()),
            }
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct BossFight {
    pub name: String,
    pub start_ts: u64,
    pub end_ts: Option<u64>,
    pub dmg: u64,
    pub taken: u64,
    pub hits: u32,
    pub attacks: Vec<Tally>,
    pub deaths: u32,
    pub kill: Option<(u64, u64)>,
    pub lost: bool,
}

#[derive(Debug, Clone, Default)]
pub struct StageStats {
    pub start_ts: u64,
    pub end_ts: Option<u64>,
    pub dmg: u64,
    pub taken: u64,
    pub hits: u32,
    pub max_hit: u64,
    pub attacks: Vec<Tally>,
    pub deaths: u32,
}

#[derive(Default, Debug, Clone)]
pub struct Run {
    pub start_ts: u64,
    pub end_ts: Option<u64>,
    pub stage: String,
    pub class: String,
    pub stage_no: Option<u32>,
    pub fights: Vec<BossFight>,
    pub deaths: u32,
    pub lost: bool,
    pub won: bool,
    pub hits: VecDeque<TakenEntry>,
    pub targets: VecDeque<TargetEntry>,
    pub death_log: VecDeque<(u64, u64)>,
    pub stages: Vec<StageStats>,
}

impl Run {
    pub fn dmg(&self) -> u64 {
        self.fights.iter().map(|f| f.dmg).sum::<u64>()
            + self.stages.iter().map(|s| s.dmg).sum::<u64>()
    }

    pub fn taken(&self) -> u64 {
        self.fights.iter().map(|f| f.taken).sum::<u64>()
            + self.stages.iter().map(|s| s.taken).sum::<u64>()
    }

    pub fn hit_count(&self) -> u32 {
        self.fights.iter().map(|f| f.hits).sum::<u32>()
            + self.stages.iter().map(|s| s.hits).sum::<u32>()
    }

    pub fn max_hit(&self) -> u64 {
        self.fights
            .iter()
            .flat_map(|f| f.attacks.iter().map(|t| t.total / t.hits.max(1) as u64))
            .chain(self.stages.iter().map(|s| s.max_hit))
            .max()
            .unwrap_or(0)
    }

    pub fn kills(&self) -> usize {
        self.fights.iter().filter(|f| f.kill.is_some()).count()
    }

    pub fn attacks(&self) -> Vec<Tally> {
        merge_tallies(
            self.fights
                .iter()
                .map(|f| f.attacks.as_slice())
                .chain(self.stages.iter().map(|s| s.attacks.as_slice())),
        )
    }

    pub fn active_secs(&self, now: u64) -> u64 {
        let span = |start: u64, end: Option<u64>| end.unwrap_or(now).saturating_sub(start);
        self.fights
            .iter()
            .map(|f| span(f.start_ts, f.end_ts))
            .chain(self.stages.iter().map(|s| span(s.start_ts, s.end_ts)))
            .sum()
    }

    pub fn groups(&self) -> Vec<std::ops::Range<usize>> {
        let mut out: Vec<std::ops::Range<usize>> = Vec::new();
        for (i, f) in self.fights.iter().enumerate() {
            let chained = i > 0 && continues(&self.fights[i - 1], &f.name, f.start_ts);
            match out.last_mut() {
                Some(g) if chained => g.end = i + 1,
                _ => out.push(i..i + 1),
            }
        }
        out
    }
}

pub fn continues(prev: &BossFight, name: &str, start_ts: u64) -> bool {
    base_name(&prev.name) == base_name(name)
        && phase_num(name) > phase_num(&prev.name)
        && prev
            .end_ts
            .is_none_or(|e| start_ts.saturating_sub(e) <= KILL_DEDUPE_SECS)
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

#[cfg(test)]
mod tests {
    use super::*;

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
            taken: 0,
            hits: 0,
            attacks: Vec::new(),
            deaths: 0,
            kill: None,
            lost: false,
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
}
