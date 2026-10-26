use super::hit::{Hit, LogHit, PartyHit};
use super::Ctl;
use crate::discord::Link;
use crate::hud::timeline::Filter;
use crate::relay::party::PartyRow;
use crate::update::Badge;
use crate::vr::VrStatus;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RunSync {
    #[default]
    Idle,
    Waiting,
    Done,
}

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
    pub scale_max: f32,
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
    pub dps_shown: f32,
    pub fight_dps_shown: f32,
    pub taken_shown: f32,
    pub taken_rate_shown: f32,
    pub update: Badge,
    pub prompt: bool,
    pub sound_on: bool,
    pub discord: Link,
    pub discord_on: bool,
    pub usage_on: bool,
    pub usage_failing: bool,
    pub board_on: bool,
    pub board_status: u8,
    pub board_delete: u8,
    pub ask_usage: bool,
    pub party_open: bool,
    pub view_run: Option<usize>,
    pub view_group: Option<usize>,
    pub group_pages: usize,
    pub run_sel: bool,
    pub group_sel: bool,
    pub stages_btn: bool,
    pub stages: bool,
    pub split_top: usize,
    pub sync: RunSync,
}

impl Frame {
    pub(super) fn live(&self) -> bool {
        !self.run_sel && !self.group_sel
    }

    pub(super) fn hov(&self, h: Hit) -> bool {
        self.hover == Some(h)
    }

    pub(super) fn prs(&self, h: Hit) -> bool {
        self.pressed == Some(h)
    }

    pub(super) fn ctl(&self, h: Hit, enabled: bool, on: bool) -> Ctl {
        Ctl::of(enabled, on, self.hov(h), self.prs(h))
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

#[derive(Default, Clone, PartialEq, Eq, Debug)]
pub struct PartyFight {
    pub name: String,
    pub secs: u64,
    pub live: bool,
    pub down: bool,
}

#[derive(Default, Clone, PartialEq, Eq, Debug)]
pub struct PartyPane {
    pub on: bool,
    pub ready: bool,
    pub link: crate::relay::Link,
    pub here: usize,
    pub fight: Option<PartyFight>,
    pub rows: Vec<PartyRow>,
    pub saved: Option<u64>,
    pub page: usize,
    pub pages: usize,
}

impl PartyPane {
    pub fn you(&self) -> Option<usize> {
        self.rows.iter().position(|r| r.you)
    }
}

pub struct PartyView<'a> {
    pub pane: &'a PartyPane,
    pub hover: Option<PartyHit>,
    pub pressed: Option<PartyHit>,
    pub tip: Option<PartyHit>,
    pub vr: bool,
}

impl PartyView<'_> {
    pub(super) fn ctl(&self, h: PartyHit, on: bool) -> Ctl {
        Ctl::of(true, on, self.hover == Some(h), self.pressed == Some(h))
    }

    pub(super) fn ctl_if(&self, h: PartyHit, enabled: bool) -> Ctl {
        Ctl::of(
            enabled,
            false,
            self.hover == Some(h),
            self.pressed == Some(h),
        )
    }
}
