use super::frame::LogView;
use super::hit::LogHit;
use super::layout::*;
use super::theme::*;
use super::Renderer;
use crate::game::event::fmt_clock;
use crate::game::names::boss_name;
use crate::game::run::base_name;
use crate::game::source::{describe_source, generic_attacker};
use crate::hud::timeline::{self, Row};
use windows_sys::Win32::Graphics::Gdi::*;

struct LogRow<'a> {
    mark: u32,
    ts: u64,
    tag: (&'a str, usize, u32),
    who: (&'a str, u32),
    what: Option<&'a str>,
}

impl Renderer {
    pub fn draw_log(&mut self, src: Option<timeline::Source<'_>>, lv: &LogView) {
        self.fill(0, 0, LOG_W, LOG_H, BG);
        let rows = timeline::timeline(src, lv.filter);
        self.text_rect(
            14,
            LOG_TAB_Y,
            74,
            LOG_TAB_H,
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
        self.text_rect(
            88,
            LOG_TAB_Y,
            76,
            LOG_TAB_H,
            F_TINY,
            DIM,
            DT_LEFT | DT_VCENTER,
            &count,
        );
        for (filter, label, x, w) in LOG_TABS {
            let active = lv.filter == filter;
            let hover = lv.hover == Some(LogHit::Tab(filter));
            let (bg, fg) = if active {
                (ACCENT, BG)
            } else if hover {
                (CARD_HI, TEXT)
            } else {
                (CARD, DIM)
            };
            self.rround(x, LOG_TAB_Y, w, LOG_TAB_H, 6, bg);
            self.text_rect(
                x,
                LOG_TAB_Y,
                w,
                LOG_TAB_H,
                F_LABEL,
                fg,
                DT_CENTER | DT_VCENTER,
                label,
            );
        }
        self.glyph_button(
            LOG_CLOSE_BTN,
            lv.hover == Some(LogHit::Close),
            false,
            false,
            GLYPH_CLOSE,
        );

        let (bx, by, bw, bh) = LOG_BODY;
        if rows.is_empty() {
            self.text_rect(
                bx,
                by,
                bw,
                bh,
                F_BODY,
                DIM,
                DT_CENTER | DT_VCENTER,
                "nothing logged yet",
            );
            unsafe { GdiFlush() };
            return;
        }
        unsafe {
            let saved = SaveDC(self.dc);
            IntersectClipRect(
                self.dc,
                self.px(bx),
                self.px(by),
                self.px(bx + bw),
                self.px(by + bh),
            );
            let first = (lv.scroll / timeline::ROW_H as f32).floor().max(0.0) as usize;
            let mut y = by + (first as i32 * timeline::ROW_H) - lv.scroll as i32 + lv.slide as i32;
            for row in rows.iter().skip(first) {
                if y > by + bh {
                    break;
                }
                self.draw_log_row(bx, y, bw, row);
                y += timeline::ROW_H;
            }
            RestoreDC(self.dc, saved);
        }
        if let Some((ty, th)) = timeline::thumb(rows.len(), bh, lv.scroll) {
            let (tx, tyy, tw, thh) = LOG_TRACK;
            self.rround(tx, tyy, tw, thh, 3, CARD);
            let lit = lv.dragging || lv.hover == Some(LogHit::Thumb);
            let color = if lit {
                ACCENT
            } else {
                mix(CARD_HI, DIM, lv.thumb_t)
            };
            self.rround(tx, tyy + ty, tw, th, 3, color);
        }
        if let Some(hit) = lv.tip {
            self.tooltip(hit.rect(lv.thumb), hit.tip(), LOG_W, LOG_H);
        }
        unsafe { GdiFlush() };
    }

    fn draw_log_row(&self, x: i32, y: i32, w: i32, row: &Row) {
        match row {
            Row::Fight(f) => {
                self.fill(x, y + 12, w, 1, CARD_HI);
                let name = boss_name(base_name(&f.name));
                let label = format!("{name}  {}", &fmt_clock(f.start_ts)[..5]);
                let tw = label.chars().count() as i32 * 7 + 16;
                self.fill(x, y, tw, timeline::ROW_H, BG);
                self.text_rect(
                    x + 4,
                    y,
                    tw,
                    timeline::ROW_H,
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
                    LogRow {
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
                LogRow {
                    mark: ACCENT,
                    ts: t.ts,
                    tag: ("aggro", F_TINY, ACCENT),
                    who: (&t.player, TEXT),
                    what: Some(boss_name(base_name(&t.boss))),
                },
            ),
            Row::Death { ts, .. } => self.log_row(
                x,
                y,
                w,
                LogRow {
                    mark: DANGER,
                    ts: *ts,
                    tag: ("death", F_TINY, DANGER),
                    who: ("you died", DANGER),
                    what: None,
                },
            ),
        }
    }

    fn log_row(&self, x: i32, y: i32, w: i32, r: LogRow) {
        let vc = DT_LEFT | DT_VCENTER;
        let h = timeline::ROW_H;
        self.rround(x, y + 5, 3, h - 10, 1, r.mark);
        self.text_rect(x + 12, y, 62, h, F_TINY, DIM, vc, &fmt_clock(r.ts));
        let (tag, font, color) = r.tag;
        self.text_rect(x + 76, y, 44, h, font, color, DT_RIGHT | DT_VCENTER, tag);
        let who_w = if r.what.is_some() { 120 } else { w - 130 };
        self.text_rect(x + 130, y, who_w, h, F_BODY, r.who.1, vc, r.who.0);
        if let Some(what) = r.what {
            self.text_rect(x + 254, y, w - 254, h, F_BODY, DIM, vc, what);
        }
    }
}
