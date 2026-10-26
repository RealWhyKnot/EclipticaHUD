use super::fmt::{fmt_count, fmt_dur};
use super::layout::*;
use super::theme::*;
use super::Renderer;
use crate::game::state::GameState;
use windows_sys::Win32::Graphics::Gdi::*;

pub const ENEMY_CAP: usize = 999;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Shown {
    pub n: usize,
    pub clear: bool,
}

pub(super) fn enemies_shown(gs: &GameState, now: u64) -> Option<Shown> {
    let n = gs.enemies_left()?;
    Some(Shown {
        n,
        clear: n == 0 && gs.clear_settled(now),
    })
}

pub(super) fn enemy_num(n: usize) -> String {
    if n > ENEMY_CAP {
        format!("{ENEMY_CAP}+")
    } else {
        n.to_string()
    }
}

pub(super) fn enemies_text(s: Shown) -> String {
    if s.clear {
        return "all clear".to_string();
    }
    let unit = if s.n == 1 {
        "enemy left"
    } else {
        "enemies left"
    };
    format!("{} {unit}", enemy_num(s.n))
}

pub(super) const PAD: i32 = 12;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Slot {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub font: usize,
    pub color: u32,
    pub flags: u32,
    pub text: String,
}

pub(super) struct ClearingIn {
    pub secs: u64,
    pub tokens: Option<(u32, u32)>,
    pub enemies: Option<Shown>,
}

fn tone(s: Shown) -> u32 {
    if s.clear {
        GOOD
    } else if s.n == 0 {
        DIM
    } else {
        TEXT
    }
}

pub(super) fn clearing_slots(c: &ClearingIn) -> Vec<Slot> {
    let (x, w) = (M + PAD, W - 2 * PAD);
    let slot = |y, h, font, color, flags, text: String| Slot {
        x,
        y,
        w,
        h,
        font,
        color,
        flags,
        text,
    };
    let right = DT_RIGHT | DT_VCENTER;
    let mut out = vec![
        slot(114, 40, F_LABEL, DIM, DT_LEFT, "CLEARING".into()),
        slot(128, 40, F_BOSS, AMBER, DT_LEFT, fmt_dur(c.secs)),
    ];
    if let Some((got, total)) = c.tokens {
        let t = format!("{got}/{}", fmt_count(total as usize, "token", "tokens"));
        out.push(slot(128, 22, F_TINY, DIM, right, t));
    }
    if let Some(e) = c.enemies {
        out.push(slot(114, 16, F_TINY, tone(e), right, enemies_text(e)));
    }
    out
}

impl Renderer {
    pub(super) fn draw_clearing(&self, c: &ClearingIn) {
        for s in clearing_slots(c) {
            self.text_rect(s.x, s.y, s.w, s.h, s.font, s.color, s.flags, &s.text);
        }
    }
}
