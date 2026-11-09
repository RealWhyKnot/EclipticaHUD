use super::frame::LogView;
use super::hit::LogHit;
use super::layout::{
    LOG_BODY, LOG_CLOSE_BTN, LOG_COUNT, LOG_H, LOG_TAB_H, LOG_TAB_Y, LOG_TABS, LOG_TRACK, LOG_W,
};
use super::theme::{
    ACCENT, BG, CARD, CARD_HI, DANGER, DIM, EDGE, F_BODY, F_LABEL, F_TINY, GLYPH_CLOSE, TEXT, mix,
};
use super::{Ctl, Renderer};
use crate::game::event::fmt_clock;
use crate::game::names::boss_name;
use crate::game::run::base_name;
use crate::game::source::{describe_source, generic_attacker};
use crate::hud::timeline::{self, Row};
use windows_sys::Win32::Graphics::Gdi::{
    DT_CENTER, DT_LEFT, DT_RIGHT, DT_VCENTER, GdiFlush, IntersectClipRect, RestoreDC, SaveDC,
};

struct LogRow<'a> {
    mark: u32,
    ts: u64,
    tag: (&'a str, usize, u32),
    who: (&'a str, u32),
    what: Option<&'a str>,
}

impl Renderer {
    pub(crate) fn draw_log(&mut self, src: Option<timeline::Source<'_>>, lv: &LogView) {
        self.fill(0, 0, LOG_W, LOG_H, BG);
        let rows = timeline::timeline(src, lv.filter);
        self.text_rect(
            (14, LOG_TAB_Y, 74, LOG_TAB_H),
            F_LABEL,
            ACCENT,
            DT_LEFT | DT_VCENTER,
            "EVENT LOG",
        );
        let count = match rows.iter().filter(|r| !matches!(r, Row::Fight(_))).count() {
            0 => "nothing yet".to_string(),
            1 => "1 event".to_string(),
            n => format!("{n} events"),
        };
        let (cx, cy, cw, ch) = LOG_COUNT;
        self.text_rect((cx, cy, cw, ch), F_TINY, DIM, DT_LEFT | DT_VCENTER, &count);
        for (filter, label, x, w) in LOG_TABS {
            let ctl = Ctl::rest(lv.hover == Some(LogHit::Tab(filter))).lit(lv.filter == filter);
            self.text_button((x, LOG_TAB_Y, w, LOG_TAB_H), ctl, label);
        }
        let close = Ctl::rest(lv.hover == Some(LogHit::Close));
        self.glyph_button(LOG_CLOSE_BTN, close, GLYPH_CLOSE);

        let (bx, by, bw, bh) = LOG_BODY;
        if rows.is_empty() {
            let msg = "nothing logged yet";
            self.text_rect((bx, by, bw, bh), F_BODY, DIM, DT_CENTER | DT_VCENTER, msg);
            if let Some(hit) = lv.tip {
                self.tooltip(hit.rect(lv.thumb), hit.tip(), LOG_W, LOG_H);
            }
            // SAFETY: GdiFlush takes no arguments and only waits for pending drawing.
            unsafe { GdiFlush() };
            return;
        }
        self.draw_rows(&rows, lv);
        let n = rows.len();
        if let Some((ty, th)) = timeline::thumb(n, bh, lv.scroll) {
            let (tx, tyy, tw, thh) = LOG_TRACK;
            self.rround(tx, tyy, tw, thh, 3, CARD);
            let lit = lv.dragging || lv.hover == Some(LogHit::Thumb);
            let color = if lit {
                ACCENT
            } else {
                mix(EDGE, DIM, lv.thumb_t)
            };
            self.rround(tx, tyy + ty, tw, th, 3, color);
        }
        if let Some(hit) = lv.tip {
            self.tooltip(hit.rect(lv.thumb), hit.tip(), LOG_W, LOG_H);
        }
        // SAFETY: GdiFlush takes no arguments and only waits for pending drawing.
        unsafe { GdiFlush() };
    }

    fn draw_rows(&self, rows: &[Row], lv: &LogView) {
        let (bx, by, bw, bh) = LOG_BODY;
        // SAFETY: `self.dc` is this renderer's live DC, and the state saved here is restored before returning.
        unsafe {
            let saved = SaveDC(self.dc);
            IntersectClipRect(
                self.dc,
                self.px(bx),
                self.px(by),
                self.px(bx + bw),
                self.px(by + bh),
            );
            let first = crate::num::to_usize(
                (lv.scroll / crate::num::to_f32(timeline::ROW_H))
                    .floor()
                    .max(0.0),
            );
            let mut y = by + (crate::num::to_i32(first) * timeline::ROW_H)
                - crate::num::to_i32(lv.scroll)
                + crate::num::to_i32(lv.slide);
            for row in rows.iter().skip(first) {
                if y > by + bh {
                    break;
                }
                self.draw_log_row(bx, y, bw, row);
                y += timeline::ROW_H;
            }
            RestoreDC(self.dc, saved);
        }
    }

    fn draw_log_row(&self, x: i32, y: i32, w: i32, row: &Row) {
        match row {
            Row::Fight(f) => {
                self.fill(x, y + 12, w, 1, CARD_HI);
                let name = boss_name(base_name(&f.name));
                let label = format!("{name}  {}", &fmt_clock(f.start_ts)[..5]);
                let tw = crate::num::to_i32(label.chars().count()) * 7 + 16;
                self.fill(x, y, tw, timeline::ROW_H, BG);
                self.text_rect(
                    (x + 4, y, tw, timeline::ROW_H),
                    F_LABEL,
                    DIM,
                    DT_LEFT | DT_VCENTER,
                    &label,
                );
            }
            Row::Hit(h) => {
                let (who, attack) = describe_source(&h.source);
                let who_color = if generic_attacker(&who) { DIM } else { TEXT };
                self.log_row(
                    x,
                    y,
                    w,
                    &LogRow {
                        mark: DANGER,
                        ts: h.ts,
                        tag: (&h.amount.to_string(), F_BODY, DANGER),
                        who: (&who, who_color),
                        what: Some(&attack),
                    },
                );
            }
            Row::Target(t) => self.log_row(
                x,
                y,
                w,
                &LogRow {
                    mark: ACCENT,
                    ts: t.ts,
                    tag: ("aggro", F_TINY, ACCENT),
                    who: (&t.player, TEXT),
                    what: Some(&boss_name(base_name(&t.boss))),
                },
            ),
            Row::Death { ts, .. } => self.log_row(
                x,
                y,
                w,
                &LogRow {
                    mark: DANGER,
                    ts: *ts,
                    tag: ("death", F_TINY, DANGER),
                    who: ("you died", DANGER),
                    what: None,
                },
            ),
        }
    }

    fn log_row(&self, x: i32, y: i32, width: i32, row: &LogRow) {
        let vc = DT_LEFT | DT_VCENTER;
        let height = timeline::ROW_H;
        self.rround(x, y + 5, 3, height - 10, 1, row.mark);
        self.text_rect((x + 12, y, 62, height), F_TINY, DIM, vc, &fmt_clock(row.ts));
        let (tag, font, color) = row.tag;
        self.text_rect(
            (x + 76, y, 44, height),
            font,
            color,
            DT_RIGHT | DT_VCENTER,
            tag,
        );
        let who_w = if row.what.is_some() { 120 } else { width - 130 };
        self.text_rect(
            (x + 130, y, who_w, height),
            F_BODY,
            row.who.1,
            vc,
            row.who.0,
        );
        if let Some(what) = row.what {
            self.text_rect((x + 254, y, width - 254, height), F_BODY, DIM, vc, what);
        }
    }
}
