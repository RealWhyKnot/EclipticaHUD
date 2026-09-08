use crate::logwatch::{self, LogWatch};
use crate::render::{Frame, Renderer};
use crate::state::{GameState, Run};
use crate::update::{self, Badge};
use crate::vr::VrOverlay;
use std::io::Read;
use std::path::Path;
use std::time::Instant;

const FLASH_MS: f32 = 3000.0;

pub struct Tick {
    pub redraw: bool,
    pub timer_ms: Option<u32>,
    pub quit: bool,
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
    pub sound_on: bool,
    pub sel_run: Option<usize>,
    pub sel_group: Option<usize>,
    pub sel_phase: Option<usize>,
    flash_at: Option<Instant>,
    last_target_since: u64,
    progress_shown: f32,
    dps_peak: u64,
    dps_boss: Option<String>,
    dps_frac_shown: f32,
    timer_ms: u32,
    badge: Badge,
}

fn blip() {
    #[cfg(not(test))]
    unsafe {
        static BLIP: &[u8] = include_bytes!("../assets/target.wav");
        use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
        PlaySoundW(
            BLIP.as_ptr() as _,
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
        let Ok(mut file) = logwatch::open_shared(path) else {
            continue;
        };
        let mut bytes = Vec::new();
        if file.read_to_end(&mut bytes).is_err() {
            continue;
        }
        for line in String::from_utf8_lossy(&bytes).lines() {
            gs.feed(line);
        }
        gs.log_rotated();
    }
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
            sound_on: true,
            sel_run: None,
            sel_group: None,
            sel_phase: None,
            flash_at: None,
            last_target_since: 0,
            progress_shown: 0.0,
            dps_peak: 0,
            dps_boss: None,
            dps_frac_shown: 0.0,
            timer_ms: 1000,
            badge: Badge::None,
        }
    }

    pub fn update_ready(&self) -> bool {
        matches!(self.badge, Badge::Ready(_))
    }

    pub fn backfill_history(&mut self) {
        if let Some(dir) = logwatch::log_dir() {
            backfill(&mut self.gs, &dir);
        }
    }

    pub fn viewed_run(&self) -> Option<(usize, &Run)> {
        let n = self.gs.runs.len();
        let i = self.sel_run.unwrap_or(n.checked_sub(1)?).min(n - 1);
        Some((i, &self.gs.runs[i]))
    }

    pub fn viewed_group(&self) -> Option<(usize, std::ops::Range<usize>)> {
        let (_, run) = self.viewed_run()?;
        let groups = run.groups();
        let n = groups.len();
        let i = self.sel_group.unwrap_or(n.checked_sub(1)?).min(n - 1);
        Some((i, groups[i].clone()))
    }

    pub fn is_live(&self) -> bool {
        self.sel_run.is_none() && self.sel_group.is_none() && self.sel_phase.is_none()
    }

    pub fn run_prev(&mut self) -> bool {
        match self.viewed_run() {
            Some((i, _)) if i > 0 => {
                self.sel_run = Some(i - 1);
                self.sel_group = None;
                self.sel_phase = None;
                true
            }
            _ => false,
        }
    }

    pub fn run_next(&mut self) -> bool {
        match self.sel_run {
            Some(i) => {
                self.sel_run = (i + 2 < self.gs.runs.len()).then_some(i + 1);
                self.sel_group = None;
                self.sel_phase = None;
                true
            }
            None => false,
        }
    }

    pub fn group_prev(&mut self) -> bool {
        match self.viewed_group() {
            Some((i, _)) if i > 0 => {
                self.sel_group = Some(i - 1);
                self.sel_phase = None;
                true
            }
            _ => false,
        }
    }

    pub fn group_next(&mut self) -> bool {
        match self.sel_group {
            Some(i) => {
                let n = self.viewed_run().map_or(0, |(_, r)| r.groups().len());
                self.sel_group = (i + 2 < n).then_some(i + 1);
                self.sel_phase = None;
                true
            }
            None => false,
        }
    }

    pub fn phase_cycle(&mut self) -> bool {
        if self.is_live() {
            return false;
        }
        let Some((_, g)) = self.viewed_group() else {
            return false;
        };
        if g.len() < 2 {
            return false;
        }
        self.sel_phase = match self.sel_phase {
            None => Some(0),
            Some(i) if i + 1 < g.len() => Some(i + 1),
            Some(_) => None,
        };
        true
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
            update: self.badge.clone(),
            sound_on: self.sound_on,
            view_run: self.viewed_run().map(|(i, _)| i),
            view_group: self.viewed_group().map(|(i, _)| i),
            view_phase: self.sel_phase,
            run_sel: self.sel_run.is_some(),
            group_sel: self.sel_group.is_some(),
        };
        self.renderer.draw(&mut self.gs, &frame);
        self.renderer.rgba(&mut self.rgba);
        self.vr.submit(
            &self.rgba,
            self.renderer.width as u32,
            self.renderer.height as u32,
            true,
        );
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
        let now = self.now();
        if self.gs.target_since != self.last_target_since {
            self.last_target_since = self.gs.target_since;
            if self.gs.target.is_some() {
                self.flash_at = Some(Instant::now());
                if self.sound_on {
                    blip();
                }
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
        let badge = update::badge();
        let badge_changed = badge != self.badge;
        if badge_changed {
            self.badge = badge;
        }
        let mut anim = approach(&mut self.progress_shown, self.gs.progress);
        anim |= approach(&mut self.dps_frac_shown, dps_target);
        anim |= self.flash_at.is_some() && self.flash_t() < 1.0;
        let redraw = self.gs.changed || anim || self.gs.boss.is_some() || badge_changed;
        self.gs.changed = false;
        if redraw {
            self.render();
        } else {
            self.vr.submit(
                &self.rgba,
                self.renderer.width as u32,
                self.renderer.height as u32,
                false,
            );
        }
        let want: u32 = if anim { 33 } else { 1000 };
        let timer_ms = (want != self.timer_ms).then_some(want);
        self.timer_ms = want;
        Tick {
            redraw,
            timer_ms,
            quit: update::restart_pending(),
        }
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
        assert_eq!(t.timer_ms, Some(33));
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
        let mut app = App::new(Renderer::new(96));
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
        app.gs.feed(&format!("{P}{STAGE_B}"));
        assert_eq!(app.gs.runs.len(), 2);
        assert!(app.is_live());
        assert!(!app.run_next());
        assert!(!app.group_next());
        assert!(!app.phase_cycle());
        assert!(app.run_prev());
        assert_eq!(app.sel_run, Some(0));
        let (gi, g) = app.viewed_group().unwrap();
        assert_eq!((gi, g.len()), (1, 1));
        assert!(app.group_prev());
        let (gi, g) = app.viewed_group().unwrap();
        assert_eq!((gi, g.len()), (0, 2));
        assert!(!app.group_prev());
        assert!(app.phase_cycle());
        assert_eq!(app.sel_phase, Some(0));
        assert!(app.phase_cycle());
        assert_eq!(app.sel_phase, Some(1));
        assert!(app.phase_cycle());
        assert_eq!(app.sel_phase, None);
        assert!(app.group_next());
        assert_eq!((app.sel_group, app.sel_phase), (None, None));
        assert!(app.run_next());
        assert!(app.is_live());
    }

    #[test]
    fn timer_settles_after_animation() {
        let mut app = headless();
        app.gs.feed(&format!(
            "{P}ECLIPTICA - now in stage: Stage_Test on phase: 0.5 as class: Blade"
        ));
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
