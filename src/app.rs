use crate::logwatch::LogWatch;
use crate::render::{Frame, Renderer};
use crate::state::GameState;
use crate::vr::VrOverlay;
use std::time::Instant;

const FLASH_MS: f32 = 3000.0;

pub struct Tick {
    pub redraw: bool,
    pub timer_ms: Option<u32>,
}

pub struct App {
    pub gs: GameState,
    pub watch: LogWatch,
    pub renderer: Renderer,
    vr: VrOverlay,
    rgba: Vec<u8>,
    last_ts: u64,
    last_ts_at: Instant,
    pub hover_close: bool,
    pub pressed_close: bool,
    pub tracking: bool,
    flash_at: Option<Instant>,
    last_target_since: u64,
    progress_shown: f32,
    dps_peak: u64,
    dps_boss: Option<String>,
    dps_frac_shown: f32,
    timer_ms: u32,
}

fn approach(cur: &mut f32, target: f32) -> bool {
    let d = target - *cur;
    if d.abs() < 0.002 {
        *cur = target;
        false
    } else {
        *cur += d * 0.18;
        true
    }
}

impl App {
    pub fn new(renderer: Renderer) -> Self {
        App {
            gs: GameState::default(),
            watch: LogWatch::new(),
            renderer,
            vr: VrOverlay::new(),
            rgba: Vec::new(),
            last_ts: 0,
            last_ts_at: Instant::now(),
            hover_close: false,
            pressed_close: false,
            tracking: false,
            flash_at: None,
            last_target_since: 0,
            progress_shown: 0.0,
            dps_peak: 0,
            dps_boss: None,
            dps_frac_shown: 0.0,
            timer_ms: 1000,
        }
    }

    fn now(&self) -> u64 {
        self.last_ts + self.last_ts_at.elapsed().as_secs()
    }

    fn flash_t(&self) -> f32 {
        match self.flash_at {
            Some(t) => (t.elapsed().as_millis() as f32 / FLASH_MS).min(1.0),
            None => 1.0,
        }
    }

    pub fn render(&mut self) {
        let frame = Frame {
            now: self.now(),
            flash_t: self.flash_t(),
            hover_close: self.hover_close,
            pressed_close: self.pressed_close,
            vr: self.vr.status(),
            log_ok: self.watch.path.is_some(),
            progress_shown: self.progress_shown,
            dps_frac_shown: self.dps_frac_shown,
        };
        self.renderer.draw(&mut self.gs, &frame);
        self.renderer.rgba(&mut self.rgba);
        self.vr.submit(&self.rgba, self.renderer.width as u32, self.renderer.height as u32, true);
    }

    pub fn tick(&mut self) -> Tick {
        let gs = &mut self.gs;
        let mut newest = 0u64;
        self.watch.poll(|line| {
            if let Some(ts) = gs.feed(line) {
                newest = newest.max(ts);
            }
        });
        if newest > self.last_ts {
            self.last_ts = newest;
            self.last_ts_at = Instant::now();
        }
        let now = self.now();
        if self.gs.target_since != self.last_target_since {
            self.last_target_since = self.gs.target_since;
            if self.gs.target.is_some() {
                self.flash_at = Some(Instant::now());
            }
        }
        if self.gs.boss != self.dps_boss {
            self.dps_boss = self.gs.boss.clone();
            self.dps_peak = 0;
        }
        let rolling = self.gs.rolling_dps(now);
        self.dps_peak = self.dps_peak.max(rolling);
        let dps_target = if self.gs.boss.is_some() && self.dps_peak > 0 {
            rolling as f32 / self.dps_peak as f32
        } else {
            0.0
        };
        let mut anim = approach(&mut self.progress_shown, self.gs.progress);
        anim |= approach(&mut self.dps_frac_shown, dps_target);
        anim |= self.flash_at.is_some() && self.flash_t() < 1.0;
        let redraw = self.gs.changed || anim || self.gs.boss.is_some();
        self.gs.changed = false;
        if redraw {
            self.render();
        } else {
            self.vr.submit(&self.rgba, self.renderer.width as u32, self.renderer.height as u32, false);
        }
        let want: u32 = if anim { 33 } else { 1000 };
        let timer_ms = (want != self.timer_ms).then_some(want);
        self.timer_ms = want;
        Tick { redraw, timer_ms }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = "2026.09.07 09:12:28 Debug      -  ";

    fn headless() -> App {
        let mut app = App::new(Renderer::new(96));
        app.watch = LogWatch::default();
        app
    }

    #[test]
    fn flash_on_target_change() {
        let mut app = headless();
        let t = app.tick();
        assert!(!t.redraw);
        assert_eq!(t.timer_ms, None);
        app.gs.feed(&format!("{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1"));
        app.gs.feed(&format!("{P}ownership of Kakarot transferred to Alice"));
        let t = app.tick();
        assert!(t.redraw);
        assert_eq!(t.timer_ms, Some(33));
        assert!(app.flash_at.is_some());
        app.gs.feed(&format!("{P}Boss Kakarot dead, personal damage dealt: "));
        app.tick();
        assert_eq!(app.dps_boss, None);
    }

    #[test]
    fn timer_settles_after_animation() {
        let mut app = headless();
        app.gs.feed(&format!("{P}ECLIPTICA - now in stage: Stage_Test on phase: 0.5 as class: Blade"));
        let t = app.tick();
        assert!(t.redraw);
        assert_eq!(t.timer_ms, Some(33));
        for _ in 0..200 {
            if app.tick().timer_ms == Some(1000) {
                assert_eq!(app.progress_shown, 0.5);
                return;
            }
        }
        panic!("animation never settled");
    }
}
