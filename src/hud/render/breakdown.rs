use super::fmt::*;
use super::layout::*;
use super::theme::*;
use super::Renderer;
use crate::game::run::Tally;
use crate::game::source::generic_attacker;
use windows_sys::Win32::Graphics::Gdi::*;

const SEG_COLORS: [u32; 4] = [DANGER, AMBER, ACCENT, DIM];

impl Renderer {
    pub(super) fn breakdown(&self, y0: i32, tallies: &[Tally], total: u64) {
        if tallies.is_empty() {
            self.text_rect(
                M + 12,
                y0,
                W - 24,
                120,
                F_BODY,
                DIM,
                DT_CENTER | DT_VCENTER,
                "untouched so far",
            );
            return;
        }
        self.text(M + 12, y0, W - 24, F_LABEL, DIM, DT_LEFT, "BY ATTACKER");
        self.text(
            M + 12,
            y0 + 50,
            W - 24,
            F_LABEL,
            DIM,
            DT_LEFT,
            "TOP ATTACKS",
        );
        let mut attackers: Vec<(&str, u64)> = Vec::new();
        for t in tallies {
            match attackers.iter_mut().find(|a| a.0 == t.who) {
                Some(a) => a.1 += t.total,
                None => attackers.push((&t.who, t.total)),
            }
        }
        attackers.sort_by_key(|a| std::cmp::Reverse(a.1));
        let total = total.max(1) as f32;
        let bw = W - 24;
        let by = y0 + 16;
        self.rround(M + 12, by, bw, 8, 4, BG);
        let mut x = 0;
        let mut legend = String::new();
        let mut other = 0;
        for (i, (name, amt)) in attackers.iter().enumerate() {
            if i >= 3 {
                other += amt;
                continue;
            }
            let w = ((bw as f32) * (*amt as f32 / total)) as i32;
            if w >= 4 {
                self.rround(M + 12 + x, by, w, 8, 4, SEG_COLORS[i]);
            }
            x += w;
            if !legend.is_empty() {
                legend.push_str("   ");
            }
            legend.push_str(&format!(
                "{name} {}%",
                (*amt as f32 / total * 100.0).round() as u32
            ));
        }
        if other > 0 {
            let w = ((bw as f32) * (other as f32 / total)) as i32;
            if w >= 4 {
                self.rround(M + 12 + x, by, w, 8, 4, SEG_COLORS[3]);
            }
            legend.push_str(&format!(
                "   other {}%",
                (other as f32 / total * 100.0).round() as u32
            ));
        }
        self.text(M + 12, y0 + 28, W - 24, F_TINY, DIM, DT_LEFT, &legend);
        let mut attacks: Vec<&Tally> = tallies.iter().collect();
        attacks.sort_by_key(|a| std::cmp::Reverse(a.total));
        let top = attacks.first().map_or(1, |a| a.total).max(1) as f32;
        for (i, t) in attacks.iter().take(3).enumerate() {
            let y = y0 + 66 + i as i32 * 20;
            let label = if generic_attacker(&t.who) {
                t.attack.clone()
            } else {
                format!("{}  {}", t.who, t.attack)
            };
            self.text_rect(
                M + 12,
                y,
                W - 110,
                17,
                F_BODY,
                TEXT,
                DT_LEFT | DT_VCENTER,
                &label,
            );
            let right = format!("{}  x{}", group_digits(t.total), t.hits);
            self.text_rect(
                M + 12,
                y,
                W - 24,
                17,
                F_TINY,
                DIM,
                DT_RIGHT | DT_VCENTER,
                &right,
            );
            self.bar_on(
                M + 12,
                y + 17,
                W - 24,
                2,
                t.total as f32 / top,
                mix(CARD, DANGER, 0.6),
                BG,
            );
        }
    }
}
