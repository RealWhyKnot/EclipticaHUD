use super::fmt::{compact, group_digits};
use super::frame::{PartyPane, PartyView};
use super::hit::PartyHit;
use super::layout::*;
use super::theme::*;
use super::{wide, Renderer};
use crate::relay::party::PartyRow;
use crate::relay::Link;
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::*;

const OFF_LINES: [&str; 3] = [
    "While a boss fight is on, the HUD sends your damage total for it to the server, at most once a second.",
    "Players in this instance who turn sharing on see it beside your VRChat name, and you see theirs.",
    "The server keeps totals in memory only and drops yours 10 seconds after you stop sending.",
];

pub(super) fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

fn clock(secs: u64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

impl PartyPane {
    pub fn board_head(&self) -> &'static str {
        match &self.fight {
            Some(f) if f.live => "THIS FIGHT",
            Some(_) => "LAST FIGHT",
            None => "BOSS DAMAGE",
        }
    }

    pub fn board_empty(&self) -> Option<&'static str> {
        if !self.rows.is_empty() {
            return None;
        }
        Some(match (self.ready, self.link) {
            (false, _) => "Nothing to show outside Ecliptica",
            (true, Link::Retrying) => "Can't reach the server. The HUD keeps trying.",
            (true, Link::Off | Link::Connecting) => "Connecting to the server",
            (true, Link::Connected) => "Damage shows up here once a boss fight starts",
        })
    }

    pub fn status(&self) -> (&'static str, u32) {
        match (self.on, self.ready, self.link) {
            (false, _, _) => ("off", DIM),
            (true, false, _) => ("waiting for Ecliptica", DIM),
            (true, true, Link::Connected) => ("connected", GOOD),
            (true, true, Link::Off | Link::Connecting) => ("connecting", AMBER),
            (true, true, Link::Retrying) => ("can't reach the server", DANGER),
        }
    }

    pub(super) fn board(&self) -> Vec<(usize, &PartyRow)> {
        let mut out: Vec<_> = self.rows.iter().enumerate().take(PARTY_ROWS).collect();
        if let Some(i) = self.you().filter(|&i| i >= PARTY_ROWS) {
            out.truncate(PARTY_ROWS - 1);
            out.push((i, &self.rows[i]));
        }
        out
    }

    pub(super) fn gap_line(&self) -> Option<String> {
        let i = self.you()?;
        let me = &self.rows[i];
        let (other, ahead) = match i {
            0 => (self.rows.get(1)?, true),
            _ => (&self.rows[i - 1], false),
        };
        let gap = me.dmg.abs_diff(other.dmg);
        Some(match (gap, ahead) {
            (0, _) => format!("tied with {}", other.name),
            (_, true) => format!("{} ahead of {}", compact(gap), other.name),
            (_, false) => format!("{} behind {}", compact(gap), other.name),
        })
    }

    fn fight_title(&self) -> String {
        match &self.fight {
            Some(f) if f.live => format!("{}   {}", f.name.to_uppercase(), clock(f.secs)),
            Some(f) if f.down => format!("{} DOWN   {}", f.name.to_uppercase(), clock(f.secs)),
            Some(f) => format!("{}   {}", f.name.to_uppercase(), clock(f.secs)),
            None => String::new(),
        }
    }
}

impl Renderer {
    pub fn draw_party(&mut self, v: &PartyView) {
        let p = v.pane;
        self.fill(0, 0, PARTY_W, PARTY_H, BG);
        let vc = DT_VCENTER;
        self.text_rect(
            M,
            8,
            160,
            24,
            F_LABEL,
            ACCENT,
            DT_LEFT | vc,
            "SHARED DAMAGE",
        );
        if !v.vr {
            let (tx, ty, _, th) = PARTY_TOGGLE;
            self.text_rect(tx - 88, ty, 80, th, F_TINY, DIM, DT_RIGHT | vc, "sharing");
            let word = if p.on { "On" } else { "Off" };
            self.text_button(PARTY_TOGGLE, v.ctl(PartyHit::Toggle, p.on), word);
            self.glyph_button(PARTY_CLOSE_BTN, v.ctl(PartyHit::Close, false), GLYPH_CLOSE);
        }
        self.draw_party_status(p);
        self.draw_party_board(p);
        if let Some(hit) = v.tip.filter(|_| !v.vr) {
            self.tooltip(hit.rect(), hit.tip(p.on), PARTY_W, PARTY_H);
        }
        unsafe { GdiFlush() };
    }

    fn draw_party_status(&self, p: &PartyPane) {
        let (cx, cy, cw, ch) = PARTY_CARD1;
        self.rround(cx, cy, cw, ch, 8, CARD);
        let (x, w) = (cx + 12, cw - 24);
        let vc = DT_VCENTER;
        let (word, color) = p.status();
        let ww = self.unscale(self.text_w(F_TINY, word));
        self.text_rect(x, cy + 6, w, 18, F_TINY, DIM, DT_RIGHT | vc, word);
        self.dot(x + w - ww - 14, cy + 11, 8, color);
        let title = p.fight_title();
        self.text_rect(
            x,
            cy + 6,
            w - ww - 26,
            18,
            F_LABEL,
            DIM,
            DT_LEFT | vc,
            &title,
        );
        let (head, sub) = match (p.on, p.you()) {
            (false, _) => (
                "Sharing is off".to_string(),
                "Turn it on above to compare boss damage with others here".to_string(),
            ),
            (true, None) if !p.ready => (
                "Not in Ecliptica".to_string(),
                "Sharing starts when you join the world".to_string(),
            ),
            (true, None) => (
                "Waiting for a boss fight".to_string(),
                "Your place shows here once you deal boss damage".to_string(),
            ),
            (true, Some(i)) => {
                let rank = ordinal(i + 1);
                let rw = self.unscale(self.text_w(F_BIG, &rank));
                self.text_rect(x, cy + 28, rw + 2, 36, F_BIG, TEXT, DT_LEFT | vc, &rank);
                let sx = x + rw + 10;
                let of = format!("of {} sharing", p.rows.len());
                self.text_rect(sx, cy + 29, x + w - sx, 18, F_BODY, TEXT, DT_LEFT | vc, &of);
                let gap = p
                    .gap_line()
                    .unwrap_or_else(|| "no one else here has sent damage yet".to_string());
                self.text_rect(sx, cy + 47, x + w - sx, 16, F_TINY, DIM, DT_LEFT | vc, &gap);
                return;
            }
        };
        self.text_rect(x, cy + 28, w, 18, F_BODY, TEXT, DT_LEFT | vc, &head);
        self.text_rect(x, cy + 47, w, 16, F_TINY, DIM, DT_LEFT | vc, &sub);
    }

    fn draw_party_board(&self, p: &PartyPane) {
        let (cx, cy, cw, ch) = PARTY_CARD2;
        self.rround(cx, cy, cw, ch, 8, CARD);
        let (x, w) = (cx + 12, cw - 24);
        let vc = DT_VCENTER;
        if !p.on {
            self.text_rect(
                x,
                cy + 6,
                w,
                18,
                F_LABEL,
                DIM,
                DT_LEFT | vc,
                "WHAT GETS SHARED",
            );
            let mut y = cy + 34;
            for line in OFF_LINES {
                y += self.para(x, y, w, F_BODY, DIM, line) + 12;
            }
            return;
        }
        self.text_rect(x, cy + 6, w, 18, F_LABEL, DIM, DT_LEFT | vc, p.board_head());
        if let Some(msg) = p.board_empty() {
            let (by, bh) = (cy + 24, ch - 24);
            self.text_rect(x, by, w, bh, F_BODY, DIM, DT_CENTER | vc, msg);
            return;
        }
        let n = p.rows.len();
        let count = if p.here >= n {
            format!("{n} of {} here sharing", p.here)
        } else {
            format!("{n} sharing")
        };
        self.text_rect(x, cy + 6, w, 18, F_TINY, DIM, DT_RIGHT | vc, &count);
        let board = p.board();
        let leader = p.rows[0].dmg.max(1);
        let full = board
            .iter()
            .all(|(_, r)| self.text_w(F_BODY, &group_digits(r.dmg)) <= self.px(PARTY_DMG.1));
        let stale = p.link != Link::Connected;
        let h = PARTY_ROW_H;
        for (slot, (i, r)) in board.into_iter().enumerate() {
            let y = PARTY_ROW_Y + slot as i32 * h;
            if r.you {
                self.rround(cx + 6, y, cw - 12, h, 5, CARD_HI);
            }
            let ink = if stale && !r.you { DIM } else { TEXT };
            let name_ink = if r.you { ACCENT } else { ink };
            let rank = (i + 1).to_string();
            let (rx, rw) = PARTY_RANK;
            self.text_rect(rx, y, rw, h, F_TINY, DIM, DT_RIGHT | vc, &rank);
            let (nx, nw) = PARTY_NAME;
            self.text_rect(nx, y, nw, h, F_BODY, name_ink, DT_LEFT | vc, &r.name);
            let frac = r.dmg as f32 / leader as f32;
            let bar = match (r.you, stale) {
                (true, _) => ACCENT,
                (false, false) => EDGE,
                (false, true) => mix(EDGE, CARD, 0.5),
            };
            let (bx, bw) = PARTY_BAR;
            self.bar_on(bx, y + h / 2 - 2, bw, 4, frac, bar, BG);
            let num = if full {
                group_digits(r.dmg)
            } else {
                compact(r.dmg)
            };
            let (dx, dw) = PARTY_DMG;
            self.text_rect(dx, y, dw, h, F_BODY, ink, DT_RIGHT | vc, &num);
        }
    }

    pub(super) fn para(&self, x: i32, y: i32, w: i32, font: usize, color: u32, s: &str) -> i32 {
        unsafe {
            SelectObject(self.dc, self.fonts[font] as _);
            SetTextColor(self.dc, color);
            let mut buf = wide(s);
            let flags = DT_LEFT | DT_WORDBREAK | DT_NOPREFIX;
            let mut r = RECT {
                left: self.px(x),
                top: self.px(y),
                right: self.px(x + w),
                bottom: self.px(y),
            };
            DrawTextW(
                self.dc,
                buf.as_mut_ptr(),
                buf.len() as i32,
                &mut r,
                flags | DT_CALCRECT,
            );
            let h = r.bottom - r.top;
            r.right = self.px(x + w);
            DrawTextW(self.dc, buf.as_mut_ptr(), buf.len() as i32, &mut r, flags);
            self.unscale(h)
        }
    }
}
