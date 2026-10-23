use super::App;
use crate::hud::render::{MAX_ALPHA, MAX_SCALE, MIN_ALPHA, MIN_SCALE};

const SCALE_STEP: f32 = 0.1;
pub(super) const ALPHA_STEP: u8 = 10;
pub const WINDOW_STEPS: [u64; 12] = [3, 4, 5, 6, 7, 8, 9, 10, 15, 20, 25, 30];

pub fn fit_scale(dpi: u32, size: (i32, i32), work: (i32, i32)) -> f32 {
    let mut s = MAX_SCALE;
    loop {
        let px = (dpi as f32 * s).round() / 96.0;
        let fits = (size.0 as f32 * px) as i32 <= work.0 && (size.1 as f32 * px) as i32 <= work.1;
        if fits || s <= MIN_SCALE + 0.001 {
            return s;
        }
        s = ((s - SCALE_STEP) * 10.0).round() / 10.0;
    }
}

impl App {
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.clamp(MIN_SCALE, MAX_SCALE);
        self.rescale();
    }

    pub fn scale_max(&self) -> f32 {
        self.scale_cap.clamp(MIN_SCALE, MAX_SCALE)
    }

    pub fn shown_scale(&self) -> f32 {
        self.scale.min(self.scale_max())
    }

    pub fn set_scale_cap(&mut self, cap: f32) -> bool {
        let before = self.shown_scale();
        self.scale_cap = cap;
        let changed = (self.shown_scale() - before).abs() > 0.001;
        if changed {
            self.rescale();
        }
        changed
    }

    pub fn set_alpha(&mut self, alpha: u8) {
        self.alpha = alpha.clamp(MIN_ALPHA, MAX_ALPHA);
    }

    pub(super) fn step_scale(&mut self, dir: f32) -> bool {
        let shown = self.shown_scale();
        let next = ((shown + dir * SCALE_STEP) * 10.0).round() / 10.0;
        let next = next.clamp(MIN_SCALE, self.scale_max());
        if (next - shown).abs() < 0.001 {
            return false;
        }
        self.set_scale(next);
        true
    }

    pub(super) fn step_alpha(&mut self, up: bool) -> bool {
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

    pub(super) fn step_window(&mut self, up: bool) -> bool {
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
}
