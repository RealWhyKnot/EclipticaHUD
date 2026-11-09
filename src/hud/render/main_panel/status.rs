use super::super::Renderer;
use super::super::fmt::fmt_run_deaths;
use super::super::frame::{Env, Frame};
use super::super::hit::Hit;
use super::super::layout::{LOGICAL_W, M, RUN_NEXT_HIT, RUN_PREV_HIT, W};
use super::super::theme::{
    ACCENT, AMBER, BG, DANGER, DIM, F_BODY, F_LABEL, F_TINY, GLYPH_NEXT, GLYPH_PREV, GOOD, TEXT,
    mix,
};
use super::{STATUS_GAP, sync_tag};
use crate::game::event::fmt_clock;
use crate::game::names::{phase_name, stage_name};
use crate::game::run::RunEnd;
use crate::game::state::{GameState, Life, Mode};
use crate::hud::flags::Flag;
use std::fmt::Write as _;
use windows_sys::Win32::Graphics::Gdi::{DT_CENTER, DT_LEFT, DT_RIGHT, DT_VCENTER};

impl Renderer {
    pub(super) fn draw_status(&self, f: &Frame, gs: &GameState) {
        let now = f.now;
        let in_world = f.env == Env::InWorld;
        let (status, status_color) = match (f.env, &gs.mode) {
            (Env::NoVrchat, _) => ("VRChat is not running".to_string(), DIM),
            (Env::NotInWorld, _) => ("waiting for you to join Ecliptica".to_string(), DIM),
            (_, Mode::Idle) if gs.boss.is_some() => ("in a boss fight".to_string(), GOOD),
            (_, Mode::Idle) => ("in Ecliptica, waiting for a run".to_string(), DIM),
            (_, Mode::Lobby) => ("in lobby".to_string(), AMBER),
            (_, Mode::Intermission) => ("intermission".to_string(), ACCENT),
            (_, Mode::Stage) => (
                format!(
                    "{}   {}   {}",
                    stage_name(&gs.stage),
                    phase_name(gs.progress),
                    gs.class
                ),
                GOOD,
            ),
        };
        let life = gs.life(now);
        let mut right = 0;
        if !in_world {
        } else if let Some((tone, label)) = super::super::party_panel::dead_look(life) {
            let color = mix(mix(BG, tone, 0.45), tone, f.dead_pulse);
            self.text((M, 28, W), F_BODY, color, DT_RIGHT, label);
            right = self.unscale(self.text_w(F_BODY, label));
        } else {
            if let Some((got, total)) = gs.tokens_shown() {
                let rune = gs.level_tokens.iter().any(|t| t.0);
                let txt = format!(
                    "{got}/{total} token{}{}",
                    if total == 1 { "" } else { "s" },
                    if rune { " +rune" } else { "" }
                );
                let color = if got == total {
                    GOOD
                } else if rune {
                    AMBER
                } else {
                    DIM
                };
                self.text_rect((M, 28, W, 18), F_TINY, color, DT_RIGHT | DT_VCENTER, &txt);
                right = self.unscale(self.text_w(F_TINY, &txt)) + STATUS_GAP;
            }
            if life == Life::OneLeft {
                let label = "LAST LIFE";
                self.text_rect(
                    (M, 28, W - right, 18),
                    F_TINY,
                    AMBER,
                    DT_RIGHT | DT_VCENTER,
                    label,
                );
                right += self.unscale(self.text_w(F_TINY, label)) + STATUS_GAP;
            }
            right = right.saturating_sub(STATUS_GAP);
        }
        let room = if right > 0 { W - right - STATUS_GAP } else { W };
        self.text_rect((M, 28, room, 40), F_BODY, status_color, DT_LEFT, &status);
        if in_world && gs.mode == Mode::Stage {
            self.bar(M, 47, W, 4, f.progress_shown, GOOD);
        }
    }

    pub(super) fn draw_run_pager(&self, f: &Frame, gs: &GameState) {
        let free = !f.flags.has(Flag::PromptOpen);
        self.glyph_button(
            RUN_PREV_HIT,
            f.ctl(Hit::RunPrev).usable(free && f.view_page > 0),
            GLYPH_PREV,
        );
        self.glyph_button(
            RUN_NEXT_HIT,
            f.ctl(Hit::RunNext)
                .usable(free && f.flags.has(Flag::RunSelected)),
            GLYPH_NEXT,
        );
        let tag = sync_tag(f);
        let (run_label, run_color) = match f.view_run {
            None if tag.is_some() => (format!("RUN {}/{}", f.view_page + 1, f.pages), DIM),
            None => (
                format!("RUN {}/{}   not started", f.view_page + 1, f.pages),
                DIM,
            ),
            Some(_) if f.live() => {
                let mut s = format!("RUN {}/{}", f.view_page + 1, f.pages);
                if let Some(n) = gs.stage_no {
                    let _ = write!(s, "   stage {n}");
                }
                (s, DIM)
            }
            Some(i) => {
                let r = &gs.runs[i];
                let (tag, color) = match r.end {
                    Some(RunEnd::Won) => ("WON", GOOD),
                    Some(RunEnd::Lost) => ("LOST", DANGER),
                    Some(RunEnd::Lobby) => ("ENDED", TEXT),
                    Some(RunEnd::Left) => ("LEFT", TEXT),
                    None => ("RUN", TEXT),
                };
                let mut s = format!(
                    "{} {}/{}   {}   {}",
                    tag,
                    i + 1,
                    f.pages,
                    &fmt_clock(r.start_ts)[..5],
                    stage_name(&r.stage)
                );
                if let Some(n) = r.stage_no {
                    let _ = write!(s, "   stage {n}");
                }
                if r.deaths > 0 {
                    let _ = write!(s, "   {}", fmt_run_deaths(r.deaths));
                }
                (s, color)
            }
        };
        let tag = tag.filter(|_| f.view_run.is_none() || f.live());
        if let Some((tag, color)) = tag {
            let gap = "   ";
            let base = self.unscale(self.text_w(F_LABEL, &format!("{run_label}{gap}")));
            let full = base + self.unscale(self.text_w(F_LABEL, tag));
            let x = 44 + ((LOGICAL_W - 88 - full) / 2).max(0);
            self.text_rect(
                (x, 54, base, 24),
                F_LABEL,
                run_color,
                DT_LEFT | DT_VCENTER,
                &run_label,
            );
            self.text_rect(
                (x + base, 54, LOGICAL_W - 44 - x - base, 24),
                F_LABEL,
                color,
                DT_LEFT | DT_VCENTER,
                tag,
            );
            return;
        }
        self.text_rect(
            (44, 54, LOGICAL_W - 88, 24),
            F_LABEL,
            run_color,
            DT_CENTER | DT_VCENTER,
            &run_label,
        );
    }
}
