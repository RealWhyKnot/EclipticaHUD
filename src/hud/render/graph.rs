use super::Renderer;
use super::theme::{BG, CARD, DIM, mix};
use crate::game::run::Graph;

pub(super) const GRAPH_COLS: usize = 77;

impl Renderer {
    pub(super) fn graph_well(&self, x: i32, y: i32, w: i32, h: i32) {
        self.rround(x - 2, y - 2, w + 4, h + 4, 4, mix(CARD, BG, 0.55));
    }

    pub(super) fn graph(&self, x: i32, y: i32, width: i32, height: i32, graph: &Graph, color: u32) {
        self.graph_well(x, y, width, height);
        let count = graph.cols.len();
        let peak = graph.cols.iter().copied().max().unwrap_or(0);
        if count == 0 || peak == 0 {
            return;
        }
        let body = mix(CARD, color, 0.45);
        for (index, &value) in graph.cols.iter().enumerate() {
            let x0 = x + crate::num::to_i32(index) * width / crate::num::to_i32(count);
            let x1 = x + (crate::num::to_i32(index) + 1) * width / crate::num::to_i32(count);
            let cw = if x1 - x0 >= 3 { x1 - x0 - 1 } else { x1 - x0 };
            let ch =
                crate::num::to_i32((value * crate::num::to_u64(height)).div_ceil(peak)).min(height);
            if ch > 0 {
                self.fill(x0, y + height - ch, cw, ch, body);
                self.fill(x0, y + height - ch, cw, 1, color);
            }
        }
        for &mark in &graph.marks {
            let mx = x + crate::num::to_i32(mark) * width / crate::num::to_i32(count);
            self.fill(mx, y, 1, height, mix(CARD, DIM, 0.6));
        }
    }
}
