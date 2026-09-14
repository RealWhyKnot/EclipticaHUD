use crate::discord::{self, Link, Presence};
use crate::game::state::{base_name, GameState, Run};
use crate::log::{self, Filter};
use crate::logwatch::{self, LogWatch};
use crate::render::{
    tip_ready, Env, Frame, Hit, Info, LogHit, LogView, Renderer, LOGICAL_H, LOGICAL_W, LOG_H,
    LOG_W, MAX_ALPHA, MAX_SCALE, MIN_ALPHA, MIN_SCALE,
};
use crate::update::{self, Badge};
use crate::vr::VrOverlay;
use std::io::BufRead;
use std::path::Path;
use std::time::Instant;
use windows_sys::Win32::Foundation::HWND;

const FLASH_MS: f32 = 3000.0;
const TAKEN_FLASH_MS: f32 = 320.0;
const FADE_MS: f32 = 200.0;
const SLIDE_MS: f32 = 160.0;
const THUMB_HOLD_MS: u128 = 900;
const DEAD_PULSE_MS: f32 = 1100.0;
const WARN_MS: f32 = 5000.0;
const WHEEL_DELTA: i32 = 120;
const ANIM_MS: u32 = 16;
const STALE_SECS: u64 = 120;
const VRC_CHECK_SECS: u64 = 2;
const SCALE_STEP: f32 = 0.1;
const ALPHA_STEP: u8 = 10;
pub const WINDOW_STEPS: [u64; 12] = [3, 4, 5, 6, 7, 8, 9, 10, 15, 20, 25, 30];

pub struct Tick {
    pub redraw: bool,
    pub redraw_log: bool,
    pub timer_ms: Option<u32>,
    pub quit: bool,
}

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

pub struct App {
    pub gs: GameState,
    pub watch: LogWatch,
    pub renderer: Renderer,
    pub log: LogState,
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
    warned_stage: u64,
    last_target_since: u64,
    progress_shown: f32,
    dps_peak: u64,
    dps_boss: Option<String>,
    dps_frac_shown: f32,
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
}

static BLIP: &[u8] = include_bytes!("../assets/target.wav");
static TOKENS: &[u8] = include_bytes!("../assets/tokens.wav");

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
    let logs = logwatch::all_logs(dir);
    let Some((_, old)) = logs.split_last() else {
        return;
    };
    for path in old {
        let Ok(file) = logwatch::open_shared(path) else {
            continue;
        };
        let mut reader = std::io::BufReader::new(file);
        let mut raw = Vec::new();
        loop {
            raw.clear();
            match reader.read_until(b'\n', &mut raw) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let line = String::from_utf8_lossy(&raw);
            gs.feed(line.trim_end_matches(['\r', '\n']));
        }
        gs.log_rotated();
    }
}

fn approach(cur: &mut f32, target: f32, eps: f32) -> bool {
    approach_rate(cur, target, eps, 0.18)
}

fn approach_rate(cur: &mut f32, target: f32, eps: f32, rate: f32) -> bool {
    let d = target - *cur;
    if d.abs() < eps {
        *cur = target;
        false
    } else {
        *cur += d * rate;
        true
    }
}

fn timed(at: Option<Instant>, ms: f32) -> f32 {
    match at {
        Some(t) => (t.elapsed().as_millis() as f32 / ms).min(1.0),
        None => 1.0,
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
            warned_stage: 0,
            last_target_since: 0,
            progress_shown: 0.0,
            dps_peak: 0,
            dps_boss: None,
            dps_frac_shown: 0.0,
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
        }
    }

    pub fn rescale(&mut self) {
        let eff = (self.dpi as f32 * self.scale).round() as u32;
        self.renderer = Renderer::new(eff, LOGICAL_W, LOGICAL_H);
        self.log.renderer = Renderer::new(eff, LOG_W, LOG_H);
        self.rgba.clear();
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.clamp(MIN_SCALE, MAX_SCALE);
        self.rescale();
    }

    pub fn set_alpha(&mut self, alpha: u8) {
        self.alpha = alpha.clamp(MIN_ALPHA, MAX_ALPHA);
    }

    fn step_scale(&mut self, dir: f32) -> bool {
        let next = ((self.scale + dir * SCALE_STEP) * 10.0).round() / 10.0;
        let next = next.clamp(MIN_SCALE, MAX_SCALE);
        if (next - self.scale).abs() < 0.001 {
            return false;
        }
        self.set_scale(next);
        true
    }

    fn step_alpha(&mut self, up: bool) -> bool {
        let next = if up {
            self.alpha.saturating_add(ALPHA_STEP)
        } else {
            self.alpha.saturating_sub(ALPHA_STEP)
        }
        .clamp(MIN_ALPHA, MAX_ALPHA);
        if next == self.alpha {
            return false;
        }
        self.alpha = next;
        true
    }

    pub fn set_window(&mut self, window: u64) {
        self.gs.window = window.clamp(WINDOW_STEPS[0], WINDOW_STEPS[WINDOW_STEPS.len() - 1]);
    }

    fn step_window(&mut self, up: bool) -> bool {
        let cur = self.gs.win();
        let next = if up {
            WINDOW_STEPS.iter().copied().find(|w| *w > cur)
        } else {
            WINDOW_STEPS.iter().rev().copied().find(|w| *w < cur)
        };
        match next {
            Some(w) => {
                self.gs.window = w;
                true
            }
            None => false,
        }
    }

    pub fn alpha_byte(&self) -> u8 {
        (self.alpha as u32 * 255 / 100) as u8
    }

    pub fn update_ready(&self) -> bool {
        matches!(self.badge, Badge::Ready(_))
    }

    pub fn backfill_history(&mut self) {
        if let Some(dir) = logwatch::log_dir() {
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

    fn shown_run(&self) -> Option<(usize, &Run)> {
        if self.sel_run.is_none() && self.env() != Env::InWorld {
            return None;
        }
        self.viewed_run()
    }

    fn log_source(&self) -> Option<log::Source<'_>> {
        Self::source(&self.gs, self.sel_run, self.viewed_page(), self.env())
    }

    fn source(
        gs: &GameState,
        sel: Option<usize>,
        page: usize,
        env: Env,
    ) -> Option<log::Source<'_>> {
        match sel {
            None => log::live(gs).filter(|_| env == Env::InWorld),
            Some(_) => gs.runs.get(page).map(log::of_run),
        }
    }

    fn log_reset(&mut self) {
        self.log.scroll = 0.0;
        self.log.scroll_shown = 0.0;
        self.log.rows = self.log_rows();
    }

    pub fn group_pages(&self) -> usize {
        let n = self.viewed_run().map_or(0, |(_, r)| r.groups().len());
        n + usize::from(self.sel_run.is_none() && self.gs.boss.is_none())
    }

    fn live_group_index(&self) -> Option<usize> {
        self.group_pages().checked_sub(1)
    }

    fn cur_group(&self) -> Option<usize> {
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

    pub fn hit_enabled(&self, hit: Hit) -> bool {
        match hit {
            Hit::Close | Hit::Log | Hit::Pin | Hit::Discord | Hit::Settings => true,
            Hit::ScaleDown => self.settings_open && self.scale > MIN_SCALE + 0.001,
            Hit::ScaleUp => self.settings_open && self.scale < MAX_SCALE - 0.001,
            Hit::AlphaDown => self.settings_open && self.alpha > MIN_ALPHA,
            Hit::AlphaUp => self.settings_open && self.alpha < MAX_ALPHA,
            Hit::WindowDown => self.settings_open && self.gs.win() > WINDOW_STEPS[0],
            Hit::WindowUp => {
                self.settings_open && self.gs.win() < WINDOW_STEPS[WINDOW_STEPS.len() - 1]
            }
            Hit::Target | Hit::FightPrev | Hit::FightNext if self.settings_open => false,
            Hit::Info(
                Info::Boss | Info::Result | Info::Dealt(_) | Info::DealtBar | Info::LastKill,
            ) if self.settings_open => false,
            Hit::Info(Info::Version) => !self.update_ready(),
            Hit::Info(i) if i.live_only() => self.is_live(),
            Hit::Info(i) if i.history_only() => !self.is_live(),
            Hit::Info(_) => true,
            Hit::Update => self.update_ready(),
            Hit::Target => self.is_live() && self.gs.boss.is_some() && self.gs.target.is_some(),
            Hit::RunPrev => self.viewed_page() > 0,
            Hit::RunNext => self.sel_run.is_some(),
            Hit::FightPrev => self.cur_group().is_some_and(|i| i > 0),
            Hit::FightNext => self.sel_group.is_some(),
        }
    }

    pub fn enabled_hit(&self, x: i32, y: i32) -> Option<Hit> {
        self.renderer.hit_test_where(x, y, |h| self.hit_enabled(h))
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

    pub fn activate(&mut self, hit: Hit) -> bool {
        match hit {
            Hit::Target => {
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
            Hit::ScaleDown => self.step_scale(-1.0),
            Hit::ScaleUp => self.step_scale(1.0),
            Hit::AlphaDown => self.step_alpha(false),
            Hit::AlphaUp => self.step_alpha(true),
            Hit::WindowDown => self.step_window(false),
            Hit::WindowUp => self.step_window(true),
            Hit::Close | Hit::Update | Hit::Info(_) => false,
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

    pub fn log_toggle(&mut self) {
        if self.log.visible {
            self.log_close();
        } else {
            self.log_open();
        }
    }

    pub fn log_open(&mut self) {
        self.log.visible = true;
        self.log.alpha = 0;
        self.log.alpha_shown = 0;
        self.log.opened_at = Some(Instant::now());
    }

    pub fn log_close(&mut self) {
        self.log.visible = false;
        self.log.drag = None;
        self.log.hover = None;
    }

    pub fn log_rows(&self) -> usize {
        log::timeline(self.log_source(), self.log.filter).len()
    }

    pub fn log_thumb(&self) -> Option<(i32, i32)> {
        log::thumb(
            self.log.rows,
            self.log.renderer.body_h(),
            self.log.scroll_shown,
        )
    }

    pub fn log_set_filter(&mut self, filter: Filter) {
        if self.log.filter != filter {
            self.log.filter = filter;
            self.log_reset();
        }
    }

    fn log_clamp(&mut self) {
        let max = log::max_scroll(self.log.rows, self.log.renderer.body_h());
        self.log.scroll = self.log.scroll.clamp(0.0, max);
        self.log.scroll_shown = self.log.scroll_shown.clamp(0.0, max);
    }

    pub fn log_wheel(&mut self, delta: i32) {
        self.log.wheel_acc += delta;
        let steps = self.log.wheel_acc / WHEEL_DELTA;
        self.log.wheel_acc -= steps * WHEEL_DELTA;
        if steps != 0 {
            self.log.scroll -= steps as f32 * 3.0 * log::ROW_H as f32;
            self.log_clamp();
            self.log.thumb_seen = Some(Instant::now());
        }
    }

    pub fn log_press(&mut self, hit: LogHit, y: i32) {
        match hit {
            LogHit::Title | LogHit::Count => {}
            LogHit::Close => self.log_close(),
            LogHit::Tab(f) => self.log_set_filter(f),
            LogHit::Thumb => self.log.drag = Some((y, self.log.scroll)),
            LogHit::Track => {
                let page = self.log.renderer.body_h() as f32;
                let above = self
                    .log_thumb()
                    .is_some_and(|(ty, _)| self.log.renderer.unscale(y) < LOG_BODY_Y + ty);
                self.log.scroll += if above { -page } else { page };
                self.log_clamp();
                self.log.thumb_seen = Some(Instant::now());
            }
        }
    }

    pub fn log_drag_to(&mut self, y: i32) {
        if let Some((y0, s0)) = self.log.drag {
            let dy = self.log.renderer.unscale(y - y0);
            self.log.scroll = log::drag_scroll(self.log.rows, self.log.renderer.body_h(), s0, dy);
            self.log.scroll_shown = self.log.scroll;
            self.log.thumb_seen = Some(Instant::now());
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

    pub fn log_set_hover(&mut self, over: Option<LogHit>) -> bool {
        if over == self.log.hover {
            return false;
        }
        self.log.hover = over;
        self.log.hover_at = over.map(|_| Instant::now());
        true
    }

    pub fn nudge_timer(&mut self) {
        self.timer_ms = ANIM_MS;
    }

    pub fn tip(&self) -> Option<Hit> {
        self.hover.filter(|_| tip_ready(self.hover_at))
    }

    pub fn log_tip(&self) -> Option<LogHit> {
        self.log.hover.filter(|_| tip_ready(self.log.hover_at))
    }

    pub fn log_release(&mut self) {
        self.log.drag = None;
    }

    fn now(&self) -> u64 {
        let elapsed = self.last_ts_at.elapsed().as_secs();
        if self.env() == Env::InWorld && elapsed <= STALE_SECS {
            self.last_ts + elapsed
        } else {
            self.last_ts
        }
    }

    fn dead_pulse(&self) -> f32 {
        match self.dead_since {
            Some(t) => {
                let ph = t.elapsed().as_millis() as f32 / DEAD_PULSE_MS * std::f32::consts::TAU;
                ph.sin() * 0.5 + 0.5
            }
            None => 0.0,
        }
    }

    pub fn render(&mut self) {
        let env = self.env();
        let frame = Frame {
            now: self.now(),
            env,
            pages: self.pages(),
            view_page: self.viewed_page(),
            settings_open: self.settings_open,
            scale: self.scale,
            alpha: self.alpha,
            window: self.gs.win(),
            flash_t: timed(self.flash_at, FLASH_MS),
            taken_flash_t: timed(self.taken_flash_at, TAKEN_FLASH_MS),
            warn_t: timed(self.warn_at, WARN_MS),
            dead_pulse: self.dead_pulse(),
            hover: self.hover,
            pressed: self.pressed,
            tip: self.tip(),
            vr: self.vr.status(),
            log_ok: env != Env::NoVrchat && self.watch.path.is_some(),
            log_open: self.log.visible,
            progress_shown: self.progress_shown,
            dps_frac_shown: self.dps_frac_shown,
            taken_frac_shown: self.taken_frac_shown,
            dps_shown: self.dps_shown,
            fight_dps_shown: self.fight_dps_shown,
            taken_shown: self.taken_shown,
            taken_rate_shown: self.taken_rate_shown,
            update: self.badge.clone(),
            sound_on: self.sound_on,
            topmost: self.topmost,
            discord: self.link,
            discord_on: self.discord_on,
            view_run: self.shown_run().map(|(i, _)| i),
            view_group: self.viewed_group().map(|(i, _)| i),
            group_pages: self.group_pages(),
            run_sel: self.sel_run.is_some(),
            group_sel: self.sel_group.is_some(),
        };
        self.renderer.draw_main(&mut self.gs, &frame);
        self.renderer.rgba(&mut self.rgba);
        self.vr.submit(
            &self.rgba,
            self.renderer.width as u32,
            self.renderer.height as u32,
            true,
            env == Env::InWorld,
            self.alpha as f32 / 100.0,
            self.scale,
        );
    }

    pub fn render_log(&mut self) {
        let slide_t = timed(self.log.slide_at, SLIDE_MS);
        let view = LogView {
            scroll: self.log.scroll_shown,
            filter: self.log.filter,
            hover: self.log.hover,
            tip: self.log_tip(),
            thumb: self.log_thumb(),
            dragging: self.log.drag.is_some(),
            thumb_t: self.log.thumb_t,
            slide: -(1.0 - crate::render::ease_out_cubic(slide_t)) * log::ROW_H as f32,
        };
        let (sel, page, env) = (self.sel_run, self.viewed_page(), self.env());
        let src = Self::source(&self.gs, sel, page, env);
        self.log.renderer.draw_log(src, &view);
    }

    pub fn tick(&mut self) -> Tick {
        let gs = &mut self.gs;
        let mut newest = 0u64;
        let rotated = self.watch.poll(|line| {
            if let Some(ts) = gs.feed(line) {
                newest = newest.max(ts);
            }
        });
        if rotated {
            self.gs.log_rotated();
        }
        if newest > self.last_ts {
            self.last_ts = newest;
            self.last_ts_at = Instant::now();
        }
        if cfg!(not(test))
            && self
                .vrc_checked
                .is_none_or(|t| t.elapsed().as_secs() >= VRC_CHECK_SECS)
        {
            self.vrc_checked = Some(Instant::now());
            self.vrc_running = crate::vr::process_running("VRChat.exe");
        }
        let env = self.env();
        let env_changed = self.last_env.is_some_and(|e| e != env);
        self.last_env = Some(env);
        let now = self.now();
        if self.gs.target_since != self.last_target_since {
            self.last_target_since = self.gs.target_since;
            if self.gs.target.is_some() {
                self.flash_at = Some(Instant::now());
                if self.sound_on {
                    play(BLIP);
                }
            }
        }
        let stage_key = self.gs.stage_stats.start_ts;
        if self.warned_stage != stage_key
            && self.gs.wave_idle(now)
            && self.gs.tokens_missing().is_some()
        {
            self.warned_stage = stage_key;
            self.warn_at = Some(Instant::now());
            if self.sound_on {
                play(TOKENS);
            }
        }
        let taken_seq = self.gs.taken.back().map_or(0, |e| e.seq);
        if taken_seq != self.last_taken_seq {
            self.last_taken_seq = taken_seq;
            if self.primed && taken_seq != 0 {
                self.taken_flash_at = Some(Instant::now());
            }
        }
        let dps_base = self.gs.boss.as_deref().map(base_name);
        if dps_base != self.dps_boss.as_deref() {
            self.dps_boss = dps_base.map(str::to_string);
            self.dps_peak = 0;
            self.taken_peak = 0;
        }
        let rolling = self.gs.rolling_dps(now);
        self.dps_peak = self.dps_peak.max(rolling);
        let dps_target = if self.gs.boss.is_some() && self.dps_peak > 0 {
            rolling as f32 / self.dps_peak as f32
        } else {
            0.0
        };
        let rolling_taken = self.gs.rolling_taken(now);
        self.taken_peak = self.taken_peak.max(rolling_taken);
        let taken_target = if self.gs.boss.is_some() && self.taken_peak > 0 {
            rolling_taken as f32 / self.taken_peak as f32
        } else {
            0.0
        };
        self.feed_presence(now);
        let link = discord::link();
        let link_changed = link != self.link;
        self.link = link;
        let badge = update::badge();
        let badge_changed = badge != self.badge;
        if badge_changed {
            self.badge = badge;
        }
        let tip_pending = self.hover.is_some() && self.tip().is_none();
        let log_tip_pending = self.log.hover.is_some() && self.log_tip().is_none();
        let mut anim = approach(&mut self.progress_shown, self.gs.progress, 0.002);
        anim |= tip_pending;
        anim |= approach(&mut self.dps_frac_shown, dps_target, 0.002);
        anim |= approach(&mut self.taken_frac_shown, taken_target, 0.002);
        anim |= approach(&mut self.dps_shown, rolling as f32, 0.5);
        anim |= approach(
            &mut self.fight_dps_shown,
            self.gs.fight_dps(now) as f32,
            0.5,
        );
        anim |= approach(&mut self.taken_shown, rolling_taken as f32, 0.5);
        anim |= approach(
            &mut self.taken_rate_shown,
            self.gs.fight_taken_rate(now) as f32,
            0.5,
        );
        anim |= self.flash_at.is_some() && timed(self.flash_at, FLASH_MS) < 1.0;
        anim |= self.taken_flash_at.is_some() && timed(self.taken_flash_at, TAKEN_FLASH_MS) < 1.0;
        anim |= self.warn_at.is_some() && timed(self.warn_at, WARN_MS) < 1.0;
        let dead = env == Env::InWorld && self.gs.is_dead(now);
        if dead && self.dead_since.is_none() {
            self.dead_since = Some(Instant::now());
        } else if !dead {
            self.dead_since = None;
        }
        anim |= dead;
        let tip = self.tip();
        let tip_changed = tip != self.last_tip;
        self.last_tip = tip;
        let redraw = tip_changed
            || self.gs.changed
            || anim
            || (env == Env::InWorld && self.gs.boss.is_some())
            || badge_changed
            || link_changed
            || env_changed
            || dead != self.was_dead;
        self.was_dead = dead;

        let changed = self.gs.changed;
        self.gs.changed = false;
        let mut log_anim = false;
        let mut redraw_log = false;
        if self.log.visible {
            let rows = self.log_rows();
            if rows != self.log.rows {
                if rows > self.log.rows && self.primed && self.log.scroll_shown < 1.0 {
                    self.log.slide_at = Some(Instant::now());
                }
                self.log.rows = rows;
                self.log_clamp();
            }
            log_anim |= approach_rate(&mut self.log.scroll_shown, self.log.scroll, 0.5, 0.4);
            let thumb_lit = self.log.drag.is_some()
                || matches!(self.log.hover, Some(LogHit::Thumb | LogHit::Track))
                || self
                    .log
                    .thumb_seen
                    .is_some_and(|t| t.elapsed().as_millis() < THUMB_HOLD_MS);
            log_anim |= approach(
                &mut self.log.thumb_t,
                if thumb_lit { 1.0 } else { 0.0 },
                0.01,
            );
            log_anim |= self.log.slide_at.is_some() && timed(self.log.slide_at, SLIDE_MS) < 1.0;
            let fade = timed(self.log.opened_at, FADE_MS);
            self.log.alpha = (crate::render::ease_out_cubic(fade) * self.alpha_byte() as f32) as u8;
            log_anim |= fade < 1.0;
            log_anim |= log_tip_pending;
            let log_tip = self.log_tip();
            let log_tip_changed = log_tip != self.last_log_tip;
            self.last_log_tip = log_tip;
            redraw_log = changed || log_anim || rotated || log_tip_changed || env_changed;
            if redraw_log {
                self.render_log();
            }
        }
        self.primed = true;
        if redraw {
            self.render();
        } else {
            self.vr.submit(
                &self.rgba,
                self.renderer.width as u32,
                self.renderer.height as u32,
                false,
                env == Env::InWorld,
                self.alpha as f32 / 100.0,
                self.scale,
            );
        }
        let want: u32 = if anim || log_anim { ANIM_MS } else { 1000 };
        let timer_ms = (want != self.timer_ms).then_some(want);
        self.timer_ms = want;
        Tick {
            redraw,
            redraw_log,
            timer_ms,
            quit: update::restart_pending(),
        }
    }
}

const LOG_BODY_Y: i32 = crate::render::LOG_BODY.1;

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = "2026.09.07 09:12:28 Debug      -  ";
    const ENTER: &str = "[Behaviour] Entering Room: Ecliptica - Demo Playtest";

    fn headless() -> App {
        let mut app = App::new(96);
        app.watch = LogWatch::default();
        app.gs.feed(&format!("{P}{ENTER}"));
        app.gs.changed = false;
        app
    }

    #[test]
    fn flash_on_target_change() {
        let mut app = headless();
        assert!(app.sound_on);
        let t = app.tick();
        assert!(!t.redraw);
        assert_eq!(t.timer_ms, None);
        app.gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1"
        ));
        app.gs
            .feed(&format!("{P}ownership of Kakarot transferred to Alice"));
        let t = app.tick();
        assert!(t.redraw);
        assert_eq!(t.timer_ms, Some(ANIM_MS));
        assert!(app.flash_at.is_some());
        app.gs
            .feed(&format!("{P}Boss Kakarot dead, personal damage dealt: "));
        app.tick();
        assert_eq!(app.dps_boss, None);
    }

    const STAGE_A: &str =
        "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Blade";
    const STAGE_B: &str =
        "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Twinmage";

    #[test]
    fn settings_steps_and_clamps() {
        let mut app = headless();
        let base = app.renderer.width;
        assert!(!app.hit_enabled(Hit::ScaleUp));
        assert!(app.hit_enabled(Hit::Target) || app.gs.boss.is_none());
        assert!(app.activate(Hit::Settings));
        assert!(app.settings_open);
        assert!(!app.hit_enabled(Hit::Target));
        assert!(!app.hit_enabled(Hit::Info(Info::Boss)));
        assert!(!app.hit_enabled(Hit::AlphaUp));
        assert!(app.hit_enabled(Hit::AlphaDown));
        assert!(app.activate(Hit::ScaleUp));
        assert!((app.scale - 1.1).abs() < 0.001);
        assert!(app.renderer.width > base);
        assert!(app.log.renderer.width > LOG_W);
        for _ in 0..20 {
            app.activate(Hit::ScaleUp);
        }
        assert_eq!(app.scale, MAX_SCALE);
        assert!(!app.hit_enabled(Hit::ScaleUp));
        assert!(!app.activate(Hit::ScaleUp));
        assert_eq!(app.renderer.width, base * 2);
        for _ in 0..20 {
            app.activate(Hit::ScaleDown);
        }
        assert_eq!(app.scale, MIN_SCALE);
        assert_eq!(app.renderer.width, base / 2);
        assert!(!app.hit_enabled(Hit::ScaleDown));
        for _ in 0..10 {
            app.activate(Hit::AlphaDown);
        }
        assert_eq!(app.alpha, MIN_ALPHA);
        assert!(!app.activate(Hit::AlphaDown));
        assert!(app.activate(Hit::AlphaUp));
        assert_eq!(app.alpha, MIN_ALPHA + ALPHA_STEP);
        assert_eq!(app.alpha_byte(), 102);
        assert_eq!(app.gs.win(), 10);
        assert!(app.hit_enabled(Hit::WindowUp) && app.hit_enabled(Hit::WindowDown));
        assert!(app.activate(Hit::WindowUp));
        assert_eq!(app.gs.win(), 15);
        for _ in 0..10 {
            app.activate(Hit::WindowUp);
        }
        assert_eq!(app.gs.win(), 30);
        assert!(!app.hit_enabled(Hit::WindowUp));
        assert!(!app.activate(Hit::WindowUp));
        for _ in 0..20 {
            app.activate(Hit::WindowDown);
        }
        assert_eq!(app.gs.win(), 3);
        assert!(!app.activate(Hit::WindowDown));
        app.set_window(12);
        assert_eq!(app.gs.win(), 12);
        assert!(app.activate(Hit::WindowDown));
        assert_eq!(app.gs.win(), 10);
        app.set_window(99);
        assert_eq!(app.gs.win(), 30);
        assert!(!app.hit_enabled(Hit::Info(Info::LastKill)));
        app.set_scale(9.0);
        assert_eq!(app.scale, MAX_SCALE);
        app.set_alpha(0);
        assert_eq!(app.alpha, MIN_ALPHA);
        assert!(app.activate(Hit::Settings));
        assert!(!app.settings_open);
        assert!(!app.hit_enabled(Hit::ScaleDown));
    }

    #[test]
    fn empty_state_has_one_page() {
        let mut app = headless();
        assert_eq!(app.pages(), 1);
        assert_eq!(app.viewed_page(), 0);
        assert!(app.viewed_run().is_none());
        assert!(!app.hit_enabled(Hit::RunPrev));
        assert!(!app.run_prev());
        assert!(app.log_rows() == 0);
        app.gs.feed(&format!("{P}{STAGE_A}"));
        app.gs.feed(&format!("{P}ECLIPTICA - now in lobby"));
        app.gs.feed(&format!("{P}{STAGE_B}"));
        assert_eq!(app.pages(), 2);
        assert!(app.run_prev());
        app.gs.feed(&format!("{P}ECLIPTICA - now in lobby"));
        assert_eq!(app.sel_run, Some(0));
        assert_eq!(app.pages(), 3);
        assert!(app.run_next());
        assert_eq!(app.sel_run, Some(1));
        assert!(app.run_next());
        assert_eq!(app.sel_run, None);
    }

    #[test]
    fn env_and_clock() {
        let mut app = headless();
        assert_eq!(app.env(), Env::InWorld);
        app.gs.feed(&format!("{P}{STAGE_A}"));
        app.tick();
        let ts = app.last_ts;
        assert_eq!(app.now(), ts);
        app.last_ts_at = Instant::now() - std::time::Duration::from_secs(STALE_SECS + 5);
        assert_eq!(app.now(), ts);
        app.last_ts_at = Instant::now() - std::time::Duration::from_secs(10);
        assert_eq!(app.now(), ts + 10);
        app.gs
            .feed(&format!("{P}[Behaviour] Entering Room: Sky Dream"));
        assert_eq!(app.env(), Env::NotInWorld);
        assert_eq!(app.now(), ts);
        assert!(app.gs.runs[0].end_ts.is_some());
        assert_eq!(app.pages(), 2);
        assert!(app.log_rows() == 0);
        app.vrc_running = false;
        assert_eq!(app.env(), Env::NoVrchat);
        let t = app.tick();
        assert!(t.redraw);
        assert!(!app.tick().redraw);
    }

    #[test]
    fn backfill_reads_all_but_newest() {
        let dir = std::env::temp_dir().join(format!("ehud_backfill_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log_a = dir.join("output_log_2026-01-01_00-00-00.txt");
        std::fs::write(
            &log_a,
            format!("2026.01.01 10:00:00 Debug      -  {STAGE_A}\r\n"),
        )
        .unwrap();
        let log_b = dir.join("output_log_2026-01-02_00-00-00.txt");
        std::fs::write(
            &log_b,
            format!("2026.01.02 10:00:00 Debug      -  {STAGE_B}\n"),
        )
        .unwrap();
        let future = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        std::fs::File::options()
            .write(true)
            .open(&log_b)
            .unwrap()
            .set_modified(future)
            .unwrap();
        let mut gs = GameState::default();
        backfill(&mut gs, &dir);
        assert_eq!(gs.runs.len(), 1);
        assert!(gs.runs[0].end_ts.is_some());
        assert_eq!(gs.runs[0].class, "Blade");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rotation_in_tick_closes_run() {
        let dir = std::env::temp_dir().join(format!("ehud_rotate_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log_a = dir.join("output_log_2026-01-01_00-00-00.txt");
        std::fs::write(
            &log_a,
            format!("2026.01.01 10:00:00 Debug      -  {STAGE_A}\n"),
        )
        .unwrap();
        let mut app = App::new(96);
        app.watch = LogWatch::for_dir(dir.clone());
        app.tick();
        assert_eq!(app.gs.runs.len(), 1);
        assert!(app.gs.runs[0].end_ts.is_none());
        let log_b = dir.join("output_log_2026-01-02_00-00-00.txt");
        std::fs::write(
            &log_b,
            format!("2026.01.02 10:00:00 Debug      -  {STAGE_B}\n"),
        )
        .unwrap();
        let future = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        std::fs::File::options()
            .write(true)
            .open(&log_b)
            .unwrap()
            .set_modified(future)
            .unwrap();
        app.tick();
        assert_eq!(app.gs.runs.len(), 1);
        assert!(app.gs.runs[0].end_ts.is_some());
        app.tick();
        assert_eq!(app.gs.runs.len(), 2);
        assert_eq!(app.gs.runs[1].class, "Twinmage");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn navigation() {
        let mut app = headless();
        app.gs.feed(&format!("{P}{STAGE_A}"));
        app.gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0"
        ));
        app.gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: YukiPhase2(Clone) on phase: 0"
        ));
        app.gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0"
        ));
        app.gs.feed(&format!("{P}ECLIPTICA - now in lobby"));
        assert_eq!(app.pages(), 2);
        assert_eq!(app.viewed_page(), 1);
        assert!(app.viewed_run().is_none());
        assert!(app.hit_enabled(Hit::RunPrev));
        assert!(!app.hit_enabled(Hit::RunNext));
        assert!(app.run_prev());
        assert_eq!(app.sel_run, Some(0));
        assert!(app
            .viewed_run()
            .is_some_and(|(i, r)| i == 0 && r.fights.len() == 3));
        assert!(app.run_next());
        assert!(app.is_live() && app.sel_run.is_none());
        assert!(app.viewed_run().is_none());
        app.gs.feed(&format!("{P}{STAGE_B}"));
        assert_eq!(app.gs.runs.len(), 2);
        assert_eq!(app.pages(), 2);
        assert!(app.viewed_run().is_some_and(|(i, _)| i == 1));
        assert!(app.is_live());
        assert!(!app.run_next());
        assert!(!app.group_next());
        assert!(!app.hit_enabled(Hit::RunNext));
        assert!(app.hit_enabled(Hit::RunPrev));
        assert!(app.run_prev());
        assert_eq!(app.sel_run, Some(0));
        let (gi, g) = app.viewed_group().unwrap();
        assert_eq!((gi, g.len()), (1, 1));
        assert!(app.group_prev());
        let (gi, g) = app.viewed_group().unwrap();
        assert_eq!((gi, g.len()), (0, 2));
        assert!(!app.group_prev());
        assert!(app.group_next());
        assert_eq!(app.sel_group, None);
        assert!(app.run_next());
        assert!(app.is_live());
        assert!(!app.hit_enabled(Hit::Target));
        assert!(app.hit_enabled(Hit::Log));
        assert!(app.topmost);
        assert!(app.activate(Hit::Pin));
        assert!(!app.topmost);
        assert!(!app.discord_on);
        assert!(app.hit_enabled(Hit::Discord));
        assert!(app.activate(Hit::Discord));
        assert!(app.discord_on && app.presence.is_some());
        assert!(app.activate(Hit::Discord));
        assert!(!app.discord_on);
        assert!(app.hit_enabled(Hit::Info(Info::LastHit)));
        assert!(!app.hit_enabled(Hit::Info(Info::Result)));
        assert!(app.run_prev());
        assert!(!app.hit_enabled(Hit::Info(Info::LastHit)));
        assert!(app.hit_enabled(Hit::Info(Info::Result)));
    }

    #[test]
    fn killed_boss_moves_to_previous_page() {
        let mut app = headless();
        app.gs.feed(&format!("{P}{STAGE_A}"));
        assert_eq!(app.group_pages(), 1);
        assert!(app.viewed_group().is_none());
        assert!(!app.hit_enabled(Hit::FightPrev));
        app.gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: Despair(Clone) on phase: 0"
        ));
        app.gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: DespairPhase2(Clone) on phase: 0"
        ));
        assert_eq!(app.group_pages(), 1);
        assert!(app
            .viewed_group()
            .is_some_and(|(i, g)| i == 0 && g.len() == 2));
        assert!(!app.hit_enabled(Hit::FightPrev));
        app.gs.feed(&format!(
            "{P}Boss DespairPhase2 dead, personal damage dealt: "
        ));
        app.gs.feed(&format!("{P}STRIKE DMG: 100"));
        app.gs.feed(&format!("{P}NON-STRIKE DMG: 0"));
        app.gs.feed(&format!("{P}ECLIPTICA - now in intermission"));
        app.gs.feed(&format!("{P}{STAGE_B}"));
        assert_eq!(app.group_pages(), 2);
        assert!(app.is_live());
        assert!(app.viewed_group().is_none());
        assert!(app.hit_enabled(Hit::FightPrev));
        assert!(app.group_prev());
        assert!(app
            .viewed_group()
            .is_some_and(|(i, g)| i == 0 && g.len() == 2));
        assert!(!app.group_prev());
        assert!(app.group_next());
        assert!(app.is_live());
        app.gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0"
        ));
        assert_eq!(app.group_pages(), 2);
        assert!(app.viewed_group().is_some_and(|(i, _)| i == 1));
        assert!(app.group_prev());
        assert!(app.viewed_group().is_some_and(|(i, _)| i == 0));
        assert!(app.group_next());
        assert!(app.is_live());
    }

    #[test]
    fn token_warning_once_per_quiet_stage() {
        let mut app = headless();
        for _ in 0..3 {
            app.gs.feed(&format!("{P}spawn token, False, 0"));
        }
        let t = app.gs.feed(&format!("{P}{STAGE_A}")).unwrap();
        app.gs.feed(&format!("{P}ECLIPTICA saving SESSION ID 2505"));
        app.last_ts = t + 10;
        app.tick();
        assert!(app.warn_at.is_none());
        app.last_ts = t + 20;
        let tick = app.tick();
        assert!(app.warn_at.is_some());
        assert_eq!(tick.timer_ms, Some(ANIM_MS));
        app.warn_at = None;
        app.last_ts = t + 40;
        app.tick();
        assert!(app.warn_at.is_none());
        const P2: &str = "2026.09.07 09:20:00 Debug      -  ";
        app.gs.feed(&format!("{P2}ECLIPTICA - now in intermission"));
        for _ in 0..3 {
            app.gs.feed(&format!("{P2}spawn token, False, 0"));
        }
        let t = app.gs.feed(&format!("{P2}{STAGE_B}")).unwrap();
        app.last_ts = t + 20;
        app.tick();
        assert!(app.warn_at.is_some());
        app.warn_at = None;
        for _ in 0..3 {
            app.gs
                .feed(&format!("{P2}ECLIPTICA saving SESSION ID 2505"));
        }
        app.warned_stage = 0;
        app.last_ts = t + 60;
        app.tick();
        assert!(app.warn_at.is_none());
    }

    #[test]
    fn dps_peak_survives_phase_change() {
        let mut app = headless();
        app.gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0"
        ));
        app.gs.feed(&format!("{P}Dealing 100 STRIKE damage"));
        app.gs
            .feed(&format!("{P}damage has been taken: 40, from source: "));
        app.tick();
        let peak = app.dps_peak;
        assert!(peak > 0);
        assert_eq!(app.taken_peak, 40);
        assert_eq!(app.dps_boss.as_deref(), Some("Yuki"));
        app.gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: YukiPhase2(Clone) on phase: 0"
        ));
        app.tick();
        assert_eq!(app.dps_peak, peak);
        assert_eq!(app.taken_peak, 40);
        assert_eq!(app.dps_boss.as_deref(), Some("Yuki"));
        app.gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0"
        ));
        app.tick();
        assert_eq!(app.dps_boss.as_deref(), Some("Kakarot"));
    }

    #[test]
    fn timer_settles_after_animation() {
        let mut app = headless();
        app.gs.feed(&format!(
            "{P}ECLIPTICA - now in stage: Stage_Test on phase: 0.5 as class: Blade"
        ));
        let t = app.tick();
        assert!(t.redraw);
        assert_eq!(t.timer_ms, Some(ANIM_MS));
        for _ in 0..200 {
            if app.tick().timer_ms == Some(1000) {
                assert_eq!(app.progress_shown, 0.5);
                return;
            }
        }
        panic!("animation never settled");
    }

    #[test]
    fn tooltip_after_hover_delay() {
        let mut app = headless();
        app.tick();
        assert!(app.set_hover(Some(Hit::Log)));
        assert!(!app.set_hover(Some(Hit::Log)));
        assert_eq!(app.tip(), None);
        let t = app.tick();
        assert_eq!(t.timer_ms, Some(ANIM_MS));
        std::thread::sleep(std::time::Duration::from_millis(500));
        let t = app.tick();
        assert_eq!(app.tip(), Some(Hit::Log));
        assert!(t.redraw);
        assert_eq!(t.timer_ms, Some(1000));
        assert!(!app.tick().redraw);
        assert!(app.set_hover(None));
        assert_eq!(app.tip(), None);
        app.log_open();
        assert!(app.log_set_hover(Some(LogHit::Close)));
        assert_eq!(app.log_tip(), None);
        app.tick();
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert!(app.tick().redraw_log);
        assert_eq!(app.log_tip(), Some(LogHit::Close));
        assert!(!app.tick().redraw_log);
    }

    #[test]
    fn taken_pulse_only_after_first_tick() {
        let mut app = headless();
        app.gs
            .feed(&format!("{P}damage has been taken: 5, from source: "));
        app.tick();
        assert!(app.taken_flash_at.is_none());
        app.gs
            .feed(&format!("{P}damage has been taken: 5, from source: "));
        let t = app.tick();
        assert!(app.taken_flash_at.is_some());
        assert!(t.timer_ms.is_none_or(|ms| ms == ANIM_MS));
        assert_eq!(app.timer_ms, ANIM_MS);
        assert!(app.taken_shown > 0.0);
    }

    #[test]
    fn log_scroll_filter_and_fade() {
        let mut app = headless();
        for i in 0..60 {
            app.gs.feed(&format!(
                "2026.09.07 09:12:{:02} Debug      -  damage has been taken: 1, from source: ",
                i % 60
            ));
        }
        app.gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0"
        ));
        app.gs
            .feed(&format!("{P}ownership of Yuki transferred to Alice"));
        let t = app.tick();
        assert!(!t.redraw_log);
        assert!(app.activate(Hit::Log));
        assert!(app.log.visible);
        assert_eq!(app.log.alpha, 0);
        let t = app.tick();
        assert!(t.redraw_log);
        assert_eq!(app.log.rows, 62);
        let max = log::max_scroll(62, app.log.renderer.body_h());
        app.log_wheel(-WHEEL_DELTA * 100);
        assert_eq!(app.log.scroll, max);
        app.log_wheel(WHEEL_DELTA / 2);
        assert_eq!(app.log.scroll, max);
        app.log_wheel(WHEEL_DELTA / 2);
        assert_eq!(app.log.scroll, max - 3.0 * log::ROW_H as f32);
        app.log_press(LogHit::Tab(Filter::Targets), 0);
        assert_eq!(app.log.scroll, 0.0);
        assert_eq!(app.log.rows, 2);
        app.log_press(LogHit::Tab(Filter::All), 0);
        app.log_wheel(-WHEEL_DELTA * 100);
        app.log.scroll_shown = app.log.scroll;
        app.log_press(LogHit::Thumb, 100);
        app.log_drag_to(-5000);
        assert_eq!(app.log.scroll, 0.0);
        app.log_release();
        assert!(app.log.drag.is_none());
        app.gs.log_rotated();
        app.tick();
        assert_eq!(app.log.rows, 0);
        assert_eq!(app.log.scroll, 0.0);
        std::thread::sleep(std::time::Duration::from_millis(220));
        app.tick();
        assert_eq!(app.log.alpha, 255);
        app.log_press(LogHit::Close, 0);
        assert!(!app.log.visible);
    }
}
