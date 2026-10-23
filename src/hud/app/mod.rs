mod anim;
mod logwin;
mod nav;
mod party;
mod settings;
#[cfg(test)]
mod tests;
mod tick;

pub use settings::WINDOW_STEPS;

use crate::discord::{self, Link, Presence};
use crate::game::state::GameState;
use crate::hud::render::{
    tip_ready, Env, Hit, Info, LogHit, PartyHit, Renderer, LOGICAL_H, LOGICAL_W, LOG_H, LOG_W,
    MAX_ALPHA, MAX_SCALE, MIN_ALPHA, MIN_SCALE, PARTY_H, PARTY_W,
};
use crate::hud::timeline::Filter;
use crate::update::Badge;
use crate::vr::VrOverlay;
use crate::vrchat::log::{all_logs, log_dir, read_lines, LogWatch};
use std::path::Path;
use std::time::Instant;
use windows_sys::Win32::Foundation::HWND;

pub struct LogState {
    pub hwnd: HWND,
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

pub struct PartyWin {
    pub hwnd: HWND,
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

pub struct App {
    pub gs: GameState,
    pub watch: LogWatch,
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
    pub tracking: bool,
    pub sound_on: bool,
    pub topmost: bool,
    pub discord_on: bool,
    pub party_on: bool,
    party: party::PartyState,
    presence: Option<Presence>,
    presence_at: Option<Instant>,
    link: Link,
    pub sel_run: Option<usize>,
    pub sel_group: Option<usize>,
    pub vrc_running: bool,
    vrc_checked: Option<Instant>,
    last_env: Option<Env>,
    pub dpi: u32,
    pub scale: f32,
    pub alpha: u8,
    pub settings_open: bool,
    flash_at: Option<Instant>,
    warn_at: Option<Instant>,
    token_alerts_seen: u64,
    last_target_since: u64,
    progress_shown: f32,
    dps_boss: Option<String>,
    taken_peak: u64,
    taken_frac_shown: f32,
    taken_flash_at: Option<Instant>,
    last_taken_seq: u64,
    primed: bool,
    dps_shown: f32,
    fight_dps_shown: f32,
    taken_shown: f32,
    taken_rate_shown: f32,
    dead_since: Option<Instant>,
    was_dead: bool,
    last_tip: Option<Hit>,
    last_log_tip: Option<LogHit>,
    timer_ms: u32,
    badge: Badge,
    prompt_open: bool,
}

static BLIP: &[u8] = include_bytes!("../../../assets/target.wav");
static TOKENS: &[u8] = include_bytes!("../../../assets/tokens.wav");
static YOU: &[u8] = include_bytes!("../../../assets/target_you.wav");

fn play(wav: &'static [u8]) {
    let _ = wav;
    #[cfg(not(test))]
    unsafe {
        use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
        PlaySoundW(
            wav.as_ptr() as _,
            std::ptr::null_mut(),
            SND_ASYNC | SND_MEMORY | SND_NODEFAULT,
        );
    }
}

fn backfill(gs: &mut GameState, dir: &Path) {
    let logs = all_logs(dir);
    let Some((_, old)) = logs.split_last() else {
        return;
    };
    for path in old {
        if read_lines(path, |line| {
            gs.feed(line);
        })
        .is_ok()
        {
            gs.log_rotated();
        }
    }
}

impl App {
    pub fn new(dpi: u32) -> Self {
        App {
            gs: GameState::default(),
            watch: LogWatch::new(),
            renderer: Renderer::new(dpi, LOGICAL_W, LOGICAL_H),
            log: LogState {
                hwnd: std::ptr::null_mut(),
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
            tracking: false,
            sound_on: true,
            topmost: true,
            discord_on: false,
            party_on: false,
            party: party::PartyState::new(),
            presence: None,
            presence_at: None,
            link: Link::Off,
            sel_run: None,
            sel_group: None,
            vrc_running: true,
            vrc_checked: None,
            last_env: None,
            dpi,
            scale: 1.0,
            alpha: MAX_ALPHA,
            settings_open: false,
            flash_at: None,
            warn_at: None,
            token_alerts_seen: 0,
            last_target_since: 0,
            progress_shown: 0.0,
            dps_boss: None,
            taken_peak: 0,
            taken_frac_shown: 0.0,
            taken_flash_at: None,
            last_taken_seq: 0,
            primed: false,
            dps_shown: 0.0,
            fight_dps_shown: 0.0,
            taken_shown: 0.0,
            taken_rate_shown: 0.0,
            dead_since: None,
            was_dead: false,
            last_tip: None,
            last_log_tip: None,
            timer_ms: 1000,
            badge: Badge::None,
            prompt_open: false,
        }
    }

    pub fn rescale(&mut self) {
        let eff = (self.dpi as f32 * self.scale).round() as u32;
        self.renderer = Renderer::new(eff, LOGICAL_W, LOGICAL_H);
        self.log.renderer = Renderer::new(eff, LOG_W, LOG_H);
        self.party_win.renderer = Renderer::new(eff, PARTY_W, PARTY_H);
        self.rgba.clear();
        self.party_win.vr_rgba.clear();
    }

    pub fn alpha_byte(&self) -> u8 {
        (self.alpha as u32 * 255 / 100) as u8
    }

    pub fn update_ready(&self) -> bool {
        matches!(self.badge, Badge::Ready(_) | Badge::Failed(_))
    }

    pub fn prompt_shown(&self) -> bool {
        self.prompt_open && self.badge != Badge::None && self.gs.boss.is_none()
    }

    pub fn skip_update(&mut self) -> Option<String> {
        self.prompt_open = false;
        let tag = crate::update::skip();
        self.badge = crate::update::badge();
        tag
    }

    pub fn backfill_history(&mut self) {
        if let Some(dir) = log_dir() {
            backfill(&mut self.gs, &dir);
        }
    }

    pub fn env(&self) -> Env {
        if !self.vrc_running {
            Env::NoVrchat
        } else if !self.gs.in_ecliptica() {
            Env::NotInWorld
        } else {
            Env::InWorld
        }
    }

    pub fn hit_enabled(&self, hit: Hit) -> bool {
        if self.prompt_shown() {
            return match hit {
                Hit::Close | Hit::Log | Hit::Pin | Hit::Party => true,
                Hit::UpdateNow | Hit::UpdateLater => self.update_ready(),
                Hit::UpdateSkip => matches!(self.badge, Badge::Ready(_)),
                _ => false,
            };
        }
        match hit {
            Hit::Close | Hit::Log | Hit::Pin | Hit::Settings | Hit::Party => true,
            Hit::Discord | Hit::Sound => self.settings_open,
            Hit::ScaleDown => self.settings_open && self.scale > MIN_SCALE + 0.001,
            Hit::ScaleUp => self.settings_open && self.scale < MAX_SCALE - 0.001,
            Hit::AlphaDown => self.settings_open && self.alpha > MIN_ALPHA,
            Hit::AlphaUp => self.settings_open && self.alpha < MAX_ALPHA,
            Hit::WindowDown => self.settings_open && self.gs.win() > WINDOW_STEPS[0],
            Hit::WindowUp => {
                self.settings_open && self.gs.win() < WINDOW_STEPS[WINDOW_STEPS.len() - 1]
            }
            Hit::Mute | Hit::FightPrev | Hit::FightNext if self.settings_open => false,
            Hit::Info(
                Info::Boss
                | Info::Result
                | Info::Target
                | Info::Dealt(_)
                | Info::DealtGraph
                | Info::LastKill,
            ) if self.settings_open => false,
            Hit::Info(Info::Version) => !self.update_ready(),
            Hit::Info(i) if i.live_only() => self.is_live(),
            Hit::Info(i) if i.history_only() => !self.is_live(),
            Hit::Info(_) => true,
            Hit::Update => self.update_ready(),
            Hit::UpdateNow | Hit::UpdateLater | Hit::UpdateSkip => false,
            Hit::Mute => self.is_live() && self.gs.boss.is_some(),
            Hit::RunPrev => self.viewed_page() > 0,
            Hit::RunNext => self.sel_run.is_some(),
            Hit::FightPrev => self.cur_group().is_some_and(|i| i > 0),
            Hit::FightNext => self.sel_group.is_some(),
        }
    }

    pub fn enabled_hit(&self, x: i32, y: i32) -> Option<Hit> {
        self.renderer.hit_test_where(x, y, |h| self.hit_enabled(h))
    }

    pub fn activate(&mut self, hit: Hit) -> bool {
        match hit {
            Hit::Mute | Hit::Sound => {
                self.sound_on = !self.sound_on;
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
                self.topmost = !self.topmost;
                true
            }
            Hit::Discord => {
                self.set_discord(!self.discord_on);
                true
            }
            Hit::Settings => {
                self.settings_open = !self.settings_open;
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
                self.prompt_open = true;
                true
            }
            Hit::UpdateLater => {
                self.prompt_open = false;
                true
            }
            Hit::Close | Hit::UpdateNow | Hit::UpdateSkip | Hit::Info(_) => false,
        }
    }

    pub fn set_discord(&mut self, on: bool) {
        self.discord_on = on;
        self.presence_at = None;
        if on {
            discord::register_scheme();
            self.presence.get_or_insert_with(Presence::new);
        } else if let Some(p) = self.presence.as_mut() {
            p.off();
        }
    }

    fn feed_presence(&mut self, now: u64) {
        if !self.discord_on
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
        let activity = if self.vrc_running {
            discord::activity(&self.gs, now, unix_now)
        } else {
            None
        };
        if let Some(p) = self.presence.as_mut() {
            p.show(activity);
        }
    }

    pub fn set_hover(&mut self, over: Option<Hit>) -> bool {
        if over == self.hover {
            return false;
        }
        self.hover = over;
        self.hover_at = over.map(|_| Instant::now());
        true
    }

    pub fn tip(&self) -> Option<Hit> {
        self.hover.filter(|_| tip_ready(self.hover_at))
    }
}
