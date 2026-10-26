use super::{GameState, Mode};
use crate::game::run::{Borrowed, BossFight};

pub const MAX_MARKS: usize = 24;
const SLACK: u64 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mark {
    pub boss: bool,
    pub name: String,
    pub start: u64,
    pub end: Option<u64>,
    pub killed: bool,
    pub lost: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Summary {
    pub start: u64,
    pub stages: Option<u32>,
    pub marks: Vec<Mark>,
}

fn mark(boss: bool, f: &BossFight) -> Mark {
    Mark {
        boss,
        name: f.name.clone(),
        start: f.start_ts,
        end: f.end_ts,
        killed: boss && f.kill.is_some(),
        lost: boss && f.lost,
    }
}

fn record(m: &Mark, end: u64) -> BossFight {
    BossFight {
        name: m.name.clone(),
        start_ts: m.start,
        end_ts: Some(m.end.unwrap_or(end).max(m.start)),
        kill: m.killed.then_some((0, 0)),
        lost: m.lost,
        missed: true,
        ..Default::default()
    }
}

impl GameState {
    pub fn summary(&self) -> Option<Summary> {
        let run = self.live_run()?;
        let open = (self.stage_stats.start_ts != 0).then_some(&self.stage_stats);
        let mut marks: Vec<Mark> = run
            .stages
            .iter()
            .chain(open)
            .map(|s| mark(false, s))
            .chain(run.fights.iter().map(|f| mark(true, f)))
            .collect();
        marks.sort_by_key(|m| m.start);
        let cut = marks.len().saturating_sub(MAX_MARKS);
        marks.drain(..cut);
        Some(Summary {
            start: run.start_ts,
            stages: run.stage_count(open.is_some()),
            marks,
        })
    }

    fn first_mark(&self) -> Option<Mark> {
        let run = self.live_run()?;
        let open = (self.stage_stats.start_ts != 0).then_some(&self.stage_stats);
        let stage = run.stages.iter().chain(open).map(|s| mark(false, s));
        let fight = run.fights.iter().map(|f| mark(true, f));
        stage.chain(fight).min_by_key(|m| m.start)
    }

    pub fn catch_up(&mut self, s: &Summary) -> Option<bool> {
        if s.marks.is_empty() || s.marks.windows(2).any(|w| w[0].start > w[1].start) {
            return Some(false);
        }
        let k = match self.first_mark() {
            Some(first) => {
                let same = |m: &Mark| {
                    m.boss == first.boss
                        && m.name == first.name
                        && m.start <= first.start + SLACK
                        && m.end.is_none_or(|e| e + SLACK >= first.start)
                };
                s.marks.iter().rposition(same)?
            }
            None if self.live_run().is_some() => return None,
            None => {
                if self.mode != Mode::Intermission || s.marks.iter().any(|m| m.end.is_none()) {
                    return None;
                }
                self.open_run(s.start).borrowed = Some(Borrowed {
                    start_ts: None,
                    skipped: None,
                });
                s.marks.len()
            }
        };
        let earlier = &s.marks[..k];
        let run = self.runs.last_mut().filter(|r| r.end_ts.is_none())?;
        let start = s.start.min(earlier.first().map_or(u64::MAX, |m| m.start));
        let listed = s.marks.iter().filter(|m| !m.boss).count() as u32;
        let skipped = s.stages.and_then(|n| n.checked_sub(listed));
        let counted = run.from_start || run.borrowed.is_some_and(|b| b.skipped.is_some());
        let learns = skipped.is_some() && !counted;
        if earlier.is_empty() && start + SLACK >= run.start_ts && !learns {
            return Some(false);
        }
        let until = s.marks.get(k).map_or(start, |m| m.start);
        let stages: Vec<BossFight> = earlier
            .iter()
            .filter(|m| !m.boss)
            .map(|m| record(m, until))
            .collect();
        let fights: Vec<BossFight> = earlier
            .iter()
            .filter(|m| m.boss)
            .map(|m| record(m, until))
            .collect();
        let b = run.borrowed.get_or_insert(Borrowed {
            start_ts: Some(run.start_ts),
            skipped: None,
        });
        b.skipped = skipped.or(b.skipped);
        run.stages.splice(0..0, stages);
        run.fights.splice(0..0, fights);
        run.start_ts = run.start_ts.min(start);
        run.stage_no = run.stage_count(self.stage_stats.start_ts != 0);
        self.stage_no = run.stage_no;
        self.changed = true;
        Some(true)
    }

    pub fn needs_history(&self) -> bool {
        match self.live_run() {
            Some(r) => !r.from_start && r.borrowed.is_none_or(|b| b.skipped.is_none()),
            None => self.mode == Mode::Intermission,
        }
    }

    pub fn drop_borrowed(&mut self) {
        let Some(run) = self.runs.last_mut() else {
            return;
        };
        let Some(b) = run.borrowed.take() else {
            return;
        };
        run.stages.retain(|s| !s.missed);
        run.fights.retain(|f| !f.missed);
        let live = run.end_ts.is_none();
        let open = live && self.stage_stats.start_ts != 0;
        let own = run
            .stages
            .iter()
            .chain(&run.fights)
            .map(|f| f.start_ts)
            .chain(open.then_some(self.stage_stats.start_ts))
            .min();
        match b.start_ts.or(own) {
            Some(start) => run.start_ts = start,
            None => {
                self.runs.pop();
                self.stage_no = None;
                self.changed = true;
                return;
            }
        }
        run.stage_no = run.stage_count(open);
        if live {
            self.stage_no = run.stage_no;
        }
        self.changed = true;
    }
}
