use super::anim::*;
use super::App;
use crate::game::state::GameState;
use crate::hud::render::{tip_ready, Env, LogHit, LogView};
use crate::hud::timeline::{self, Filter};
use std::time::Instant;

pub(super) const WHEEL_DELTA: i32 = 120;
const LOG_BODY_Y: i32 = crate::hud::render::LOG_BODY.1;

impl App {
    fn log_source(&self) -> Option<timeline::Source<'_>> {
        Self::source(&self.gs, self.sel_run, self.viewed_page(), self.env())
    }

    fn source(
        gs: &GameState,
        sel: Option<usize>,
        page: usize,
        env: Env,
    ) -> Option<timeline::Source<'_>> {
        match sel {
            None => timeline::live(gs).filter(|_| env == Env::InWorld),
            Some(_) => gs.runs.get(page).map(timeline::of_run),
        }
    }

    pub(super) fn log_reset(&mut self) {
        self.log.scroll = 0.0;
        self.log.scroll_shown = 0.0;
        self.log.rows = self.log_rows();
    }

    pub fn log_toggle(&mut self) {
        if self.log.visible {
            self.log_close();
        } else {
            self.log_open();
        }
    }

    pub fn log_open(&mut self) {
        self.log.visible = true;
        self.log.alpha = 0;
        self.log.alpha_shown = 0;
        self.log.opened_at = Some(Instant::now());
    }

    pub fn log_close(&mut self) {
        self.log.visible = false;
        self.log.drag = None;
        self.log.hover = None;
    }

    pub fn log_rows(&self) -> usize {
        timeline::timeline(self.log_source(), self.log.filter).len()
    }

    pub fn log_thumb(&self) -> Option<(i32, i32)> {
        timeline::thumb(
            self.log.rows,
            self.log.renderer.body_h(),
            self.log.scroll_shown,
        )
    }

    pub fn log_set_filter(&mut self, filter: Filter) {
        if self.log.filter != filter {
            self.log.filter = filter;
            self.log_reset();
        }
    }

    pub(super) fn log_clamp(&mut self) {
        let max = timeline::max_scroll(self.log.rows, self.log.renderer.body_h());
        self.log.scroll = self.log.scroll.clamp(0.0, max);
        self.log.scroll_shown = self.log.scroll_shown.clamp(0.0, max);
    }

    pub fn log_wheel(&mut self, delta: i32) {
        self.log.wheel_acc += delta;
        let steps = self.log.wheel_acc / WHEEL_DELTA;
        self.log.wheel_acc -= steps * WHEEL_DELTA;
        if steps != 0 {
            self.log.scroll -= steps as f32 * 3.0 * timeline::ROW_H as f32;
            self.log_clamp();
            self.log.thumb_seen = Some(Instant::now());
        }
    }

    pub fn log_press(&mut self, hit: LogHit, y: i32) {
        match hit {
            LogHit::Title | LogHit::Count => {}
            LogHit::Close => self.log_close(),
            LogHit::Tab(f) => self.log_set_filter(f),
            LogHit::Thumb => self.log.drag = Some((y, self.log.scroll)),
            LogHit::Track => {
                let page = self.log.renderer.body_h() as f32;
                let above = self
                    .log_thumb()
                    .is_some_and(|(ty, _)| self.log.renderer.unscale(y) < LOG_BODY_Y + ty);
                self.log.scroll += if above { -page } else { page };
                self.log_clamp();
                self.log.thumb_seen = Some(Instant::now());
            }
        }
    }

    pub fn log_drag_to(&mut self, y: i32) {
        if let Some((y0, s0)) = self.log.drag {
            let dy = self.log.renderer.unscale(y - y0);
            self.log.scroll =
                timeline::drag_scroll(self.log.rows, self.log.renderer.body_h(), s0, dy);
            self.log.scroll_shown = self.log.scroll;
            self.log.thumb_seen = Some(Instant::now());
        }
    }

    pub fn log_set_hover(&mut self, over: Option<LogHit>) -> bool {
        if over == self.log.hover {
            return false;
        }
        self.log.hover = over;
        self.log.hover_at = over.map(|_| Instant::now());
        true
    }

    pub fn log_tip(&self) -> Option<LogHit> {
        self.log.hover.filter(|_| tip_ready(self.log.hover_at))
    }

    pub fn log_release(&mut self) {
        self.log.drag = None;
    }

    pub fn render_log(&mut self) {
        let slide_t = timed(self.log.slide_at, SLIDE_MS);
        let view = LogView {
            scroll: self.log.scroll_shown,
            filter: self.log.filter,
            hover: self.log.hover,
            tip: self.log_tip(),
            thumb: self.log_thumb(),
            dragging: self.log.drag.is_some(),
            thumb_t: self.log.thumb_t,
            slide: -(1.0 - crate::hud::render::ease_out_cubic(slide_t)) * timeline::ROW_H as f32,
        };
        let (sel, page, env) = (self.sel_run, self.viewed_page(), self.env());
        let src = Self::source(&self.gs, sel, page, env);
        self.log.renderer.draw_log(src, &view);
    }
}
