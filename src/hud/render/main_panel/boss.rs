use super::super::Renderer;
use super::super::enemies::{ClearingIn, enemies_shown};
use super::super::fmt::{fmt_count, fmt_dur};
use super::super::frame::Frame;
use super::super::graph::GRAPH_COLS;
use super::super::hit::Hit;
use super::super::layout::{FIGHT_NEXT_HIT, FIGHT_PREV_HIT, GRAPH_RECT, M, MUTE_HIT, W};
use super::super::theme::{
    ACCENT, AMBER, CARD, DANGER, DIM, F_BIG, F_BOSS, F_LABEL, F_TINY, GLYPH_MUTE, GLYPH_NEXT,
    GLYPH_PREV, GLYPH_VOLUME, GOOD, TEXT, ease_out_cubic, mix,
};
use super::{DASH, Live, MISSED, STRIP_GAP, STRIP_H, STRIP_X, STRIP_Y, live_kind, strip_layout};
use crate::game::names::{boss_name, stage_name};
use crate::game::run::{FightGroup, Page, Run, RunEnd, base_name, phase_num};
use crate::game::state::{GameState, Mode};
use crate::hud::flags::Flag;
use std::borrow::Cow;
use windows_sys::Win32::Graphics::Gdi::{DT_CENTER, DT_LEFT, DT_RIGHT, DT_VCENTER};

impl Renderer {
    pub(super) fn draw_boss_card(&self, f: &Frame, gs: &GameState, hist: Option<&FightGroup>) {
        let now = f.now;
        let mine = f.live() && !f.empty() && gs.boss.is_some() && gs.targets_me();
        let tx = self.boss_card_head(f, gs, hist, mine);
        if !f.live() {
            return self.draw_boss_history(f, f.view_run.map(|i| &gs.runs[i]), hist, tx);
        }
        match (gs.boss.as_ref().filter(|_| !f.empty()), &gs.target) {
            (Some(boss), _) => self.boss_fight(f, gs, boss, tx, mine),
            (None, _) if gs.pre_boss() && !f.empty() => {
                self.text(
                    (tx, 88, M + 226 - tx),
                    F_BOSS,
                    TEXT,
                    DT_LEFT,
                    stage_name(&gs.stage),
                );
                self.draw_clearing(&ClearingIn {
                    secs: gs.stage_secs(now),
                    tokens: gs.tokens_shown(),
                    enemies: enemies_shown(gs, now),
                });
            }
            (None, _) if gs.mode == Mode::Intermission && !f.empty() => {
                self.boss_intermission(gs, now);
            }
            (None, _) => {
                self.text(
                    (M + 12, 112, W - 24),
                    F_BOSS,
                    DIM,
                    DT_LEFT,
                    "no boss active",
                );
            }
        }
    }

    fn boss_card_head(
        &self,
        f: &Frame,
        gs: &GameState,
        hist: Option<&FightGroup>,
        mine: bool,
    ) -> i32 {
        let has_fights = f.view_run.is_some_and(|i| !gs.runs[i].pages().is_empty());
        let stage_page = !f.live() && hist.is_some_and(crate::game::run::FightGroup::is_stage);
        let viewed = f.view_run.zip(f.view_group).and_then(|(r, g)| {
            let run = &gs.runs[r];
            Some(run.stage_of(&run.pages().into_iter().nth(g)?))
        });
        let live_no = gs.stage_no.filter(|_| f.live());
        let (card_label, kind, num) = if f.live() && !f.empty() && gs.pre_boss() {
            ("STAGE", ACCENT, live_no)
        } else if f.live() && !f.empty() && gs.mode == Mode::Intermission {
            ("RUN", DIM, None)
        } else if stage_page || (!f.live() && !has_fights) {
            ("STAGE", ACCENT, viewed)
        } else if f.live() && (f.empty() || gs.boss.is_none()) {
            ("BOSS", DIM, None)
        } else {
            ("BOSS", DANGER, live_no.or(viewed))
        };
        let card = if mine {
            mix(CARD, DANGER, 0.16)
        } else if kind != DIM {
            mix(CARD, kind, 0.08)
        } else {
            CARD
        };
        self.rround(M, 82, W, 96, 8, card);
        let tx = if kind == DIM {
            self.text((M + 12, 88, W - 24), F_LABEL, DIM, DT_LEFT, card_label);
            M + 60
        } else {
            let label = match num {
                Some(n) => format!("{card_label} {n}"),
                None => card_label.to_string(),
            };
            let pw = self.unscale(self.text_w(F_LABEL, &label)) + 12;
            self.rround(M + 10, 87, pw, 18, 4, mix(CARD, kind, 0.7));
            self.text_rect(
                (M + 10, 87, pw, 18),
                F_LABEL,
                TEXT,
                DT_CENTER | DT_VCENTER,
                &label,
            );
            (M + 10 + pw + 8).max(M + 60)
        };
        if has_fights
            && !f.live()
            && let (Some(r), Some(g)) = (f.view_run, f.view_group)
        {
            self.kind_strip(&gs.runs[r], g);
        }
        if has_fights {
            let free = !f.flags.has(Flag::PromptOpen) && !f.flags.has(Flag::SettingsOpen);
            let back = f.view_group.map_or(f.group_pages > 1, |i| i > 0);
            self.glyph_button(
                FIGHT_PREV_HIT,
                f.ctl(Hit::FightPrev).usable(free && back),
                GLYPH_PREV,
            );
            self.glyph_button(
                FIGHT_NEXT_HIT,
                f.ctl(Hit::FightNext)
                    .usable(free && f.flags.has(Flag::GroupSelected)),
                GLYPH_NEXT,
            );
            let idx = format!(
                "{}/{}",
                f.view_group.map_or(f.group_pages, |i| i + 1),
                f.group_pages
            );
            self.text_rect((266, 86, 42, 20), F_TINY, DIM, DT_CENTER | DT_VCENTER, &idx);
        }
        tx
    }

    fn boss_fight(&self, f: &Frame, gs: &GameState, boss: &str, tx: i32, mine: bool) {
        let now = f.now;
        let target = &gs.target;
        let shown = boss_name(base_name(boss));
        self.text((tx, 88, M + 226 - tx), F_BOSS, TEXT, DT_LEFT, &shown);
        if mine {
            self.text(
                (M + 12, 114, W - 24),
                F_LABEL,
                DANGER,
                DT_LEFT,
                "TARGETING YOU",
            );
        } else {
            self.text((M + 12, 114, W - 24), F_LABEL, DIM, DT_LEFT, "TARGET");
        }
        let glyph = if f.flags.has(Flag::Sound) {
            GLYPH_VOLUME
        } else {
            GLYPH_MUTE
        };
        let free = !f.flags.has(Flag::PromptOpen) && !f.flags.has(Flag::SettingsOpen);
        self.glyph_button(MUTE_HIT, f.ctl(Hit::Mute).usable(free), glyph);
        let phase = phase_num(boss);
        if phase > 1 {
            let note = format!("phase {phase}");
            let right = MUTE_HIT.0 - 8 - (M + 12);
            self.text((M + 12, 114, right), F_TINY, DIM, DT_RIGHT, &note);
        }
        let color = if mine {
            mix(TEXT, DANGER, ease_out_cubic(f.flash_t))
        } else {
            mix(ACCENT, TEXT, ease_out_cubic(f.flash_t))
        };
        match target {
            Some(t) => {
                self.text((M + 12, 128, W - 24), F_BIG, color, DT_LEFT, t);
                let held = now.saturating_sub(gs.target_since);
                self.text_rect(
                    (M + 12, 128, W - 24, 34),
                    F_TINY,
                    DIM,
                    DT_RIGHT | DT_VCENTER,
                    &format!("{held}s"),
                );
            }
            None => self.text((M + 12, 128, W - 24), F_BOSS, DIM, DT_LEFT, DASH),
        }
    }

    fn boss_intermission(&self, gs: &GameState, now: u64) {
        let run = gs.live_run();
        let head = match run.and_then(|r| r.fights.last()) {
            Some(fight) => format!(
                "{} {}",
                boss_name(base_name(&fight.name)),
                if fight.lost {
                    "lost"
                } else if fight.kill.is_some() {
                    "killed"
                } else {
                    "unfinished"
                }
            ),
            None => "no boss yet".to_string(),
        };
        self.text((M + 60, 88, 166), F_BOSS, TEXT, DT_LEFT, &head);
        self.text((M + 12, 114, W - 24), F_LABEL, DIM, DT_LEFT, "INTERMISSION");
        let line = match run {
            Some(r) => format!(
                "{} down   {} in the run",
                fmt_count(r.kills(), "boss", "bosses"),
                fmt_dur(now.saturating_sub(r.start_ts))
            ),
            None => DASH.to_string(),
        };
        self.text((M + 12, 128, W - 24), F_BOSS, TEXT, DT_LEFT, &line);
    }

    fn kind_strip(&self, run: &Run, cur: usize) {
        let pages = run.pages();
        let (shown, sw) = strip_layout(pages.len(), cur);
        for (k, i) in shown.enumerate() {
            let p = &pages[i];
            let stage = matches!(p, Page::Stage(_));
            let tone = if stage { ACCENT } else { DANGER };
            let on = i == cur;
            let x = STRIP_X + crate::num::to_i32(k) * (sw + STRIP_GAP);
            let fill = mix(CARD, tone, if on { 0.75 } else { 0.22 });
            self.rround(x, STRIP_Y, sw, STRIP_H, 4, fill);
            let t = format!("{}{}", if stage { "S" } else { "B" }, run.stage_of(p));
            if self.unscale(self.text_w(F_TINY, &t)) + 4 <= sw {
                let ink = if on { TEXT } else { DIM };
                self.text_rect(
                    (x, STRIP_Y, sw, STRIP_H),
                    F_TINY,
                    ink,
                    DT_CENTER | DT_VCENTER,
                    &t,
                );
            }
        }
    }

    fn draw_boss_history(&self, f: &Frame, run: Option<&Run>, hist: Option<&FightGroup>, tx: i32) {
        let Some(h) = hist else {
            let Some(r) = run else {
                return self.text(
                    (M + 12, 112, W - 24),
                    F_BOSS,
                    DIM,
                    DT_LEFT,
                    "no boss fights",
                );
            };
            self.text(
                (tx, 88, M + 226 - tx),
                F_BOSS,
                TEXT,
                DT_LEFT,
                stage_name(&r.stage),
            );
            self.text((M + 12, 114, W - 24), F_LABEL, DIM, DT_LEFT, "RESULT");
            let res = match r.end {
                Some(RunEnd::Left) => "left before a boss",
                Some(_) => "lobby before a boss",
                None => "no boss yet",
            };
            self.text((M + 12, 128, W - 24), F_BOSS, DIM, DT_LEFT, res);
            let dur = fmt_dur(r.end_ts.unwrap_or(f.now).saturating_sub(r.start_ts));
            return self.text_rect(
                (M + 12, 128, W - 24, 22),
                F_TINY,
                DIM,
                DT_RIGHT | DT_VCENTER,
                &dur,
            );
        };
        self.text(
            (tx, 88, M + 226 - tx),
            F_BOSS,
            TEXT,
            DT_LEFT,
            &if h.is_stage() {
                Cow::Borrowed(stage_name(h.name))
            } else {
                boss_name(base_name(h.name))
            },
        );
        self.text((M + 12, 114, W - 24), F_LABEL, DIM, DT_LEFT, "RESULT");
        if h.n_phases > 1 {
            let note = format!("{} phases", h.n_phases);
            self.text((M + 12, 114, W - 24), F_TINY, DIM, DT_RIGHT, &note);
        }
        let res = h.result();
        let color = match res {
            "lost" => DANGER,
            "killed" | "cleared" => GOOD,
            "unfinished" => DIM,
            _ => AMBER,
        };
        self.text((M + 12, 128, W - 24), F_BOSS, color, DT_LEFT, res);
        let deaths: u32 = h.fights.iter().map(|p| p.deaths).sum();
        let mut right = fmt_dur(h.duration(f.now));
        if deaths > 0 {
            right = format!("died {deaths}x   {right}");
        }
        self.text_rect(
            (M + 12, 128, W - 24, 22),
            F_TINY,
            DIM,
            DT_RIGHT | DT_VCENTER,
            &right,
        );
    }

    pub(super) fn card_graph(
        &self,
        f: &Frame,
        gs: &GameState,
        hist: Option<&FightGroup>,
        gy: i32,
        taken: bool,
    ) {
        let live_group = match live_kind(f, gs) {
            Live::Fight | Live::Inter(_) => gs
                .live_run()
                .and_then(|r| r.group(r.groups().len().checked_sub(1)?)),
            _ => None,
        };
        let shown = if f.live() { live_group.as_ref() } else { hist };
        let (gx, _, gw, gh) = GRAPH_RECT;
        match shown {
            Some(g) if g.missed => {
                self.graph_well(gx, gy, gw, gh);
                self.text_rect(
                    (gx, gy, gw, gh),
                    F_TINY,
                    DIM,
                    DT_CENTER | DT_VCENTER,
                    MISSED,
                );
            }
            Some(g) if taken => {
                self.graph(gx, gy, gw, gh, &g.taken_graph(f.now, GRAPH_COLS), DANGER);
            }
            Some(g) => self.graph(gx, gy, gw, gh, &g.graph(f.now, GRAPH_COLS), AMBER),
            None if f.live() => {
                self.graph_well(gx, gy, gw, gh);
                self.text_rect(
                    (gx, gy, gw, gh),
                    F_TINY,
                    DIM,
                    DT_CENTER | DT_VCENTER,
                    "graph starts with the boss",
                );
            }
            None => {}
        }
    }
}
