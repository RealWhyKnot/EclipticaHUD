use super::App;
use std::time::Instant;

pub(super) const FLASH_MS: f32 = 3000.0;
pub(super) const TAKEN_FLASH_MS: f32 = 320.0;
pub(super) const FADE_MS: f32 = 200.0;
pub(super) const SLIDE_MS: f32 = 160.0;
pub(super) const THUMB_HOLD_MS: u128 = 900;
pub(super) const DEAD_PULSE_MS: f32 = 1100.0;
pub(super) const WARN_MS: f32 = 5000.0;
pub(super) const ANIM_MS: u32 = 16;

pub(super) fn approach(cur: &mut f32, target: f32, eps: f32) -> bool {
    approach_rate(cur, target, eps, 0.18)
}

pub(super) fn approach_rate(cur: &mut f32, target: f32, eps: f32, rate: f32) -> bool {
    let d = target - *cur;
    if d.abs() < eps {
        *cur = target;
        false
    } else {
        *cur += d * rate;
        true
    }
}

pub(super) fn timed(at: Option<Instant>, ms: f32) -> f32 {
    match at {
        Some(t) => (t.elapsed().as_millis() as f32 / ms).min(1.0),
        None => 1.0,
    }
}

impl App {
    pub(super) fn dead_pulse(&self) -> f32 {
        match self.dead_since {
            Some(t) => {
                let ph = t.elapsed().as_millis() as f32 / DEAD_PULSE_MS * std::f32::consts::TAU;
                ph.sin() * 0.5 + 0.5
            }
            None => 0.0,
        }
    }
}
