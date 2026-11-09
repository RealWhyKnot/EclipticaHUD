mod anim;
mod archive;
mod logwin;
mod nav;
mod party;
mod settings;
#[cfg(test)]
mod tests;
mod tick;

pub(crate) use settings::{WINDOW_STEPS, fit_scale};

use crate::discord::{self, Link, Presence};
use crate::game::state::GameState;
use crate::history::History;
use crate::hud::flags::{Flag, Flags};
use crate::hud::render::{
    Env, Hit, Info, LOG_H, LOG_W, LOGICAL_H, LOGICAL_W, LogHit, MAX_ALPHA, MAX_SCALE, MIN_ALPHA,
    MIN_SCALE, PARTY_H, PARTY_W, PartyHit, Renderer, RunSync, tip_ready,
};
use crate::hud::timeline::Filter;
use crate::update::Badge;
use crate::vr::VrOverlay;
use crate::vrchat::log::{LogWatch, log_dir};
use std::time::Instant;
use windows_sys::Win32::Foundation::HWND;

pub(crate) struct LogState {
    pub hwnd: HWND,
    pub dpi: u32,
    pub renderer: Renderer,
    pub visible: bool,
    pub filter: Filter,
    pub scroll: f32,
    pub scroll_shown: f32,
    pub drag: Option<(i32, f32)>,
    pub hover: Option<LogHit>,
    pub hover_at: Option<Instant>,
    pub tracking: bool,
    pub alpha: u8,
    pub alpha_shown: u8,
    wheel_acc: i32,
    opened_at: Option<Instant>,
    thumb_t: f32,
    thumb_seen: Option<Instant>,
    slide_at: Option<Instant>,
    rows: usize,
}

pub(crate) struct PartyWin {
    pub hwnd: HWND,
    pub dpi: u32,
    pub renderer: Renderer,
    pub visible: bool,
    pub hover: Option<PartyHit>,
    pub hover_at: Option<Instant>,
    pub pressed: Option<PartyHit>,
    pub tracking: bool,
    pub alpha: u8,
    pub alpha_shown: u8,
    opened_at: Option<Instant>,
    vr_rgba: Vec<u8>,
    last_tip: Option<PartyHit>,
}

pub(crate) struct App {
    pub flags: Flags,
    pub gs: GameState,
    pub watch: LogWatch,
    archive: archive::Archive,
    pub renderer: Renderer,
    pub log: LogState,
    pub party_win: PartyWin,
    vr: VrOverlay,
    rgba: Vec<u8>,
    last_ts: u64,
    last_ts_at: Instant,
    pub hover: Option<Hit>,
    pub hover_at: Option<Instant>,
    pub pressed: Option<Hit>,
    board_armed: Option<Instant>,
    pub board_status: u8,
    party: party::PartyState,
    presence: Option<Presence>,
    presence_at: Option<Instant>,
    link: Link,
    pub sel_run: Option<usize>,
    pub sel_group: Option<usize>,
    vrc_checked: Option<Instant>,
    last_env: Option<Env>,
    pub dpi: u32,
    pub scale: f32,
    pub scale_cap: f32,
    pub alpha: u8,
    pub split_top: usize,
    split_wheel_acc: i32,
    split_seen: Option<(usize, usize)>,
    flash_at: Option<Instant>,
    warn_at: Option<Instant>,
    token_alerts_seen: u64,
    last_target_since: u64,
    progress_shown: f32,
    taken_flash_at: Option<Instant>,
    last_taken_seq: u64,
    dps_shown: f32,
    fight_dps_shown: f32,
    taken_shown: f32,
    taken_rate_shown: f32,
    dead_since: Option<Instant>,
    last_tip: Option<Hit>,
    last_sync: RunSync,
    last_log_tip: Option<LogHit>,
    timer_ms: u32,
    badge: Badge,
}

static BLIP: &[u8] = include_bytes!("../../../assets/target.wav");
static TOKENS: &[u8] = include_bytes!("../../../assets/tokens.wav");
static YOU: &[u8] = include_bytes!("../../../assets/target_you.wav");

fn play(wav: &'static [u8]) {
    let _ = wav;
    #[cfg(not(test))]
    // SAFETY: `wav` is a 'static buffer, which SND_ASYNC with SND_MEMORY needs to stay valid while it plays.
    unsafe {
        use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
        PlaySoundW(
            wav.as_ptr().cast(),
            std::ptr::null_mut(),
            SND_ASYNC | SND_MEMORY | SND_NODEFAULT,
        );
    }
}

impl App {
    pub(crate) fn new(dpi: u32) -> Self {
        App {
            gs: GameState::default(),
            watch: LogWatch::new(),
            archive: archive::Archive::default(),
            renderer: Renderer::new(dpi, LOGICAL_W, LOGICAL_H),
            log: LogState {
                hwnd: std::ptr::null_mut(),
                dpi,
                renderer: Renderer::new(dpi, LOG_W, LOG_H),
                visible: false,
                filter: Filter::All,
                scroll: 0.0,
                scroll_shown: 0.0,
                drag: None,
                hover: None,
                hover_at: None,
                tracking: false,
                alpha: 0,
                alpha_shown: 0,
                wheel_acc: 0,
                opened_at: None,
                thumb_t: 0.0,
                thumb_seen: None,
                slide_at: None,
                rows: 0,
            },
            party_win: PartyWin {
                hwnd: std::ptr::null_mut(),
                dpi,
                renderer: Renderer::new(dpi, PARTY_W, PARTY_H),
                visible: false,
                hover: None,
                hover_at: None,
                pressed: None,
                tracking: false,
                alpha: 0,
                alpha_shown: 0,
                opened_at: None,
                vr_rgba: Vec::new(),
                last_tip: None,
            },
            vr: VrOverlay::new(),
            rgba: Vec::new(),
            last_ts: 0,
            last_ts_at: Instant::now(),
            hover: None,
            hover_at: None,
            pressed: None,
            flags: Flags::default()
                .with(Flag::Tracking, false)
                .with(Flag::Sound, true)
                .with(Flag::Topmost, true)
                .with(Flag::Discord, false)
                .with(Flag::Usage, false)
                .with(Flag::UsageFailing, false)
                .with(Flag::Board, false)
                .with(Flag::BoardDeleting, false)
                .with(Flag::UsageAsk, false)
                .with(Flag::Party, false)
                .with(Flag::VrcRunning, true)
                .with(Flag::SettingsOpen, false)
                .with(Flag::Stages, false)
                .with(Flag::Primed, false)
                .with(Flag::WasDead, false)
                .with(Flag::PromptOpen, false),

            board_armed: None,

            board_status: crate::board::OK,

            party: party::PartyState::new(),
            presence: None,
            presence_at: None,
            link: Link::Off,
            sel_run: None,
            sel_group: None,

            vrc_checked: None,
            last_env: None,
            dpi,
            scale: 1.0,
            scale_cap: MAX_SCALE,
            alpha: MAX_ALPHA,

            split_top: 0,
            split_wheel_acc: 0,
            split_seen: None,
            flash_at: None,
            warn_at: None,
            token_alerts_seen: 0,
            last_target_since: 0,
            progress_shown: 0.0,
            taken_flash_at: None,
            last_taken_seq: 0,

            dps_shown: 0.0,
            fight_dps_shown: 0.0,
            taken_shown: 0.0,
            taken_rate_shown: 0.0,
            dead_since: None,

            last_tip: None,
            last_sync: RunSync::Idle,
            last_log_tip: None,
            timer_ms: 1000,
            badge: Badge::None,
        }
    }

    pub(crate) fn rescale(&mut self) {
        let scale = self.shown_scale();
        let eff = |dpi: u32| crate::num::to_u32((crate::num::to_f32(dpi) * scale).round());
        self.renderer = Renderer::new(eff(self.dpi), LOGICAL_W, LOGICAL_H);
        self.log.renderer = Renderer::new(eff(self.log.dpi), LOG_W, LOG_H);
        self.party_win.renderer = Renderer::new(eff(self.party_win.dpi), PARTY_W, PARTY_H);
        self.rgba.clear();
        self.party_win.vr_rgba.clear();
    }

    pub(crate) fn alpha_byte(&self) -> u8 {
        crate::num::to_u8(u32::from(self.alpha) * 255 / 100)
    }

    pub(crate) fn update_ready(&self) -> bool {
        matches!(self.badge, Badge::Ready(_) | Badge::Failed(_))
    }

    pub(crate) fn prompt_shown(&self) -> bool {
        self.flags.has(Flag::PromptOpen) && self.badge != Badge::None && self.gs.boss.is_none()
    }

    pub(crate) fn can_restart(&self) -> bool {
        self.gs.boss.is_none()
    }

    pub(crate) fn usage_prompt_shown(&self) -> bool {
        self.flags.has(Flag::UsageAsk) && !self.prompt_shown() && self.gs.boss.is_none()
    }

    pub(crate) fn skip_update(&mut self) -> Option<String> {
        self.flags.set(Flag::PromptOpen, false);
        let tag = crate::update::skip();
        self.badge = crate::update::badge();
        tag
    }

    pub(crate) fn set_history(&mut self, db: Option<History>) {
        self.archive.db = db;
    }

    pub(crate) fn backfill_history(&mut self) {
        let dir = log_dir();
        archive::backfill(&mut self.gs, dir.as_deref(), self.archive.db.as_mut());
    }

    pub(crate) fn env(&self) -> Env {
        if !self.flags.has(Flag::VrcRunning) {
            Env::NoVrchat
        } else if !self.gs.in_ecliptica() {
            Env::NotInWorld
        } else {
            Env::InWorld
        }
    }

    pub(crate) fn hit_enabled(&self, hit: Hit) -> bool {
        if self.prompt_shown() {
            return match hit {
                Hit::UpdateNow => self.update_ready(),
                Hit::Close | Hit::Log | Hit::Pin | Hit::Party | Hit::UpdateLater => true,
                Hit::UpdateSkip => matches!(self.badge, Badge::Ready(_)),
                _ => false,
            };
        }
        if self.usage_prompt_shown() {
            return matches!(
                hit,
                Hit::Close | Hit::Log | Hit::Pin | Hit::Party | Hit::UsageYes | Hit::UsageNo
            );
        }
        match hit {
            Hit::Discord | Hit::Sound | Hit::Usage | Hit::Board => {
                self.flags.has(Flag::SettingsOpen)
            }
            Hit::BoardDelete => {
                self.flags.has(Flag::SettingsOpen) && !self.flags.has(Flag::BoardDeleting)
            }
            Hit::ScaleDown => {
                self.flags.has(Flag::SettingsOpen) && self.shown_scale() > MIN_SCALE + 0.001
            }
            Hit::ScaleUp => {
                self.flags.has(Flag::SettingsOpen) && self.shown_scale() < self.scale_max() - 0.001
            }
            Hit::AlphaDown => self.flags.has(Flag::SettingsOpen) && self.alpha > MIN_ALPHA,
            Hit::AlphaUp => self.flags.has(Flag::SettingsOpen) && self.alpha < MAX_ALPHA,
            Hit::WindowDown => {
                self.flags.has(Flag::SettingsOpen) && self.gs.win() > WINDOW_STEPS[0]
            }
            Hit::WindowUp => {
                self.flags.has(Flag::SettingsOpen)
                    && self.gs.win() < WINDOW_STEPS[WINDOW_STEPS.len() - 1]
            }
            Hit::Info(
                Info::Dealt(_)
                | Info::DealtGraph
                | Info::LastKill
                | Info::Taken(_)
                | Info::TakenGraph
                | Info::LastHit
                | Info::Deaths
                | Info::TakenMore(_)
                | Info::TopAttacks,
            ) if self.stages_shown() => false,
            Hit::Mute
            | Hit::FightPrev
            | Hit::FightNext
            | Hit::Stages
            | Hit::Splits
            | Hit::Info(
                Info::Boss
                | Info::Result
                | Info::Pages
                | Info::Target
                | Info::Dealt(_)
                | Info::DealtGraph
                | Info::LastKill,
            ) if self.flags.has(Flag::SettingsOpen) => false,
            Hit::Info(i) if self.flags.has(Flag::SettingsOpen) && i.under_settings() => false,
            Hit::Info(Info::Version) => !self.update_ready(),
            Hit::Info(Info::Pages) => !self.is_live() && self.viewed_group().is_some(),
            Hit::Info(i) if i.live_only() => self.is_live(),
            Hit::Info(i) if i.history_only() => !self.is_live(),
            Hit::Close | Hit::Log | Hit::Pin | Hit::Settings | Hit::Party | Hit::Info(_) => true,
            Hit::Update => self.badge != Badge::None,
            Hit::UpdateNow | Hit::UpdateLater | Hit::UpdateSkip | Hit::UsageYes | Hit::UsageNo => {
                false
            }
            Hit::Mute => self.is_live() && self.gs.boss.is_some(),
            Hit::RunPrev => self.viewed_page() > 0,
            Hit::RunNext => self.sel_run.is_some(),
            Hit::FightPrev => self.cur_group().is_some_and(|i| i > 0),
            Hit::FightNext => self.sel_group.is_some(),
            Hit::Stages => self.stages_ok(),
            Hit::Splits => self.stages_shown(),
        }
    }

    pub(crate) fn enabled_hit(&self, x: i32, y: i32) -> Option<Hit> {
        self.renderer.hit_test_where(x, y, |h| self.hit_enabled(h))
    }

    pub(crate) fn activate(&mut self, hit: Hit) -> bool {
        match hit {
            Hit::Mute | Hit::Sound => {
                self.flags.set(Flag::Sound, !self.flags.has(Flag::Sound));
                true
            }
            Hit::RunPrev => self.run_prev(),
            Hit::RunNext => self.run_next(),
            Hit::FightPrev => self.group_prev(),
            Hit::FightNext => self.group_next(),
            Hit::Log => {
                self.log_toggle();
                true
            }
            Hit::Pin => {
                self.flags
                    .set(Flag::Topmost, !self.flags.has(Flag::Topmost));
                true
            }
            Hit::Discord => {
                self.set_discord(!self.flags.has(Flag::Discord));
                true
            }
            Hit::Usage => {
                self.set_usage(!self.flags.has(Flag::Usage));
                true
            }
            Hit::Board => {
                self.set_board(!self.flags.has(Flag::Board));
                true
            }
            Hit::BoardDelete => {
                let now = Instant::now();
                if self
                    .board_armed
                    .is_some_and(|t| now.duration_since(t) < anim::DELETE_ARM)
                {
                    self.board_armed = None;
                    self.delete_board();
                } else {
                    self.board_armed = Some(now);
                }
                true
            }
            Hit::Settings => {
                self.flags
                    .set(Flag::SettingsOpen, !self.flags.has(Flag::SettingsOpen));
                true
            }
            Hit::Stages => {
                self.flags.set(Flag::Stages, !self.flags.has(Flag::Stages));
                self.split_follow();
                true
            }
            Hit::Party => {
                self.party_toggle();
                true
            }
            Hit::ScaleDown => self.step_scale(-1.0),
            Hit::ScaleUp => self.step_scale(1.0),
            Hit::AlphaDown => self.step_alpha(false),
            Hit::AlphaUp => self.step_alpha(true),
            Hit::WindowDown => self.step_window(false),
            Hit::WindowUp => self.step_window(true),
            Hit::Update => {
                self.flags.set(Flag::PromptOpen, true);
                true
            }
            Hit::UpdateLater => {
                self.flags.set(Flag::PromptOpen, false);
                true
            }
            Hit::UsageYes | Hit::UsageNo => {
                self.flags.set(Flag::UsageAsk, false);
                self.set_usage(hit == Hit::UsageYes);
                true
            }
            Hit::Close | Hit::UpdateNow | Hit::UpdateSkip | Hit::Splits | Hit::Info(_) => false,
        }
    }

    pub(crate) fn set_usage(&mut self, on: bool) {
        if on && let Some(db) = self.archive.db.as_mut() {
            db.usage_start();
        }
        self.store_usage(on);
    }

    pub(crate) fn restore_usage(&mut self, on: bool) {
        if on
            && let Some(db) = self.archive.db.as_mut()
            && !db.usage_cursors().contains_key("")
        {
            db.usage_start();
        }
        self.store_usage(on);
    }

    fn store_usage(&mut self, on: bool) {
        self.flags.set(Flag::Usage, on);
        crate::usage::ON.store(on, std::sync::atomic::Ordering::Relaxed);
    }

    pub(crate) fn set_board(&mut self, on: bool) {
        self.flags.set(Flag::Board, on);
        crate::board::ON.store(on, std::sync::atomic::Ordering::Relaxed);
    }

    pub(crate) fn delete_board(&mut self) {
        self.flags.set(Flag::BoardDeleting, true);
        crate::board::forget();
    }

    pub(crate) fn set_vr_config(&mut self, config: crate::vr::VrConfig) {
        self.vr.set_config(config);
    }

    pub(crate) fn set_discord(&mut self, on: bool) {
        self.flags.set(Flag::Discord, on);
        self.presence_at = None;
        if on {
            discord::register_scheme();
            self.presence.get_or_insert_with(Presence::new);
        } else if let Some(p) = self.presence.as_mut() {
            p.off();
        }
    }

    fn feed_presence(&mut self, now: u64) {
        if !self.flags.has(Flag::Discord)
            || self
                .presence_at
                .is_some_and(|t| t.elapsed().as_millis() < 1000)
        {
            return;
        }
        self.presence_at = Some(Instant::now());
        let unix_now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let activity = if self.flags.has(Flag::VrcRunning) {
            discord::activity(&self.gs, now, unix_now)
        } else {
            None
        };
        if let Some(p) = self.presence.as_mut() {
            p.show(activity);
        }
    }

    pub(crate) fn set_hover(&mut self, over: Option<Hit>) -> bool {
        if over == self.hover {
            return false;
        }
        self.hover = over;
        self.hover_at = over.map(|_| Instant::now());
        true
    }

    pub(crate) fn tip(&self) -> Option<Hit> {
        self.hover.filter(|_| tip_ready(self.hover_at))
    }
}
