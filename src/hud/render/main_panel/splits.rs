use super::super::Renderer;
use super::super::fmt::{compact, group_digits};
use super::super::frame::Frame;
use super::super::layout::{M, SPLIT_ROW_H, SPLIT_ROWS, SPLITS_CARD, STAGES_BTN, W};
use super::super::theme::{
    ACCENT, AMBER, CARD, CARD_HI, DANGER, DIM, EDGE, F_BODY, F_LABEL, F_TINY, TEXT, mix,
};
use super::{DASH, SPLIT_COLS, fmt_split};
use crate::game::names::stage_name;
use crate::game::run::Split;
use crate::game::state::GameState;
use windows_sys::Win32::Graphics::Gdi::{DT_CENTER, DT_LEFT, DT_VCENTER};

impl Renderer {
    pub(super) fn draw_splits(&self, frame: &Frame, gs: &GameState) {
        let (cx, cy, cw, ch) = SPLITS_CARD;
        self.rround(cx, cy, cw, ch, 8, CARD);
        let Some(ri) = frame.view_run else {
            return;
        };
        let run = &gs.runs[ri];
        let rows = gs.run_splits(ri, frame.now);
        let cur = if frame.live() {
            rows.len().checked_sub(1)
        } else {
            frame
                .view_group
                .and_then(|group| run.pages().into_iter().nth(group))
                .map(|page| crate::num::to_usize(run.stage_of(&page)) - 1)
        };
        let vc = DT_VCENTER;
        self.text_rect(
            (M + 12, STAGES_BTN.1, STAGES_BTN.0 - M - 20, STAGES_BTN.3),
            F_LABEL,
            DIM,
            DT_LEFT | vc,
            "THIS RUN BY STAGE",
        );
        let hy = cy + 26;
        for (x, width, align, head) in SPLIT_COLS {
            let ink = if head == "TAKEN" {
                mix(DANGER, DIM, 0.4)
            } else {
                DIM
            };
            self.text_rect((x, hy, width, 16), F_TINY, ink, align | vc, head);
        }
        let count = rows.len();
        let top = frame.split_top.min(count.saturating_sub(SPLIT_ROWS));
        let shown = top..count.min(top + SPLIT_ROWS);
        let total = rows.iter().fold(Split::default(), |mut sum, row| {
            sum.secs += row.secs;
            sum.dmg += row.dmg;
            sum.taken += row.taken;
            sum.deaths += row.deaths;
            sum
        });
        let fits = |col: usize, pick: fn(&Split) -> u64| {
            rows[shown.clone()].iter().chain([&total]).all(|row| {
                self.text_w(F_BODY, &group_digits(pick(row))) <= self.px(SPLIT_COLS[col].1)
            })
        };
        let full_d = fits(2, |row| row.dmg);
        let full_t = fits(3, |row| row.taken);
        let num = |full: bool, value: u64| {
            if full {
                group_digits(value)
            } else {
                compact(value)
            }
        };
        let head = SPLIT_ROW_H;
        let cell = |y: i32, col: usize, font: usize, ink: u32, text: &str| {
            let (x, width, align, _) = SPLIT_COLS[col];
            self.text_rect((x, y, width, head), font, ink, align | vc, text);
        };
        let line = |y: i32, row: &Split| {
            cell(y, 1, F_BODY, TEXT, &fmt_split(row.secs));
            if row.missed {
                for col in 2..=4 {
                    cell(y, col, F_BODY, DIM, DASH);
                }
                return;
            }
            cell(y, 2, F_BODY, AMBER, &num(full_d, row.dmg));
            cell(y, 3, F_BODY, DANGER, &num(full_t, row.taken));
            let dc = if row.deaths > 0 { DANGER } else { DIM };
            cell(y, 4, F_BODY, dc, &row.deaths.to_string());
        };
        let mut y = hy + 18;
        for index in shown.clone() {
            let row = &rows[index];
            if cur == Some(index) {
                self.rround(M + 6, y, W - 12, head, 5, mix(CARD, ACCENT, 0.18));
            } else if index % 2 == 1 {
                self.rround(M + 6, y, W - 12, head, 5, mix(CARD, CARD_HI, 0.5));
            }
            let name = format!("{}  {}", index + 1, stage_name(&row.stage));
            cell(y, 0, F_BODY, TEXT, &name);
            line(y, row);
            y += head;
        }
        if count > SPLIT_ROWS {
            let more = format!(
                "{} to {} of {count} stages, scroll for the rest",
                shown.start + 1,
                shown.end
            );
            self.text_rect((M + 12, y, W - 24, 20), F_TINY, DIM, DT_CENTER | vc, &more);
            y += 20;
        }
        self.rround(M + 12, y + 3, W - 24, 1, 0, EDGE);
        y += 6;
        cell(y, 0, F_LABEL, DIM, "RUN");
        line(y, &total);
    }
}
