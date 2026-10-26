use super::enemies::{enemies_shown, ClearingIn};
use super::fmt::*;
use super::frame::{Env, Frame};
use super::graph::GRAPH_COLS;
use super::hit::{Hit, TipCtx, DELETE_ARMED, DELETE_BUSY};
use super::layout::*;
use super::theme::*;
use super::Renderer;
use crate::discord::Link;
use crate::game::event::fmt_clock;
use crate::game::names::{boss_name, phase_name, stage_name};
use crate::game::run::{
    base_name, merge_tallies, phase_num, FightGroup, Page, Run, RunEnd, Split, Tally,
};
use crate::game::source::describe_source;
use crate::game::state::{GameState, Life, Mode};
use crate::update::{Badge, VERSION};
use crate::vr::VrStatus;
use std::borrow::Cow;
use windows_sys::Win32::Graphics::Gdi::*;

const DASH: &str = "-";
pub(super) const COL: i32 = (W - 24) / 3;

enum Live<'a> {
    Pre,
    Inter(&'a Run),
    Fight,
    Idle,
}

fn live_kind<'a>(f: &Frame, gs: &'a GameState) -> Live<'a> {
    if !f.live() || f.empty() {
        return Live::Idle;
    }
    if gs.pre_boss() {
        Live::Pre
    } else if let (Mode::Intermission, Some(r)) = (&gs.mode, gs.live_run()) {
        Live::Inter(r)
    } else if gs.boss.is_some() {
        Live::Fight
    } else {
        Live::Idle
    }
}

fn or_dash(f: &Frame, s: String) -> String {
    if f.empty() {
        DASH.to_string()
    } else {
        s
    }
}

fn or_none(n: u64) -> String {
    if n > 0 {
        n.to_string()
    } else {
        DASH.to_string()
    }
}

fn avg_hit(taken: u64, hits: u32) -> String {
    if hits > 0 {
        (taken / hits as u64).to_string()
    } else {
        DASH.to_string()
    }
}

struct LiveTaken {
    l0: String,
    v0: String,
    l1: &'static str,
    v1: String,
    l2: &'static str,
    v2: String,
    hits: String,
    big: String,
    avg: String,
    attacks: Vec<Tally>,
    empty: &'static str,
}

const STATUS_GAP: i32 = 12;

impl LiveTaken {
    fn of(f: &Frame, gs: &GameState) -> Self {
        let now = f.now;
        let win = format!("DTPS {}s", f.window);
        let s = &gs.stage_stats;
        match live_kind(f, gs) {
            Live::Pre => LiveTaken {
                l0: win,
                v0: or_dash(f, fmt_anim(f.taken_shown)),
                l1: "STAGE DTPS",
                v1: group_digits(gs.stage_taken_rate(now)),
                l2: "STAGE TAKEN",
                v2: group_digits(s.taken),
                hits: s.hits.to_string(),
                big: s.max_hit.to_string(),
                avg: avg_hit(s.taken, s.hits),
                attacks: s.attacks.clone(),
                empty: "nothing has hit you this stage",
            },
            Live::Inter(r) => {
                let active = r.active_secs(now).max(1);
                LiveTaken {
                    l0: "RUN DTPS".to_string(),
                    v0: group_digits(r.taken() / active),
                    l1: "RUN TAKEN",
                    v1: group_digits(r.taken()),
                    l2: "DEATHS",
                    v2: r.deaths.to_string(),
                    hits: r.hit_count().to_string(),
                    big: r.max_hit().to_string(),
                    avg: avg_hit(r.taken(), r.hit_count()),
                    attacks: r.attacks(),
                    empty: "nothing has hit you this run",
                }
            }
            Live::Fight => LiveTaken {
                l0: win,
                v0: fmt_anim(f.taken_shown),
                l1: "FIGHT DTPS",
                v1: fmt_anim(f.taken_rate_shown),
                l2: "FIGHT TAKEN",
                v2: group_digits(gs.fight_taken),
                hits: gs.fight_hits.to_string(),
                big: gs.fight_max_hit.to_string(),
                avg: avg_hit(gs.fight_taken, gs.fight_hits),
                attacks: gs.fight_attacks.clone(),
                empty: "nothing has hit you this fight",
            },
            Live::Idle => LiveTaken {
                l0: win,
                v0: or_dash(f, fmt_anim(f.taken_shown)),
                l1: "FIGHT DTPS",
                v1: DASH.to_string(),
                l2: "FIGHT TAKEN",
                v2: DASH.to_string(),
                hits: DASH.to_string(),
                big: DASH.to_string(),
                avg: DASH.to_string(),
                attacks: Vec::new(),
                empty: "attack breakdown starts with the next boss",
            },
        }
    }
}

pub(super) const SPLIT_COLS: [(i32, i32, u32, &str); 5] = [
    (M + 12, 118, DT_LEFT, "STAGE"),
    (M + 132, 44, DT_RIGHT, "TIME"),
    (M + 178, 60, DT_RIGHT, "DEALT"),
    (M + 240, 48, DT_RIGHT, "TAKEN"),
    (M + 288, 32, DT_RIGHT, "DIED"),
];

pub(super) fn fmt_split(secs: u64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

fn fmt_deaths(n: u32) -> String {
    format!("{n} death{}", if n == 1 { "" } else { "s" })
}

pub(super) const STRIP_X: i32 = M + 12;
pub(super) const STRIP_Y: i32 = 156;
pub(super) const STRIP_H: i32 = 16;
pub(super) const STRIP_W: i32 = W - 24;
const STRIP_GAP: i32 = 4;
pub(super) const STRIP_MIN: i32 = 26;

pub(super) fn strip_layout(n: usize, cur: usize) -> (std::ops::Range<usize>, i32) {
    let fit = ((STRIP_W + STRIP_GAP) / (STRIP_MIN + STRIP_GAP)) as usize;
    let shown = n.min(fit).max(1);
    let start = cur.saturating_sub(shown / 2).min(n.saturating_sub(shown));
    let k = shown as i32;
    let sw = ((STRIP_W - STRIP_GAP * (k - 1)) / k).min(40);
    (start..(start + shown).min(n), sw)
}

impl Renderer {
    pub fn draw_main(&mut self, gs: &GameState, f: &Frame) {
        let hist = f
            .view_run
            .zip(f.view_group)
            .and_then(|(r, g)| gs.runs[r].page(g));
        self.fill(0, 0, LOGICAL_W, LOGICAL_H, BG);
        self.draw_titlebar(f);
        self.shifted(BODY_DY, || {
            self.draw_status(f, gs);
            self.draw_run_pager(f, gs);
            self.draw_boss_card(f, gs, hist.as_ref());
            if f.stages {
                self.draw_splits(f, gs);
            } else {
                self.draw_dealt_card(f, gs, hist.as_ref());
            }
            if f.stages_btn && !f.settings_open {
                let ctl = f.ctl(Hit::Stages, true, f.stages);
                self.glyph_button(STAGES_BTN, ctl, GLYPH_TABLE);
            }
        });
        if !f.stages {
            self.shifted(GRAPH_DY + BODY_DY, || {
                self.draw_taken_card(f, gs, hist.as_ref())
            });
        }
        if f.settings_open {
            self.shifted(BODY_DY, || self.draw_settings(f));
        }
        self.draw_footer(f);
        if f.prompt {
            self.draw_update_prompt(f);
        } else if f.ask_usage {
            self.draw_usage_prompt(f);
        }
        if f.warn_t < 1.0 {
            self.draw_token_warning(f, gs);
        }
        if let Some(hit) = f.tip {
            let ctx = TipCtx {
                live: f.live(),
                topmost: f.topmost,
                sound_on: f.sound_on,
                log_open: f.log_open,
                discord_on: f.discord_on,
                usage_on: f.usage_on,
                usage_failing: f.usage_failing,
                board_on: f.board_on,
                board_status: f.board_status,
                board_delete: f.board_delete,
                party_open: f.party_open,
                stages: f.stages,
            };
            self.tooltip(hit.rect(), hit.tip(ctx), LOGICAL_W, LOGICAL_H);
        }
        unsafe { GdiFlush() };
    }

    fn stat(&self, y: i32, i: i32, label: &str, value: &str, color: u32) {
        let x = M + 12 + i * COL;
        self.text(x, y, COL, F_LABEL, DIM, DT_LEFT, label);
        self.text(x, y + 16, COL, F_BOSS, color, DT_LEFT, value);
    }

    fn small_stat(&self, y: i32, i: i32, label: &str, value: &str) {
        let x = M + 12 + i * COL;
        self.text(x, y, COL, F_LABEL, DIM, DT_LEFT, label);
        self.text(x, y + 15, COL, F_BODY, TEXT, DT_LEFT, value);
    }

    fn draw_titlebar(&self, f: &Frame) {
        self.text_rect(
            M,
            8,
            W,
            24,
            F_LABEL,
            ACCENT,
            DT_LEFT | DT_VCENTER,
            "ECLIPTICA HUD",
        );
        let free = !f.prompt;
        self.glyph_button(
            PARTY_BTN,
            f.ctl(Hit::Party, true, f.party_open),
            GLYPH_PARTY,
        );
        self.glyph_button(
            SETTINGS_BTN,
            f.ctl(Hit::Settings, free, f.settings_open),
            GLYPH_SETTINGS,
        );
        self.glyph_button(
            PIN_BTN,
            f.ctl(Hit::Pin, true, false),
            if f.topmost { GLYPH_PIN } else { GLYPH_UNPIN },
        );
        self.glyph_button(LOG_BTN, f.ctl(Hit::Log, true, f.log_open), GLYPH_LOG);
        self.glyph_button(CLOSE_BTN, f.ctl(Hit::Close, true, false), GLYPH_CLOSE);
    }

    fn draw_status(&self, f: &Frame, gs: &GameState) {
        let now = f.now;
        let in_world = f.env == Env::InWorld;
        let (status, status_color) = match (f.env, &gs.mode) {
            (Env::NoVrchat, _) => ("VRChat is not running".to_string(), DIM),
            (Env::NotInWorld, _) => ("waiting for you to join Ecliptica".to_string(), DIM),
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
        } else if let Some((tone, label)) = super::party_panel::dead_look(life) {
            let color = mix(mix(BG, tone, 0.45), tone, f.dead_pulse);
            self.text(M, 28, W, F_BODY, color, DT_RIGHT, label);
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
                self.text_rect(M, 28, W, 18, F_TINY, color, DT_RIGHT | DT_VCENTER, &txt);
                right = self.unscale(self.text_w(F_TINY, &txt)) + STATUS_GAP;
            }
            if life == Life::OneLeft {
                let label = "LAST LIFE";
                self.text_rect(
                    M,
                    28,
                    W - right,
                    18,
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
        self.text_rect(M, 28, room, 40, F_BODY, status_color, DT_LEFT, &status);
        if in_world && gs.mode == Mode::Stage {
            self.bar(M, 47, W, 4, f.progress_shown, GOOD);
        }
    }

    fn draw_run_pager(&self, f: &Frame, gs: &GameState) {
        let free = !f.prompt;
        self.glyph_button(
            RUN_PREV_HIT,
            f.ctl(Hit::RunPrev, free && f.view_page > 0, false),
            GLYPH_PREV,
        );
        self.glyph_button(
            RUN_NEXT_HIT,
            f.ctl(Hit::RunNext, free && f.run_sel, false),
            GLYPH_NEXT,
        );
        let (run_label, run_color) = match f.view_run {
            None => (
                format!("RUN {}/{}   not started", f.view_page + 1, f.pages),
                DIM,
            ),
            Some(_) if f.live() => {
                let mut s = format!("RUN {}/{}", f.view_page + 1, f.pages);
                if let Some(n) = gs.stage_no {
                    s.push_str(&format!("   stage {n}"));
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
                    s.push_str(&format!("   stage {n}"));
                }
                if r.deaths > 0 {
                    s.push_str(&format!("   {}", fmt_run_deaths(r.deaths)));
                }
                (s, color)
            }
        };
        self.text_rect(
            44,
            54,
            LOGICAL_W - 88,
            24,
            F_LABEL,
            run_color,
            DT_CENTER | DT_VCENTER,
            &run_label,
        );
    }

    fn draw_boss_card(&self, f: &Frame, gs: &GameState, hist: Option<&FightGroup>) {
        let now = f.now;
        let mine = f.live() && !f.empty() && gs.boss.is_some() && gs.targets_me();
        let has_fights = f.view_run.is_some_and(|i| !gs.runs[i].pages().is_empty());
        let stage_page = !f.live() && hist.is_some_and(|h| h.is_stage());
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
            self.text(M + 12, 88, W - 24, F_LABEL, DIM, DT_LEFT, card_label);
            M + 60
        } else {
            let label = match num {
                Some(n) => format!("{card_label} {n}"),
                None => card_label.to_string(),
            };
            let pw = self.unscale(self.text_w(F_LABEL, &label)) + 12;
            self.rround(M + 10, 87, pw, 18, 4, mix(CARD, kind, 0.7));
            self.text_rect(
                M + 10,
                87,
                pw,
                18,
                F_LABEL,
                TEXT,
                DT_CENTER | DT_VCENTER,
                &label,
            );
            (M + 10 + pw + 8).max(M + 60)
        };
        if has_fights && !f.live() {
            if let (Some(r), Some(g)) = (f.view_run, f.view_group) {
                self.kind_strip(&gs.runs[r], g);
            }
        }
        if has_fights {
            let free = !f.prompt && !f.settings_open;
            let back = f.view_group.map_or(f.group_pages > 1, |i| i > 0);
            self.glyph_button(
                FIGHT_PREV_HIT,
                f.ctl(Hit::FightPrev, free && back, false),
                GLYPH_PREV,
            );
            self.glyph_button(
                FIGHT_NEXT_HIT,
                f.ctl(Hit::FightNext, free && f.group_sel, false),
                GLYPH_NEXT,
            );
            let idx = format!(
                "{}/{}",
                f.view_group.map_or(f.group_pages, |i| i + 1),
                f.group_pages
            );
            self.text_rect(266, 86, 42, 20, F_TINY, DIM, DT_CENTER | DT_VCENTER, &idx);
        }
        if !f.live() {
            return self.draw_boss_history(f, f.view_run.map(|i| &gs.runs[i]), hist, tx);
        }
        match (gs.boss.as_ref().filter(|_| !f.empty()), &gs.target) {
            (Some(boss), target) => {
                let shown = boss_name(base_name(boss));
                self.text(tx, 88, M + 226 - tx, F_BOSS, TEXT, DT_LEFT, &shown);
                if mine {
                    self.text(
                        M + 12,
                        114,
                        W - 24,
                        F_LABEL,
                        DANGER,
                        DT_LEFT,
                        "TARGETING YOU",
                    );
                } else {
                    self.text(M + 12, 114, W - 24, F_LABEL, DIM, DT_LEFT, "TARGET");
                }
                let glyph = if f.sound_on { GLYPH_VOLUME } else { GLYPH_MUTE };
                let free = !f.prompt && !f.settings_open;
                self.glyph_button(MUTE_HIT, f.ctl(Hit::Mute, free, false), glyph);
                let pn = phase_num(boss);
                if pn > 1 {
                    let hint = format!("phase {pn}");
                    let right = MUTE_HIT.0 - 8 - (M + 12);
                    self.text(M + 12, 114, right, F_TINY, DIM, DT_RIGHT, &hint);
                }
                let color = if mine {
                    mix(TEXT, DANGER, ease_out_cubic(f.flash_t))
                } else {
                    mix(ACCENT, TEXT, ease_out_cubic(f.flash_t))
                };
                match target {
                    Some(t) => {
                        self.text(M + 12, 128, W - 24, F_BIG, color, DT_LEFT, t);
                        let held = now.saturating_sub(gs.target_since);
                        self.text_rect(
                            M + 12,
                            128,
                            W - 24,
                            34,
                            F_TINY,
                            DIM,
                            DT_RIGHT | DT_VCENTER,
                            &format!("{held}s"),
                        );
                    }
                    None => self.text(M + 12, 128, W - 24, F_BOSS, DIM, DT_LEFT, DASH),
                }
            }
            (None, _) if gs.pre_boss() && !f.empty() => {
                self.text(
                    tx,
                    88,
                    M + 226 - tx,
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
                self.text(M + 60, 88, 166, F_BOSS, TEXT, DT_LEFT, &head);
                self.text(M + 12, 114, W - 24, F_LABEL, DIM, DT_LEFT, "INTERMISSION");
                let line = match run {
                    Some(r) => format!(
                        "{} down   {} in the run",
                        fmt_count(r.kills(), "boss", "bosses"),
                        fmt_dur(now.saturating_sub(r.start_ts))
                    ),
                    None => DASH.to_string(),
                };
                self.text(M + 12, 128, W - 24, F_BOSS, TEXT, DT_LEFT, &line);
            }
            (None, _) => {
                self.text(M + 12, 112, W - 24, F_BOSS, DIM, DT_LEFT, "no boss active");
            }
        }
    }

    fn kind_strip(&self, run: &Run, cur: usize) {
        let pages = run.pages();
        let (shown, sw) = strip_layout(pages.len(), cur);
        for (k, i) in shown.enumerate() {
            let p = &pages[i];
            let stage = matches!(p, Page::Stage(_));
            let tone = if stage { ACCENT } else { DANGER };
            let on = i == cur;
            let x = STRIP_X + k as i32 * (sw + STRIP_GAP);
            let fill = mix(CARD, tone, if on { 0.75 } else { 0.22 });
            self.rround(x, STRIP_Y, sw, STRIP_H, 4, fill);
            let t = format!("{}{}", if stage { "S" } else { "B" }, run.stage_of(p));
            if self.unscale(self.text_w(F_TINY, &t)) + 4 <= sw {
                let ink = if on { TEXT } else { DIM };
                self.text_rect(
                    x,
                    STRIP_Y,
                    sw,
                    STRIP_H,
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
                return self.text(M + 12, 112, W - 24, F_BOSS, DIM, DT_LEFT, "no boss fights");
            };
            self.text(
                tx,
                88,
                M + 226 - tx,
                F_BOSS,
                TEXT,
                DT_LEFT,
                stage_name(&r.stage),
            );
            self.text(M + 12, 114, W - 24, F_LABEL, DIM, DT_LEFT, "RESULT");
            let res = match r.end {
                Some(RunEnd::Left) => "left before a boss",
                Some(_) => "lobby before a boss",
                None => "no boss yet",
            };
            self.text(M + 12, 128, W - 24, F_BOSS, DIM, DT_LEFT, res);
            let dur = fmt_dur(r.end_ts.unwrap_or(f.now).saturating_sub(r.start_ts));
            return self.text_rect(
                M + 12,
                128,
                W - 24,
                22,
                F_TINY,
                DIM,
                DT_RIGHT | DT_VCENTER,
                &dur,
            );
        };
        self.text(
            tx,
            88,
            M + 226 - tx,
            F_BOSS,
            TEXT,
            DT_LEFT,
            &if h.is_stage() {
                Cow::Borrowed(stage_name(h.name))
            } else {
                boss_name(base_name(h.name))
            },
        );
        self.text(M + 12, 114, W - 24, F_LABEL, DIM, DT_LEFT, "RESULT");
        if h.n_phases > 1 {
            let hint = format!("{} phases", h.n_phases);
            self.text(M + 12, 114, W - 24, F_TINY, DIM, DT_RIGHT, &hint);
        }
        let res = h.result();
        let color = match res {
            "lost" => DANGER,
            "killed" | "cleared" => GOOD,
            "unfinished" => DIM,
            _ => AMBER,
        };
        self.text(M + 12, 128, W - 24, F_BOSS, color, DT_LEFT, res);
        let deaths: u32 = h.fights.iter().map(|p| p.deaths).sum();
        let mut right = fmt_dur(h.duration(f.now));
        if deaths > 0 {
            right = format!("died {deaths}x   {right}");
        }
        self.text_rect(
            M + 12,
            128,
            W - 24,
            22,
            F_TINY,
            DIM,
            DT_RIGHT | DT_VCENTER,
            &right,
        );
    }

    fn card_graph(
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
            Some(g) if taken => {
                self.graph(gx, gy, gw, gh, &g.taken_graph(f.now, GRAPH_COLS), DANGER)
            }
            Some(g) => self.graph(gx, gy, gw, gh, &g.graph(f.now, GRAPH_COLS), AMBER),
            None if f.live() => {
                self.graph_well(gx, gy, gw, gh);
                self.text_rect(
                    gx,
                    gy,
                    gw,
                    gh,
                    F_TINY,
                    DIM,
                    DT_CENTER | DT_VCENTER,
                    "graph starts with the boss",
                );
            }
            None => {}
        }
    }

    fn draw_dealt_card(&self, f: &Frame, gs: &GameState, hist: Option<&FightGroup>) {
        let now = f.now;
        self.rround(M, 186, W, 92 + GRAPH_DY, 8, CARD);
        self.card_graph(f, gs, hist, GRAPH_RECT.1, false);
        let line_y = 288;
        if f.live() {
            let kind = live_kind(f, gs);
            let win = format!("DPS {}s", f.window);
            let (l0, v0, l1, v1, l2, v2) = match kind {
                Live::Pre => (
                    win.as_str(),
                    or_dash(f, fmt_anim(f.dps_shown)),
                    "STAGE DPS",
                    group_digits(gs.stage_dps(now)),
                    "STAGE DMG",
                    group_digits(gs.stage_stats.dmg),
                ),
                Live::Inter(r) => {
                    let active = r.active_secs(now).max(1);
                    (
                        "RUN DPS",
                        group_digits(r.dmg() / active),
                        "RUN DMG",
                        group_digits(r.dmg()),
                        "BOSSES",
                        format!("{}/{}", r.kills(), r.fights.len()),
                    )
                }
                Live::Fight => (
                    win.as_str(),
                    fmt_anim(f.dps_shown),
                    "FIGHT DPS",
                    fmt_anim(f.fight_dps_shown),
                    "FIGHT DMG",
                    group_digits(gs.fight_dmg),
                ),
                Live::Idle => (
                    win.as_str(),
                    or_dash(f, fmt_anim(f.dps_shown)),
                    "FIGHT DPS",
                    DASH.to_string(),
                    "FIGHT DMG",
                    DASH.to_string(),
                ),
            };
            self.stat(192, 0, l0, &v0, AMBER);
            self.stat(192, 1, l1, &v1, AMBER);
            self.stat(192, 2, l2, &v2, AMBER);
            match gs.last_kill_total().filter(|_| !f.empty()) {
                Some((boss, strike, other)) => {
                    let line = format!(
                        "last kill  {}   {} strike + {} other",
                        boss_name(&boss),
                        group_digits(strike),
                        group_digits(other)
                    );
                    self.text(M + 12, line_y, W - 24, F_BODY, TEXT, DT_LEFT, &line);
                }
                None => self.text(M + 12, line_y, W - 24, F_BODY, DIM, DT_LEFT, "no kills yet"),
            }
        } else if let Some(h) = hist {
            let dur = h.duration(now);
            self.stat(192, 0, "DMG", &group_digits(h.dmg), AMBER);
            self.stat(192, 1, "DPS", &h.dps(now).to_string(), AMBER);
            self.stat(192, 2, "TIME", &fmt_dur(dur), AMBER);
            match h.kill {
                Some((s, ns)) => {
                    let line = format!(
                        "{}  {} strike + {} other",
                        if h.last.lost { "dealt" } else { "kill" },
                        group_digits(s),
                        group_digits(ns)
                    );
                    self.text(M + 12, line_y, W - 24, F_BODY, TEXT, DT_LEFT, &line);
                }
                None => {
                    self.text(
                        M + 12,
                        line_y,
                        W - 24,
                        F_BODY,
                        DIM,
                        DT_LEFT,
                        if h.is_stage() {
                            "clearing before the boss"
                        } else {
                            "no kill recorded"
                        },
                    );
                }
            }
        } else if let Some(r) = f.view_run.map(|i| &gs.runs[i]) {
            let active = r.active_secs(r.end_ts.unwrap_or(now));
            self.stat(192, 0, "DMG", &group_digits(r.dmg()), AMBER);
            self.stat(192, 1, "DPS", &(r.dmg() / active.max(1)).to_string(), AMBER);
            self.stat(192, 2, "CLEARING", &fmt_dur(active), AMBER);
            self.text(
                M + 12,
                line_y,
                W - 24,
                F_BODY,
                DIM,
                DT_LEFT,
                "no boss fight",
            );
        } else {
            self.text(M + 12, 208, W - 24, F_BODY, DIM, DT_LEFT, "no data");
        }
    }

    fn draw_splits(&self, f: &Frame, gs: &GameState) {
        let (cx, cy, cw, ch) = SPLITS_CARD;
        self.rround(cx, cy, cw, ch, 8, CARD);
        let Some(ri) = f.view_run else {
            return;
        };
        let run = &gs.runs[ri];
        let rows = gs.run_splits(ri, f.now);
        let cur = if f.live() {
            rows.len().checked_sub(1)
        } else {
            f.view_group
                .and_then(|g| run.pages().into_iter().nth(g))
                .map(|p| run.stage_of(&p) as usize - 1)
        };
        let vc = DT_VCENTER;
        self.text_rect(
            M + 12,
            STAGES_BTN.1,
            STAGES_BTN.0 - M - 20,
            STAGES_BTN.3,
            F_LABEL,
            DIM,
            DT_LEFT | vc,
            "THIS RUN BY STAGE",
        );
        let hy = cy + 26;
        for (x, w, a, h) in SPLIT_COLS {
            let ink = if h == "TAKEN" {
                mix(DANGER, DIM, 0.4)
            } else {
                DIM
            };
            self.text_rect(x, hy, w, 16, F_TINY, ink, a | vc, h);
        }
        let n = rows.len();
        let top = f.split_top.min(n.saturating_sub(SPLIT_ROWS));
        let shown = top..n.min(top + SPLIT_ROWS);
        let total = rows.iter().fold(Split::default(), |mut t, r| {
            t.secs += r.secs;
            t.dmg += r.dmg;
            t.taken += r.taken;
            t.deaths += r.deaths;
            t
        });
        let fits = |col: usize, pick: fn(&Split) -> u64| {
            rows[shown.clone()]
                .iter()
                .chain([&total])
                .all(|r| self.text_w(F_BODY, &group_digits(pick(r))) <= self.px(SPLIT_COLS[col].1))
        };
        let full_d = fits(2, |r| r.dmg);
        let full_t = fits(3, |r| r.taken);
        let num = |full: bool, v: u64| if full { group_digits(v) } else { compact(v) };
        let h = SPLIT_ROW_H;
        let cell = |y: i32, col: usize, font: usize, ink: u32, s: &str| {
            let (x, w, a, _) = SPLIT_COLS[col];
            self.text_rect(x, y, w, h, font, ink, a | vc, s);
        };
        let line = |y: i32, r: &Split| {
            cell(y, 1, F_BODY, TEXT, &fmt_split(r.secs));
            cell(y, 2, F_BODY, AMBER, &num(full_d, r.dmg));
            cell(y, 3, F_BODY, DANGER, &num(full_t, r.taken));
            let dc = if r.deaths > 0 { DANGER } else { DIM };
            cell(y, 4, F_BODY, dc, &r.deaths.to_string());
        };
        let mut y = hy + 18;
        for i in shown.clone() {
            let r = &rows[i];
            if cur == Some(i) {
                self.rround(M + 6, y, W - 12, h, 5, mix(CARD, ACCENT, 0.18));
            } else if i % 2 == 1 {
                self.rround(M + 6, y, W - 12, h, 5, mix(CARD, CARD_HI, 0.5));
            }
            let name = format!("{}  {}", i + 1, stage_name(&r.stage));
            cell(y, 0, F_BODY, TEXT, &name);
            line(y, r);
            y += h;
        }
        if n > SPLIT_ROWS {
            let more = format!(
                "{} to {} of {n} stages, scroll for the rest",
                shown.start + 1,
                shown.end
            );
            self.text_rect(M + 12, y, W - 24, 20, F_TINY, DIM, DT_CENTER | vc, &more);
            y += 20;
        }
        self.rround(M + 12, y + 3, W - 24, 1, 0, EDGE);
        y += 6;
        cell(y, 0, F_LABEL, DIM, "RUN");
        line(y, &total);
    }

    fn draw_settings(&self, f: &Frame) {
        let (cx, cy, cw, ch) = SETTINGS_CARD;
        self.rround(cx, cy, cw, ch, 8, CARD_HI);
        let steppers = [
            (
                "SIZE",
                format!("{}%", (f.scale * 100.0).round() as i32),
                (Hit::ScaleDown, SCALE_DOWN_HIT, f.scale > MIN_SCALE + 0.001),
                (Hit::ScaleUp, SCALE_UP_HIT, f.scale < f.scale_max - 0.001),
            ),
            (
                "OPACITY",
                format!("{}%", f.alpha),
                (Hit::AlphaDown, ALPHA_DOWN_HIT, f.alpha > MIN_ALPHA),
                (Hit::AlphaUp, ALPHA_UP_HIT, f.alpha < MAX_ALPHA),
            ),
            (
                "STAT WINDOW",
                format!("{} s", f.window),
                (Hit::WindowDown, WINDOW_DOWN_HIT, f.window > MIN_WINDOW),
                (Hit::WindowUp, WINDOW_UP_HIT, f.window < MAX_WINDOW),
            ),
        ];
        let (vx, vw) = SETTING_VALUE;
        for (label, value, down, up) in steppers {
            let y = down.1 .1;
            self.setting_label(y, label);
            self.text_rect(vx, y, vw, 24, F_BOSS, TEXT, DT_CENTER | DT_VCENTER, &value);
            for (hit, rect, on) in [down, up] {
                let glyph = if hit == down.0 {
                    GLYPH_MINUS
                } else {
                    GLYPH_PLUS
                };
                self.glyph_button(rect, f.ctl(hit, on && !f.prompt, false), glyph);
            }
        }
        for (label, hit, rect, on) in [
            ("AGGRO SOUND", Hit::Sound, SOUND_HIT, f.sound_on),
            ("DISCORD STATUS", Hit::Discord, DISCORD_HIT, f.discord_on),
            ("SHARE FIGHT DATA", Hit::Usage, USAGE_HIT, f.usage_on),
            ("LEADERBOARD", Hit::Board, BOARD_HIT, f.board_on),
        ] {
            self.setting_label(rect.1, label);
            let word = if on { "On" } else { "Off" };
            self.text_button(rect, f.ctl(hit, !f.prompt, on), word);
        }
        if f.usage_failing {
            self.dot(USAGE_HIT.0 - 16, USAGE_HIT.1 + 8, 8, AMBER);
        }
        if f.board_on && f.board_status != crate::board::OK {
            let color = if f.board_status == crate::board::TAKEN {
                DANGER
            } else {
                AMBER
            };
            self.dot(BOARD_HIT.0 - 16, BOARD_HIT.1 + 8, 8, color);
        }
        self.setting_label(BOARD_DELETE_HIT.1, "BOARD KILLS");
        self.delete_button(f);
    }

    fn delete_button(&self, f: &Frame) {
        let (x, y, w, h) = BOARD_DELETE_HIT;
        let hover = f.hover == Some(Hit::BoardDelete) && !f.prompt;
        let (word, ink) = match f.board_delete {
            DELETE_BUSY => ("Deleting", INK_OFF),
            DELETE_ARMED => {
                let fill = if hover {
                    mix(DANGER, TEXT, 0.2)
                } else {
                    DANGER
                };
                self.rround(x, y, w, h, 6, fill);
                ("Sure?", BG)
            }
            _ if f.prompt => ("Delete", INK_OFF),
            _ => {
                if hover {
                    self.rround(x, y, w, h, 6, mix(CARD, DANGER, 0.3));
                }
                self.outline(x, y, w, h, 6, mix(EDGE, DANGER, 0.6));
                ("Delete", DANGER)
            }
        };
        self.text_rect(x, y, w, h, F_LABEL, ink, DT_CENTER | DT_VCENTER, word);
    }

    fn setting_label(&self, y: i32, label: &str) {
        self.text_rect(
            M + 12,
            y,
            SCALE_DOWN_HIT.0 - M - 20,
            24,
            F_LABEL,
            DIM,
            DT_LEFT | DT_VCENTER,
            label,
        );
    }

    fn draw_taken_card(&self, f: &Frame, gs: &GameState, hist: Option<&FightGroup>) {
        let now = f.now;
        let card = mix(CARD, DANGER, 0.22 * (1.0 - ease_out_cubic(f.taken_flash_t)));
        let top = TAKEN_CARD_Y;
        self.rround(M, top, W, TAKEN_CARD_H, 8, card);
        let stat_y = top + 6;
        let line_y = top + 102;
        let small_y = top + 128;
        let list_y = top + 170;
        if f.live() {
            let t = LiveTaken::of(f, gs);
            self.stat(stat_y, 0, &t.l0, &t.v0, DANGER);
            self.stat(stat_y, 1, t.l1, &t.v1, DANGER);
            self.stat(stat_y, 2, t.l2, &t.v2, DANGER);
            self.card_graph(f, gs, hist, top + 54, true);
            match gs.taken.back().filter(|_| !f.empty()) {
                Some(hit) => {
                    let (who, attack) = describe_source(&hit.source);
                    let line = format!("last hit  {}   {who}   {attack}", hit.amount);
                    self.text_rect(
                        M + 12,
                        line_y,
                        W - 80,
                        18,
                        F_BODY,
                        TEXT,
                        DT_LEFT | DT_VCENTER,
                        &line,
                    );
                    self.text_rect(
                        M + 12,
                        line_y,
                        W - 24,
                        18,
                        F_TINY,
                        DIM,
                        DT_RIGHT | DT_VCENTER,
                        &fmt_ago(now, hit.ts),
                    );
                }
                None => self.text(M + 12, line_y, W - 24, F_BODY, DIM, DT_LEFT, "no hits yet"),
            }
            self.small_stat(small_y, 0, "HITS", &t.hits);
            self.small_stat(small_y, 1, "BIGGEST HIT", &t.big);
            self.small_stat(small_y, 2, "AVG HIT", &t.avg);
            let deaths = gs.live_run().map_or(0, |r| r.deaths);
            let note = (!f.empty() && deaths > 0).then(|| fmt_run_deaths(deaths));
            self.breakdown(list_y, &t.attacks, note.as_deref(), t.empty);
        } else if let Some(h) = hist {
            let dur = h.duration(now).max(1);
            self.stat(stat_y, 0, "TAKEN", &group_digits(h.taken), DANGER);
            self.stat(stat_y, 1, "DTPS", &(h.taken / dur).to_string(), DANGER);
            self.stat(stat_y, 2, "HITS", &h.hits.to_string(), DANGER);
            self.card_graph(f, gs, hist, top + 54, true);
            let deaths: u32 = h.fights.iter().map(|f| f.deaths).sum();
            self.deaths_line(line_y, deaths);
            let big = h.fights.iter().map(|f| f.max_hit).max().unwrap_or(0);
            self.small_stat(small_y, 0, "BIGGEST HIT", &or_none(big));
            self.small_stat(small_y, 1, "AVG HIT", &avg_hit(h.taken, h.hits));
            let attacks = merge_tallies(h.fights.iter().map(|f| f.attacks.as_slice()));
            self.breakdown(list_y, &attacks, None, "nothing hit you");
        } else if let Some(r) = f.view_run.map(|i| &gs.runs[i]) {
            let active = r.active_secs(r.end_ts.unwrap_or(now)).max(1);
            self.stat(stat_y, 0, "TAKEN", &group_digits(r.taken()), DANGER);
            self.stat(stat_y, 1, "DTPS", &(r.taken() / active).to_string(), DANGER);
            self.stat(stat_y, 2, "HITS", &r.hit_count().to_string(), DANGER);
            self.deaths_line(line_y, r.deaths);
            self.small_stat(small_y, 0, "BIGGEST HIT", &or_none(r.max_hit()));
            self.small_stat(small_y, 1, "AVG HIT", &avg_hit(r.taken(), r.hit_count()));
            self.breakdown(list_y, &r.attacks(), None, "nothing hit you");
        } else {
            self.text(M + 12, stat_y + 16, W - 24, F_BODY, DIM, DT_LEFT, "no data");
        }
    }

    fn deaths_line(&self, y: i32, deaths: u32) {
        match deaths {
            0 => self.text(M + 12, y, W - 24, F_BODY, DIM, DT_LEFT, "no deaths"),
            n => self.text(M + 12, y, W - 24, F_BODY, TEXT, DT_LEFT, &fmt_deaths(n)),
        }
    }

    fn draw_footer(&self, f: &Frame) {
        let fy = LOGICAL_H - 26;
        let vr_color = match f.vr {
            VrStatus::On => GOOD,
            VrStatus::Off => DIM,
            VrStatus::Failing => DANGER,
        };
        self.dot(M, fy + 4, 8, vr_color);
        self.text(M + 14, fy, 80, F_TINY, DIM, DT_LEFT, "STEAMVR");
        let log_color = if f.log_ok { GOOD } else { AMBER };
        self.dot(M + 76, fy + 4, 8, log_color);
        self.text(M + 90, fy, 80, F_TINY, DIM, DT_LEFT, "LOG");
        let discord_color = match (f.discord_on, f.discord) {
            (false, _) | (true, Link::Off) => DIM,
            (true, Link::Waiting) => AMBER,
            (true, Link::Connected) => GOOD,
            (true, Link::Rejected) => DANGER,
        };
        let (dx, _, _, _) = DISCORD_DOT;
        self.dot(dx + 4, fy + 4, 8, discord_color);
        self.text(dx + 18, fy, 60, F_TINY, DIM, DT_LEFT, "DISCORD");
        let (utext, ucolor) = match &f.update {
            Badge::Ready(_) if f.prompt => (VERSION.to_string(), DIM),
            Badge::None => (VERSION.to_string(), DIM),
            Badge::Ready(tag) => (format!("update {tag}"), ACCENT),
            Badge::Installing(_) => ("updating".to_string(), AMBER),
            Badge::Failed(_) => ("update failed".to_string(), DANGER),
        };
        if matches!(f.update, Badge::Ready(_) | Badge::Installing(_)) && !f.prompt {
            let color = if f.hov(Hit::Update) { TEXT } else { ucolor };
            let x = UPDATE_HIT.0;
            self.link(
                x,
                fy - 3,
                LOGICAL_W - M - x,
                20,
                F_TINY,
                color,
                DT_RIGHT,
                &utext,
            );
        } else {
            self.text(M, fy, W, F_TINY, ucolor, DT_RIGHT, &utext);
        }
    }

    fn draw_token_warning(&self, f: &Frame, gs: &GameState) {
        let (got, total) = gs.tokens_shown().unwrap_or((0, 0));
        let t = f.warn_t;
        let slide_in = ease_out_cubic((t / 0.08).min(1.0));
        let slide_out = ease_out_cubic(((t - 0.88) / 0.12).max(0.0));
        let y = (lerp(-140.0, 96.0, slide_in) - slide_out * 236.0) as i32;
        let pulse = 0.5 + 0.5 * (t * std::f32::consts::TAU * 3.0).sin();
        self.rround(M, y, W, 128, 10, mix(AMBER, TEXT, pulse * 0.6));
        self.rround(M + 3, y + 3, W - 6, 122, 8, mix(BG, AMBER, 0.18));
        let vc = DT_CENTER | DT_VCENTER;
        self.text_rect(M, y + 14, W, 40, F_BIG, AMBER, vc, "COLLECT YOUR TOKENS");
        let line = format!("{got}/{total} picked up");
        self.text_rect(M, y + 58, W, 28, F_BOSS, TEXT, vc, &line);
        let hint = "grab the rest before you summon the boss";
        self.text_rect(M, y + 88, W, 24, F_BODY, DIM, vc, hint);
    }
}
