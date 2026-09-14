use super::App;
use crate::hud::render::{MAX_ALPHA, MAX_SCALE, MIN_ALPHA, MIN_SCALE};

const SCALE_STEP: f32 = 0.1;
pub(super) const ALPHA_STEP: u8 = 10;
pub const WINDOW_STEPS: [u64; 12] = [3, 4, 5, 6, 7, 8, 9, 10, 15, 20, 25, 30];

impl App {
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.clamp(MIN_SCALE, MAX_SCALE);
        self.rescale();
    }

    pub fn set_alpha(&mut self, alpha: u8) {
        self.alpha = alpha.clamp(MIN_ALPHA, MAX_ALPHA);
    }

    pub(super) fn step_scale(&mut self, dir: f32) -> bool {
        let next = ((self.scale + dir * SCALE_STEP) * 10.0).round() / 10.0;
        let next = next.clamp(MIN_SCALE, MAX_SCALE);
        if (next - self.scale).abs() < 0.001 {
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
