use super::theme::*;
use super::Renderer;
use crate::game::run::Graph;

pub const GRAPH_COLS: usize = 77;

impl Renderer {
    pub(super) fn graph_well(&self, x: i32, y: i32, w: i32, h: i32) {
        self.rround(x - 2, y - 2, w + 4, h + 4, 4, mix(CARD, BG, 0.55));
    }

    pub(super) fn graph(&self, x: i32, y: i32, w: i32, h: i32, g: &Graph) {
        self.graph_well(x, y, w, h);
        let n = g.cols.len();
        let peak = g.cols.iter().copied().max().unwrap_or(0);
        if n == 0 || peak == 0 {
            return;
        }
        let body = mix(CARD, AMBER, 0.45);
        for (i, &v) in g.cols.iter().enumerate() {
            let x0 = x + i as i32 * w / n as i32;
            let x1 = x + (i as i32 + 1) * w / n as i32;
            let cw = if x1 - x0 >= 3 { x1 - x0 - 1 } else { x1 - x0 };
            let ch = ((v * h as u64).div_ceil(peak) as i32).min(h);
            if ch > 0 {
                self.fill(x0, y + h - ch, cw, ch, body);
                self.fill(x0, y + h - ch, cw, 1, AMBER);
            }
        }
        for &m in &g.marks {
            let mx = x + m as i32 * w / n as i32;
            self.fill(mx, y, 1, h, mix(CARD, DIM, 0.6));
        }
    }
}
