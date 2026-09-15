use std::collections::VecDeque;

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

fn add_tally(list: &mut Vec<Tally>, who: &str, attack: &str, total: u64, hits: u32) {
    match list.iter_mut().find(|t| t.who == who && t.attack == attack) {
        Some(t) => {
            t.total += total;
            t.hits += hits;
        }
        None => list.push(Tally {
            who: who.to_string(),
            attack: attack.to_string(),
            total,
            hits,
        }),
    }
}

pub fn tally(list: &mut Vec<Tally>, who: &str, attack: &str, amount: u64) {
    add_tally(list, who, attack, amount, 1);
}

pub fn merge_tallies<'a>(groups: impl Iterator<Item = &'a [Tally]>) -> Vec<Tally> {
    let mut out = Vec::new();
    for t in groups.flatten() {
        add_tally(&mut out, &t.who, &t.attack, t.total, t.hits);
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
    pub max_hit: u64,
    pub attacks: Vec<Tally>,
    pub deaths: u32,
    pub kill: Option<(u64, u64)>,
    pub lost: bool,
}

impl BossFight {
    pub fn secs(&self, now: u64) -> u64 {
        self.end_ts.unwrap_or(now).saturating_sub(self.start_ts)
    }
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

impl StageStats {
    pub fn secs(&self, now: u64) -> u64 {
        self.end_ts.unwrap_or(now).saturating_sub(self.start_ts)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RunEnd {
    Won,
    Lost,
    Lobby,
    Left,
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
    pub end: Option<RunEnd>,
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
            .map(|f| f.max_hit)
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
        self.fights
            .iter()
            .map(|f| f.secs(now))
            .chain(self.stages.iter().map(|s| s.secs(now)))
            .sum()
    }

    pub fn group(&self, i: usize) -> Option<FightGroup<'_>> {
        let fights = &self.fights[self.groups().get(i)?.clone()];
        let last = fights.last()?;
        Some(FightGroup {
            fights,
            name: base_name(&last.name),
            start: fights[0].start_ts,
            dmg: fights.iter().map(|p| p.dmg).sum(),
            taken: fights.iter().map(|p| p.taken).sum(),
            hits: fights.iter().map(|p| p.hits).sum(),
            kill: fights
                .iter()
                .filter_map(|p| p.kill)
                .reduce(|a, b| (a.0 + b.0, a.1 + b.1)),
            last,
            n_phases: fights.len(),
        })
    }

    pub fn groups(&self) -> Vec<std::ops::Range<usize>> {
        let mut out: Vec<std::ops::Range<usize>> = Vec::new();
        for (i, f) in self.fights.iter().enumerate() {
            let chained = i > 0 && continues(&self.fights[i - 1], &f.name);
            match out.last_mut() {
                Some(g) if chained => g.end = i + 1,
                _ => out.push(i..i + 1),
            }
        }
        out
    }
}

pub struct FightGroup<'a> {
    pub fights: &'a [BossFight],
    pub name: &'a str,
    pub start: u64,
    pub dmg: u64,
    pub taken: u64,
    pub hits: u32,
    pub kill: Option<(u64, u64)>,
    pub last: &'a BossFight,
    pub n_phases: usize,
}

impl FightGroup<'_> {
    pub fn duration(&self, now: u64) -> u64 {
        self.last.end_ts.unwrap_or(now).saturating_sub(self.start)
    }

    pub fn dps(&self, now: u64) -> u64 {
        self.dmg / self.duration(now).max(1)
    }

    pub fn result(&self) -> &'static str {
        match (self.last.kill, self.last.end_ts) {
            _ if self.last.lost => "lost",
            (Some(_), _) => "killed",
            (None, Some(_)) => "unfinished",
            (None, None) => "in progress",
        }
    }
}

pub fn final_phase(name: &str) -> bool {
    base_name(name) == "JimBringer" && phase_num(name) == 3
}

pub fn continues(prev: &BossFight, name: &str) -> bool {
    base_name(&prev.name) == base_name(name) && phase_num(name) > phase_num(&prev.name)
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
            max_hit: 0,
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
