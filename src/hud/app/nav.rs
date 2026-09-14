use super::App;
use crate::game::run::Run;
use crate::hud::render::Env;

impl App {
    pub fn pages(&self) -> usize {
        self.gs.runs.len() + usize::from(self.gs.live_run().is_none())
    }

    fn live_page(&self) -> usize {
        self.pages() - 1
    }

    pub fn viewed_page(&self) -> usize {
        self.sel_run
            .map_or(self.live_page(), |i| i.min(self.live_page()))
    }

    pub fn viewed_run(&self) -> Option<(usize, &Run)> {
        let p = self.viewed_page();
        self.gs.runs.get(p).map(|r| (p, r))
    }

    pub(super) fn shown_run(&self) -> Option<(usize, &Run)> {
        if self.sel_run.is_none() && self.env() != Env::InWorld {
            return None;
        }
        self.viewed_run()
    }

    pub fn group_pages(&self) -> usize {
        let n = self.viewed_run().map_or(0, |(_, r)| r.groups().len());
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

    pub fn viewed_group(&self) -> Option<(usize, std::ops::Range<usize>)> {
        let (_, run) = self.viewed_run()?;
        let groups = run.groups();
        let last = groups.len().checked_sub(1)?;
        let i = match self.sel_group {
            Some(i) => i.min(last),
            None => self.live_group_index().filter(|i| *i <= last)?,
        };
        Some((i, groups[i].clone()))
    }

    pub fn is_live(&self) -> bool {
        self.sel_run.is_none() && self.sel_group.is_none()
    }

    pub fn run_prev(&mut self) -> bool {
        let p = self.viewed_page();
        if p == 0 {
            return false;
        }
        self.sel_run = Some(p - 1);
        self.sel_group = None;
        self.log_reset();
        true
    }

    pub fn run_next(&mut self) -> bool {
        match self.sel_run {
            Some(i) => {
                self.sel_run = (i + 1 < self.live_page()).then_some(i + 1);
                self.sel_group = None;
                self.log_reset();
                true
            }
            None => false,
        }
    }

    pub fn group_prev(&mut self) -> bool {
        match self.cur_group() {
            Some(i) if i > 0 => {
                self.sel_group = Some(i - 1);
                true
            }
            _ => false,
        }
    }

    pub fn group_next(&mut self) -> bool {
        match self.sel_group {
            Some(i) => {
                let live = self.live_group_index().unwrap_or(0);
                self.sel_group = (i + 1 < live).then_some(i + 1);
                true
            }
            None => false,
        }
    }
}
