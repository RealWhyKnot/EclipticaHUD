use super::App;
use crate::game::run::{Page, Run, Split};
use crate::hud::flags::Flag;
use crate::hud::render::{Env, SPLIT_ROWS};

const SPLIT_STEP: i32 = 3;

impl App {
    pub(crate) fn pages(&self) -> usize {
        self.gs.runs.len() + usize::from(self.gs.live_run().is_none())
    }

    fn live_page(&self) -> usize {
        self.pages() - 1
    }

    pub(crate) fn viewed_page(&self) -> usize {
        self.sel_run
            .map_or(self.live_page(), |i| i.min(self.live_page()))
    }

    pub(crate) fn viewed_run(&self) -> Option<(usize, &Run)> {
        let p = self.viewed_page();
        self.gs.runs.get(p).map(|r| (p, r))
    }

    pub(super) fn shown_run(&self) -> Option<(usize, &Run)> {
        if self.sel_run.is_none() && self.env() != Env::InWorld {
            return None;
        }
        self.viewed_run()
    }

    pub(crate) fn group_pages(&self) -> usize {
        let n = self.viewed_run().map_or(0, |(_, r)| r.pages().len());
        n + usize::from(self.sel_run.is_none() && self.gs.boss.is_none())
    }

    fn live_group_index(&self) -> Option<usize> {
        self.group_pages().checked_sub(1)
    }

    pub(super) fn cur_group(&self) -> Option<usize> {
        match self.viewed_group() {
            Some((i, _)) => Some(i),
            None => self.live_group_index(),
        }
    }

    pub(crate) fn viewed_group(&self) -> Option<(usize, Page)> {
        let (_, run) = self.viewed_run()?;
        let pages = run.pages();
        let last = pages.len().checked_sub(1)?;
        let i = match self.sel_group {
            Some(i) => i.min(last),
            None => self.live_group_index().filter(|i| *i <= last)?,
        };
        Some((i, pages[i].clone()))
    }

    pub(crate) fn is_live(&self) -> bool {
        self.sel_run.is_none() && self.sel_group.is_none()
    }

    pub(crate) fn run_prev(&mut self) -> bool {
        let p = self.viewed_page();
        if p == 0 {
            return false;
        }
        self.sel_run = Some(p - 1);
        self.sel_group = None;
        self.log_reset();
        self.split_top = 0;
        self.split_follow();
        true
    }

    pub(crate) fn run_next(&mut self) -> bool {
        match self.sel_run {
            Some(i) => {
                self.sel_run = (i + 1 < self.live_page()).then_some(i + 1);
                self.sel_group = None;
                self.log_reset();
                self.split_top = 0;
                self.split_follow();
                true
            }
            None => false,
        }
    }

    pub(crate) fn group_prev(&mut self) -> bool {
        match self.cur_group() {
            Some(i) if i > 0 => {
                self.sel_group = Some(i - 1);
                self.split_follow();
                true
            }
            _ => false,
        }
    }

    pub(crate) fn group_next(&mut self) -> bool {
        match self.sel_group {
            Some(i) => {
                let live = self.live_group_index().unwrap_or(0);
                self.sel_group = (i + 1 < live).then_some(i + 1);
                self.split_follow();
                true
            }
            None => false,
        }
    }

    pub(crate) fn splits(&self) -> Vec<Split> {
        match self.shown_run() {
            Some((i, _)) => self.gs.run_splits(i, self.now()),
            None => Vec::new(),
        }
    }

    fn split_cur(&self) -> Option<usize> {
        if self.is_live() {
            return self.splits().len().checked_sub(1);
        }
        let (_, run) = self.shown_run()?;
        let (_, page) = self.viewed_group()?;
        Some(crate::num::to_usize(run.stage_of(&page)) - 1)
    }

    pub(crate) fn stages_ok(&self) -> bool {
        (self.is_live() || self.viewed_group().is_some()) && !self.splits().is_empty()
    }

    pub(crate) fn stages_shown(&self) -> bool {
        self.flags.has(Flag::Stages) && self.stages_ok()
    }

    fn split_max(&self) -> usize {
        self.splits().len().saturating_sub(SPLIT_ROWS)
    }

    pub(super) fn split_track(&mut self) {
        let seen = self.shown_run().map(|(i, _)| i).zip(self.split_cur());
        if seen != self.split_seen {
            self.split_seen = seen;
            self.split_follow();
        }
    }

    pub(super) fn split_follow(&mut self) {
        if let Some(c) = self.split_cur() {
            if c < self.split_top {
                self.split_top = c;
            } else if c >= self.split_top + SPLIT_ROWS {
                self.split_top = c + 1 - SPLIT_ROWS;
            }
        }
        self.split_top = self.split_top.min(self.split_max());
    }

    pub(crate) fn split_wheel(&mut self, delta: i32) -> bool {
        self.split_wheel_acc += delta;
        let steps = self.split_wheel_acc / super::logwin::WHEEL_DELTA;
        self.split_wheel_acc -= steps * super::logwin::WHEEL_DELTA;
        let top = crate::num::to_i32(self.split_top) - steps * SPLIT_STEP;
        let top = crate::num::to_usize(top.clamp(0, crate::num::to_i32(self.split_max())));
        let moved = top != self.split_top;
        self.split_top = top;
        moved
    }
}
