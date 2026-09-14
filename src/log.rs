use crate::state::{BossFight, GameState, Run, TakenEntry, TargetEntry};
use std::collections::VecDeque;

pub const ROW_H: i32 = 24;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Filter {
    #[default]
    All,
    Damage,
    Targets,
}

#[derive(Debug, Clone, Copy)]
pub enum Row<'a> {
    Hit(&'a TakenEntry),
    Target(&'a TargetEntry),
    Fight(&'a BossFight),
    Death { ts: u64, seq: u64 },
}

impl Row<'_> {
    fn key(&self) -> (u64, u64) {
        match self {
            Row::Hit(h) => (h.ts, h.seq),
            Row::Target(t) => (t.ts, t.seq),
            Row::Fight(f) => (f.start_ts, 0),
            Row::Death { ts, seq } => (*ts, *seq),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Source<'a> {
    pub fights: &'a [BossFight],
    pub hits: &'a VecDeque<TakenEntry>,
    pub targets: &'a VecDeque<TargetEntry>,
    pub deaths: &'a VecDeque<(u64, u64)>,
}

pub fn live(gs: &GameState) -> Option<Source<'_>> {
    gs.live_run().map(|r| Source {
        fights: &r.fights,
        hits: &gs.taken,
        targets: &gs.history,
        deaths: &gs.deaths_log,
    })
}

pub fn of_run(r: &Run) -> Source<'_> {
    Source {
        fights: &r.fights,
        hits: &r.hits,
        targets: &r.targets,
        deaths: &r.death_log,
    }
}

pub fn timeline(src: Option<Source<'_>>, filter: Filter) -> Vec<Row<'_>> {
    let Some(src) = src else {
        return Vec::new();
    };
    let mut hits = src.hits.iter().rev().peekable();
    let mut targets = src.targets.iter().rev().peekable();
    let mut deaths = src.deaths.iter().rev().peekable();
    let mut fights = src.fights.iter().rev().peekable();
    let mut out = Vec::with_capacity(src.hits.len() + src.targets.len() + 8);
    loop {
        let h = (filter != Filter::Targets)
            .then(|| hits.peek().map(|e| Row::Hit(e)))
            .flatten();
        let t = (filter != Filter::Damage)
            .then(|| targets.peek().map(|e| Row::Target(e)))
            .flatten();
        let f = fights.peek().map(|e| Row::Fight(e));
        let d = (filter != Filter::Targets)
            .then(|| {
                deaths
                    .peek()
                    .map(|(ts, seq)| Row::Death { ts: *ts, seq: *seq })
            })
            .flatten();
        let pick = [h, t, f, d].into_iter().flatten().max_by_key(|r| r.key());
        let Some(row) = pick else { break };
        match row {
            Row::Hit(_) => {
                hits.next();
            }
            Row::Target(_) => {
                targets.next();
            }
            Row::Fight(_) => {
                fights.next();
            }
            Row::Death { .. } => {
                deaths.next();
            }
        }
        out.push(row);
    }
    let mut result = Vec::with_capacity(out.len());
    for i in 0..out.len() {
        let sep = matches!(out[i], Row::Fight(_));
        let prev_event = i > 0 && !matches!(out[i - 1], Row::Fight(_));
        if !sep || prev_event {
            result.push(out[i]);
        }
    }
    result
}

pub fn content_h(rows: usize) -> i32 {
    rows as i32 * ROW_H
}

pub fn max_scroll(rows: usize, view_h: i32) -> f32 {
    (content_h(rows) - view_h).max(0) as f32
}

pub fn thumb(rows: usize, view_h: i32, scroll: f32) -> Option<(i32, i32)> {
    let content = content_h(rows);
    if content <= view_h {
        return None;
    }
    let h = ((view_h as i64 * view_h as i64 / content as i64) as i32).clamp(24, view_h);
    let max = max_scroll(rows, view_h);
    let y = ((view_h - h) as f32 * (scroll / max).clamp(0.0, 1.0)) as i32;
    Some((y, h))
}

pub fn drag_scroll(rows: usize, view_h: i32, scroll0: f32, dy: i32) -> f32 {
    let Some((_, h)) = thumb(rows, view_h, scroll0) else {
        return 0.0;
    };
    let range = (view_h - h).max(1) as f32;
    (scroll0 + dy as f32 * max_scroll(rows, view_h) / range).clamp(0.0, max_scroll(rows, view_h))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(gs: &mut GameState, t: &str, msg: &str) {
        gs.feed(&format!("2026.09.08 {t} Debug      -  {msg}"));
    }

    fn built() -> GameState {
        let mut gs = GameState::default();
        feed(
            &mut gs,
            "10:00:00",
            "damage has been taken: 1, from source: ",
        );
        feed(
            &mut gs,
            "10:00:05",
            "ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0",
        );
        feed(
            &mut gs,
            "10:00:05",
            "ownership of Yuki transferred to Alice",
        );
        feed(
            &mut gs,
            "10:00:05",
            "damage has been taken: 7, from source: (Yuki) frostBeam",
        );
        feed(&mut gs, "10:00:09", "ownership of Yuki transferred to Bob");
        feed(&mut gs, "10:00:12", "Local controller dead, switching off.");
        feed(
            &mut gs,
            "10:00:20",
            "Boss Yuki dead, personal damage dealt: ",
        );
        feed(&mut gs, "10:00:20", "STRIKE DMG: 10");
        feed(&mut gs, "10:00:20", "NON-STRIKE DMG: 0");
        feed(
            &mut gs,
            "10:01:00",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0",
        );
        gs
    }

    fn shape(rows: &[Row]) -> String {
        rows.iter()
            .map(|r| match r {
                Row::Hit(h) => format!("h{}", h.amount),
                Row::Target(t) => format!("t{}", t.player),
                Row::Fight(f) => format!("f{}", f.name),
                Row::Death { .. } => "d".to_string(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn timeline_newest_first_ties_by_seq() {
        let gs = built();
        assert_eq!(
            shape(&timeline(live(&gs), Filter::All)),
            "d tBob h7 tAlice fYuki h1"
        );
    }

    #[test]
    fn timeline_filters() {
        let gs = built();
        assert_eq!(shape(&timeline(live(&gs), Filter::Damage)), "d h7 fYuki h1");
        assert_eq!(
            shape(&timeline(live(&gs), Filter::Targets)),
            "tBob tAlice fYuki"
        );
    }

    #[test]
    fn timeline_is_scoped_to_one_run() {
        let mut gs = built();
        feed(&mut gs, "10:02:00", "ECLIPTICA - now in lobby");
        assert!(timeline(live(&gs), Filter::All).is_empty());
        assert_eq!(
            shape(&timeline(Some(of_run(&gs.runs[0])), Filter::All)),
            "d tBob h7 tAlice fYuki h1"
        );
        feed(
            &mut gs,
            "10:05:00",
            "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Blade",
        );
        feed(
            &mut gs,
            "10:05:01",
            "damage has been taken: 9, from source: ",
        );
        assert_eq!(shape(&timeline(live(&gs), Filter::All)), "h9");
        assert_eq!(
            shape(&timeline(Some(of_run(&gs.runs[0])), Filter::Targets)),
            "tBob tAlice fYuki"
        );
        assert!(timeline(None, Filter::All).is_empty());
    }

    #[test]
    fn timeline_drops_empty_separators() {
        let mut gs = GameState::default();
        feed(
            &mut gs,
            "10:00:05",
            "ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0",
        );
        assert!(timeline(live(&gs), Filter::All).is_empty());
        gs.log_rotated();
        feed(
            &mut gs,
            "11:00:00",
            "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
        );
        feed(
            &mut gs,
            "11:00:01",
            "damage has been taken: 3, from source: ",
        );
        assert_eq!(shape(&timeline(live(&gs), Filter::All)), "h3 fNan");
    }

    #[test]
    fn scroll_maths() {
        assert_eq!(max_scroll(3, 480), 0.0);
        assert_eq!(max_scroll(40, 480), (40 * ROW_H - 480) as f32);
        assert_eq!(thumb(3, 480, 0.0), None);
        let (y, h) = thumb(40, 480, 0.0).unwrap();
        assert_eq!((y, h), (0, 480 * 480 / (40 * ROW_H)));
        let (y, _) = thumb(40, 480, max_scroll(40, 480)).unwrap();
        assert_eq!(y, 480 - h);
        let (_, h) = thumb(5000, 480, 0.0).unwrap();
        assert_eq!(h, 24);
        assert_eq!(drag_scroll(3, 480, 0.0, 50), 0.0);
        let full = drag_scroll(40, 480, 0.0, 480);
        assert_eq!(full, max_scroll(40, 480));
        assert_eq!(drag_scroll(40, 480, full, -1000), 0.0);
    }
}
