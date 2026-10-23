use super::fmt::*;
use super::layout::*;
use super::theme::*;
use super::Renderer;
use crate::game::run::Tally;
use crate::game::source::generic_attacker;
use windows_sys::Win32::Graphics::Gdi::*;

const ROW_H: i32 = 22;

impl Renderer {
    pub(super) fn breakdown(&self, y0: i32, tallies: &[Tally], note: Option<&str>, empty: &str) {
        self.text(M + 12, y0, W - 24, F_LABEL, DIM, DT_LEFT, "TOP ATTACKS");
        if let Some(note) = note {
            self.text(M + 12, y0, W - 24, F_TINY, DIM, DT_RIGHT, note);
        }
        if tallies.is_empty() {
            self.text_rect(
                M + 12,
                y0 + 18,
                W - 24,
                ROW_H * 3,
                F_BODY,
                DIM,
                DT_CENTER | DT_VCENTER,
                empty,
            );
            return;
        }
        let mut attacks: Vec<&Tally> = tallies.iter().collect();
        attacks.sort_by_key(|a| std::cmp::Reverse(a.total));
        attacks.truncate(3);
        let top = attacks[0].total.max(1) as f32;
        let (dx, dw) = ATTACK_DMG;
        let full = attacks
            .iter()
            .all(|t| self.text_w(F_BODY, &group_digits(t.total)) <= self.px(dw));
        let vc = DT_VCENTER;
        for (i, t) in attacks.iter().enumerate() {
            let y = y0 + 18 + i as i32 * ROW_H;
            let label = if generic_attacker(&t.who) {
                t.attack.clone()
            } else {
                format!("{}  {}", t.who, t.attack)
            };
            let count = format!("x{}", t.hits);
            let (bx, bw) = ATTACK_BAR;
            let nw = bx - 8 - (M + 12);
            let cw = self.unscale(self.text_w(F_TINY, &count)) + 6;
            self.text_rect(
                M + 12,
                y,
                nw - cw,
                ROW_H,
                F_BODY,
                TEXT,
                DT_LEFT | vc,
                &label,
            );
            self.text_rect(M + 12, y, nw, ROW_H, F_TINY, DIM, DT_RIGHT | vc, &count);
            let frac = t.total as f32 / top;
            self.bar_on(
                bx,
                y + ROW_H / 2 - 2,
                bw,
                4,
                frac,
                mix(CARD, DANGER, 0.7),
                BG,
            );
            let num = if full {
                group_digits(t.total)
            } else {
                compact(t.total)
            };
            self.text_rect(dx, y, dw, ROW_H, F_BODY, TEXT, DT_RIGHT | vc, &num);
        }
    }
}
