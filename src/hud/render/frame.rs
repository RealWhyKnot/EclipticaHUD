use super::hit::{Hit, LogHit};
use crate::discord::Link;
use crate::hud::timeline::Filter;
use crate::update::Badge;
use crate::vr::VrStatus;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Env {
    NoVrchat,
    NotInWorld,
    InWorld,
}

pub struct Frame {
    pub now: u64,
    pub env: Env,
    pub pages: usize,
    pub view_page: usize,
    pub settings_open: bool,
    pub scale: f32,
    pub alpha: u8,
    pub window: u64,
    pub flash_t: f32,
    pub taken_flash_t: f32,
    pub warn_t: f32,
    pub dead_pulse: f32,
    pub hover: Option<Hit>,
    pub pressed: Option<Hit>,
    pub tip: Option<Hit>,
    pub vr: VrStatus,
    pub log_ok: bool,
    pub log_open: bool,
    pub topmost: bool,
    pub progress_shown: f32,
    pub dps_frac_shown: f32,
    pub taken_frac_shown: f32,
    pub dps_shown: f32,
    pub fight_dps_shown: f32,
    pub taken_shown: f32,
    pub taken_rate_shown: f32,
    pub update: Badge,
    pub sound_on: bool,
    pub discord: Link,
    pub discord_on: bool,
    pub view_run: Option<usize>,
    pub view_group: Option<usize>,
    pub group_pages: usize,
    pub run_sel: bool,
    pub group_sel: bool,
}

impl Frame {
    pub(super) fn live(&self) -> bool {
        !self.run_sel && !self.group_sel
    }

    pub(super) fn empty(&self) -> bool {
        self.live() && (self.env != Env::InWorld || self.view_run.is_none())
    }
}

pub struct LogView {
    pub scroll: f32,
    pub filter: Filter,
    pub hover: Option<LogHit>,
    pub dragging: bool,
    pub thumb_t: f32,
    pub slide: f32,
    pub tip: Option<LogHit>,
    pub thumb: Option<(i32, i32)>,
}
