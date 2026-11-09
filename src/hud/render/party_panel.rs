use super::fmt::{compact, group_digits};
use super::frame::{PartyPane, PartyView};
use super::hit::PartyHit;
use super::layout::{
    M, PARTY_BAR, PARTY_CARD1, PARTY_CARD2, PARTY_CLOSE_BTN, PARTY_DMG, PARTY_H, PARTY_NAME,
    PARTY_NEWER, PARTY_OLDER, PARTY_PAGE, PARTY_RANK, PARTY_ROW_H, PARTY_ROW_Y, PARTY_ROWS,
    PARTY_TAKEN, PARTY_TOGGLE, PARTY_W,
};
use super::theme::{
    ACCENT, AMBER, BG, CARD, CARD_HI, DANGER, DIM, EDGE, F_BIG, F_BODY, F_LABEL, F_TINY,
    GLYPH_CLOSE, GLYPH_NEXT, GLYPH_PREV, GOOD, TEXT, mix,
};
use super::{Renderer, wide};
use crate::game::event::{fmt_clock, fmt_date};
use crate::game::state::Life;
use crate::relay::Link;
use crate::relay::party::PartyRow;
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::{
    DT_CALCRECT, DT_CENTER, DT_LEFT, DT_NOPREFIX, DT_RIGHT, DT_VCENTER, DT_WORDBREAK, DrawTextW,
    GdiFlush, SelectObject, SetTextColor,
};

const OFF_LINES: [&str; 3] = [
    "While a boss fight is on, the HUD sends the server how much damage you've dealt and taken, at most once a second.",
    "Players in this instance who turn sharing on see those numbers beside your VRChat name, and you see theirs. Late joiners also see your run's stages and bosses but don't keep them.",
    "The server keeps all of it in memory only and drops yours when your run ends or you disconnect.",
];

pub(super) fn dead_look(life: Life) -> Option<(u32, &'static str)> {
    match life {
        Life::Down => Some((AMBER, "DEAD \u{b7} CAN REVIVE")),
        Life::Out => Some((DANGER, "DEAD \u{b7} NO REVIVE")),
        Life::Alive | Life::OneLeft => None,
    }
}

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
    pub(crate) fn board_head(&self) -> &'static str {
        if self.saved.is_some() {
            return "SAVED FIGHT";
        }
        match &self.fight {
            Some(f) if f.live => "THIS FIGHT",
            Some(_) => "LAST FIGHT",
            None => "BOSS DAMAGE",
        }
    }

    pub(crate) fn board_empty(&self) -> Option<&'static str> {
        if !self.rows.is_empty() {
            return None;
        }
        Some(match (self.ready, self.link) {
            (false, _) => "Nothing to show outside Ecliptica",
            (true, Link::Retrying) => "Can't reach the server. The HUD keeps trying.",
            (true, Link::Full) => "Sharing is full here. The HUD keeps trying.",
            (true, Link::Taken) => "Another HUD is sharing as you. Trying again soon.",
            (true, Link::Off | Link::Connecting) => "Connecting to the server",
            (true, Link::Connected) => "Damage shows up here once a boss fight starts",
        })
    }

    pub(crate) fn saved_when(&self) -> Option<String> {
        self.saved
            .map(|at| format!("{}, {}", fmt_date(at), &fmt_clock(at)[..5]))
    }

    pub(crate) fn page_label(&self) -> String {
        let total = self.pages + 1;
        format!("{}/{}", total - self.page.min(self.pages), total)
    }

    pub(crate) fn tip(&self, hit: PartyHit) -> &'static str {
        match (hit, self.on, self.ready, self.link) {
            (PartyHit::Status, true, true, Link::Full) => {
                "Too many players are sharing in this instance right now"
            }
            (PartyHit::Status, true, true, Link::Taken) => {
                "Your VRChat account is sharing here from another HUD; close that one"
            }
            _ => hit.tip(self.on),
        }
    }

    pub(crate) fn status(&self) -> (&'static str, u32) {
        match (self.on, self.ready, self.link) {
            (false, _, _) => ("off", DIM),
            (true, false, _) => ("waiting for Ecliptica", DIM),
            (true, true, Link::Connected) => ("connected", GOOD),
            (true, true, Link::Off | Link::Connecting) => ("connecting", AMBER),
            (true, true, Link::Retrying) => ("can't reach the server", DANGER),
            (true, true, Link::Full) => ("sharing is full", DANGER),
            (true, true, Link::Taken) => ("already sharing", DANGER),
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

#[derive(Clone, Copy)]
struct BoardLook {
    leader: u64,
    full: bool,
    full_taken: bool,
    stale: bool,
}

impl Renderer {
    pub(crate) fn draw_party(&mut self, v: &PartyView) {
        let p = v.pane;
        self.fill(0, 0, PARTY_W, PARTY_H, BG);
        let vc = DT_VCENTER;
        self.text_rect(
            (M, 8, 160, 24),
            F_LABEL,
            ACCENT,
            DT_LEFT | vc,
            "SHARED DAMAGE",
        );
        if !v.vr {
            let (tx, ty, _, th) = PARTY_TOGGLE;
            self.text_rect((tx - 88, ty, 80, th), F_TINY, DIM, DT_RIGHT | vc, "sharing");
            let word = if p.on { "On" } else { "Off" };
            self.text_button(PARTY_TOGGLE, v.ctl(PartyHit::Toggle, p.on), word);
            self.glyph_button(PARTY_CLOSE_BTN, v.ctl(PartyHit::Close, false), GLYPH_CLOSE);
        }
        self.draw_party_status(p);
        self.draw_party_board(p);
        if p.pages > 0 {
            self.draw_party_pager(v);
        }
        if let Some(hit) = v.tip.filter(|_| !v.vr) {
            self.tooltip(hit.rect(), p.tip(hit), PARTY_W, PARTY_H);
        }
        // SAFETY: GdiFlush takes no arguments and only waits for pending drawing.
        unsafe { GdiFlush() };
    }

    fn draw_party_status(&self, p: &PartyPane) {
        let (cx, cy, cw, ch) = PARTY_CARD1;
        self.rround(cx, cy, cw, ch, 8, CARD);
        let (x, w) = (cx + 12, cw - 24);
        let vc = DT_VCENTER;
        let (word, color) = p.status();
        let when = p.saved_when();
        let shown = when.as_deref().unwrap_or(word);
        let ww = self.unscale(self.text_w(F_TINY, shown));
        self.text_rect((x, cy + 6, w, 18), F_TINY, DIM, DT_RIGHT | vc, shown);
        let lead = if when.is_none() {
            self.dot(x + w - ww - 14, cy + 11, 8, color);
            26
        } else {
            12
        };
        let title = p.fight_title();
        let right = x + w;
        self.text_rect(
            (x, cy + 6, w - ww - lead, 18),
            F_LABEL,
            DIM,
            DT_LEFT | vc,
            &title,
        );
        let (head, sub) = match (p.on || p.saved.is_some(), p.you()) {
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
                self.text_rect((x, cy + 28, rw + 2, 36), F_BIG, TEXT, DT_LEFT | vc, &rank);
                let sx = x + rw + 10;
                let of = format!("of {} sharing", p.rows.len());
                self.text_rect(
                    (sx, cy + 29, right - sx, 18),
                    F_BODY,
                    TEXT,
                    DT_LEFT | vc,
                    &of,
                );
                let gap = p
                    .gap_line()
                    .unwrap_or_else(|| "no one else here has sent damage yet".to_string());
                self.text_rect(
                    (sx, cy + 47, right - sx, 16),
                    F_TINY,
                    DIM,
                    DT_LEFT | vc,
                    &gap,
                );
                return;
            }
        };
        self.text_rect(
            (x, cy + 28, right - x, 18),
            F_BODY,
            TEXT,
            DT_LEFT | vc,
            &head,
        );
        self.text_rect((x, cy + 47, right - x, 16), F_TINY, DIM, DT_LEFT | vc, &sub);
    }

    fn draw_party_pager(&self, v: &PartyView) {
        let p = v.pane;
        self.glyph_button(
            PARTY_OLDER,
            v.ctl_if(PartyHit::Older, p.page < p.pages),
            GLYPH_PREV,
        );
        self.glyph_button(
            PARTY_NEWER,
            v.ctl_if(PartyHit::Newer, p.page > 0),
            GLYPH_NEXT,
        );
        let label = p.page_label();
        self.text_rect(PARTY_PAGE, F_TINY, DIM, DT_CENTER | DT_VCENTER, &label);
    }

    fn draw_party_board(&self, pane: &PartyPane) {
        let (cx, cy, cw, ch) = PARTY_CARD2;
        self.rround(cx, cy, cw, ch, 8, CARD);
        let (x, width) = (cx + 12, cw - 24);
        let vc = DT_VCENTER;
        if !pane.on && pane.saved.is_none() {
            self.text_rect(
                (x, cy + 6, width, 18),
                F_LABEL,
                DIM,
                DT_LEFT | vc,
                "WHAT GETS SHARED",
            );
            let mut y = cy + 34;
            for line in OFF_LINES {
                y += self.para(x, y, width, F_BODY, DIM, line) + 12;
            }
            return;
        }
        self.text_rect(
            (x, cy + 6, width, 18),
            F_LABEL,
            DIM,
            DT_LEFT | vc,
            pane.board_head(),
        );
        if let Some(msg) = pane.board_empty() {
            let (by, bh) = (cy + 24, ch - 24);
            self.text_rect((x, by, width, bh), F_BODY, DIM, DT_CENTER | vc, msg);
            return;
        }
        let total = pane.rows.len();
        let count = if pane.here >= total {
            format!("{total} of {} here sharing", pane.here)
        } else {
            format!("{total} sharing")
        };
        let (dx, dw) = PARTY_DMG;
        let (tx, tw) = PARTY_TAKEN;
        let hw = self.unscale(self.text_w(F_LABEL, pane.board_head()));
        let lw = self.unscale(self.text_w(F_LABEL, "DEALT"));
        let count_x = x + hw + 10;
        let count_w = dx + dw - lw - 6 - count_x;
        self.text_rect(
            (count_x, cy + 6, count_w, 18),
            F_TINY,
            DIM,
            DT_LEFT | vc,
            &count,
        );
        self.text_rect((dx, cy + 6, dw, 18), F_LABEL, DIM, DT_RIGHT | vc, "DEALT");
        self.text_rect(
            (tx, cy + 6, tw, 18),
            F_LABEL,
            mix(DIM, DANGER, 0.5),
            DT_RIGHT | vc,
            "TAKEN",
        );
        let board = pane.board();
        let leader = pane.rows[0].dmg.max(1);
        let full = board
            .iter()
            .all(|(_, row)| self.text_w(F_BODY, &group_digits(row.dmg)) <= self.px(dw));
        let full_t = board
            .iter()
            .all(|(_, row)| self.text_w(F_BODY, &group_digits(row.taken)) <= self.px(tw));
        let stale = pane.link != Link::Connected && pane.saved.is_none();
        let look = BoardLook {
            leader,
            full,
            full_taken: full_t,
            stale,
        };
        for (slot, (index, row)) in board.into_iter().enumerate() {
            self.party_board_row(row, slot, index, &look);
        }
    }

    fn party_board_row(&self, row: &PartyRow, slot: usize, index: usize, look: &BoardLook) {
        let (cx, _, cw, _) = PARTY_CARD2;
        let vc = DT_VCENTER;
        let (dx, dw) = PARTY_DMG;
        let (tx, tw) = PARTY_TAKEN;
        let height = PARTY_ROW_H;
        let BoardLook {
            leader,
            full,
            full_taken: full_t,
            stale,
        } = *look;
        let y = PARTY_ROW_Y + crate::num::to_i32(slot) * height;
        let base = if row.you { CARD_HI } else { CARD };
        let dead = dead_look(Life::from_wire(row.life));
        if let Some((tone, _)) = dead {
            self.rround(cx + 6, y, cw - 12, height, 5, mix(base, tone, 0.22));
        } else if row.you {
            self.rround(cx + 6, y, cw - 12, height, 5, CARD_HI);
        }
        let ink = if stale && !row.you { DIM } else { TEXT };
        let name_ink = if row.you { ACCENT } else { ink };
        let rank = (index + 1).to_string();
        let (rx, rw) = PARTY_RANK;
        self.text_rect((rx, y, rw, height), F_TINY, DIM, DT_RIGHT | vc, &rank);
        let (nx, nw) = PARTY_NAME;
        match dead {
            Some((tone, _)) => {
                let pw = self.unscale(self.text_w(F_LABEL, "DEAD")) + 12;
                if tone == DANGER {
                    self.rround(nx, y + 4, pw, height - 8, 4, mix(base, DANGER, 0.7));
                    self.text_rect((nx, y, pw, height), F_LABEL, TEXT, DT_CENTER | vc, "DEAD");
                } else {
                    self.outline(nx, y + 4, pw, height - 8, 4, AMBER);
                    self.text_rect((nx, y, pw, height), F_LABEL, AMBER, DT_CENTER | vc, "DEAD");
                }
                let tx = nx + pw + 6;
                self.text_rect(
                    (tx, y, nw - pw - 6, height),
                    F_BODY,
                    name_ink,
                    DT_LEFT | vc,
                    &row.name,
                );
            }
            None => {
                self.text_rect(
                    (nx, y, nw, height),
                    F_BODY,
                    name_ink,
                    DT_LEFT | vc,
                    &row.name,
                );
            }
        }
        let frac = crate::num::to_f32(row.dmg) / crate::num::to_f32(leader);
        let bar = match (row.you, stale) {
            (true, _) => ACCENT,
            (false, false) => EDGE,
            (false, true) => mix(EDGE, CARD, 0.5),
        };
        let (bx, bw) = PARTY_BAR;
        self.bar_on((bx, y + height / 2 - 2, bw, 4), frac, bar, BG);
        let num = if full {
            group_digits(row.dmg)
        } else {
            compact(row.dmg)
        };
        self.text_rect((dx, y, dw, height), F_BODY, ink, DT_RIGHT | vc, &num);
        let taken = if full_t {
            group_digits(row.taken)
        } else {
            compact(row.taken)
        };
        let tink = if stale && !row.you {
            DIM
        } else {
            mix(TEXT, DANGER, 0.6)
        };
        self.text_rect((tx, y, tw, height), F_BODY, tink, DT_RIGHT | vc, &taken);
    }

    pub(super) fn para(
        &self,
        x: i32,
        y: i32,
        width: i32,
        font: usize,
        color: u32,
        text: &str,
    ) -> i32 {
        // SAFETY: `self.dc` and the font are owned by this renderer, and the text buffer outlives the call.
        unsafe {
            SelectObject(self.dc, self.fonts[font].cast());
            SetTextColor(self.dc, color);
            let mut buf = wide(text);
            let flags = DT_LEFT | DT_WORDBREAK | DT_NOPREFIX;
            let mut rect = RECT {
                left: self.px(x),
                top: self.px(y),
                right: self.px(x + width),
                bottom: self.px(y),
            };
            DrawTextW(
                self.dc,
                buf.as_mut_ptr(),
                crate::num::to_i32(buf.len()),
                &raw mut rect,
                flags | DT_CALCRECT,
            );
            let height = rect.bottom - rect.top;
            rect.right = self.px(x + width);
            DrawTextW(
                self.dc,
                buf.as_mut_ptr(),
                crate::num::to_i32(buf.len()),
                &raw mut rect,
                flags,
            );
            self.unscale(height)
        }
    }
}
