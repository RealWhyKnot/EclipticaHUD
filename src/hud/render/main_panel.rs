use super::fmt::*;
use super::frame::{Env, Frame};
use super::hit::{Hit, TipCtx};
use super::layout::*;
use super::theme::*;
use super::Renderer;
use crate::discord::Link;
use crate::game::event::fmt_clock;
use crate::game::names::{boss_name, phase_name, stage_name};
use crate::game::run::{base_name, merge_tallies, phase_num, FightGroup, Run, RunEnd, Tally};
use crate::game::source::describe_source;
use crate::game::state::{GameState, Mode};
use crate::update::{Badge, VERSION};
use crate::vr::VrStatus;
use windows_sys::Win32::Graphics::Gdi::*;

const DASH: &str = "-";
const COL: i32 = (W - 24) / 3;

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
    big: String,
    avg: String,
    rate: String,
    attacks: Vec<Tally>,
    total: u64,
    empty: &'static str,
}

impl LiveTaken {
    fn of(f: &Frame, gs: &GameState) -> Self {
        let now = f.now;
        let win = format!("TAKEN {}s", f.window);
        let s = &gs.stage_stats;
        match live_kind(f, gs) {
            Live::Pre => LiveTaken {
                l0: win,
                v0: or_dash(f, fmt_anim(f.taken_shown)),
                l1: "STAGE TAKEN",
                v1: group_digits(s.taken),
                l2: "HITS",
                v2: s.hits.to_string(),
                big: s.max_hit.to_string(),
                avg: avg_hit(s.taken, s.hits),
                rate: group_digits(gs.stage_taken_rate(now)),
                attacks: s.attacks.clone(),
                total: s.taken,
                empty: "nothing has hit you this stage",
            },
            Live::Inter(r) => {
                let active = r.active_secs(now).max(1);
                LiveTaken {
                    l0: "RUN TAKEN".to_string(),
                    v0: group_digits(r.taken()),
                    l1: "HITS",
                    v1: r.hit_count().to_string(),
                    l2: "DEATHS",
                    v2: r.deaths.to_string(),
                    big: r.max_hit().to_string(),
                    avg: avg_hit(r.taken(), r.hit_count()),
                    rate: group_digits(r.taken() / active),
                    attacks: r.attacks(),
                    total: r.taken(),
                    empty: "nothing has hit you this run",
                }
            }
            Live::Fight => LiveTaken {
                l0: win,
                v0: fmt_anim(f.taken_shown),
                l1: "FIGHT TAKEN",
                v1: group_digits(gs.fight_taken),
                l2: "HITS",
                v2: gs.fight_hits.to_string(),
                big: gs.fight_max_hit.to_string(),
                avg: avg_hit(gs.fight_taken, gs.fight_hits),
                rate: fmt_anim(f.taken_rate_shown),
                attacks: gs.fight_attacks.clone(),
                total: gs.fight_taken,
                empty: "nothing has hit you this fight",
            },
            Live::Idle => LiveTaken {
                l0: win,
                v0: or_dash(f, fmt_anim(f.taken_shown)),
                l1: "FIGHT TAKEN",
                v1: DASH.to_string(),
                l2: "HITS",
                v2: DASH.to_string(),
                big: DASH.to_string(),
                avg: DASH.to_string(),
                rate: DASH.to_string(),
                attacks: Vec::new(),
                total: 0,
                empty: "attack breakdown starts with the next boss",
            },
        }
    }
}

impl Renderer {
    pub fn draw_main(&mut self, gs: &GameState, f: &Frame) {
        let hist = f
            .view_run
            .zip(f.view_group)
            .and_then(|(r, g)| gs.runs[r].group(g));
        self.fill(0, 0, LOGICAL_W, LOGICAL_H, BG);
        self.draw_titlebar(f);
        self.draw_status(f, gs);
        self.draw_run_pager(f, gs);
        self.draw_boss_card(f, gs, hist.as_ref());
        self.draw_dealt_card(f, gs, hist.as_ref());
        if f.settings_open {
            self.draw_settings(f);
        }
        self.draw_taken_card(f, gs, hist.as_ref());
        self.draw_footer(f);
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
        self.glyph_button(
            PIN_BTN,
            f.hov(Hit::Pin),
            f.prs(Hit::Pin),
            f.topmost,
            if f.topmost { GLYPH_PIN } else { GLYPH_UNPIN },
        );
        self.glyph_button(
            LOG_BTN,
            f.hov(Hit::Log),
            f.prs(Hit::Log),
            f.log_open,
            GLYPH_LOG,
        );
        self.glyph_button(
            SETTINGS_BTN,
            f.hov(Hit::Settings),
            f.prs(Hit::Settings),
            f.settings_open,
            GLYPH_SETTINGS,
        );
        self.glyph_button(
            CLOSE_BTN,
            f.hov(Hit::Close),
            f.prs(Hit::Close),
            false,
            GLYPH_CLOSE,
        );
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
        self.text(M, 28, W, F_BODY, status_color, DT_LEFT, &status);
        if !in_world {
        } else if gs.is_dead(now) {
            let color = mix(mix(BG, DANGER, 0.45), DANGER, f.dead_pulse);
            self.text(M, 28, W, F_BODY, color, DT_RIGHT, "DEAD");
        } else if let Some((got, total)) = gs.tokens_shown() {
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
        }
        if in_world && gs.mode == Mode::Stage {
            self.bar(M, 47, W, 4, f.progress_shown, GOOD);
        }
    }

    fn draw_run_pager(&self, f: &Frame, gs: &GameState) {
        self.arrow(
            RUN_PREV_HIT,
            f.view_page > 0,
            f.hov(Hit::RunPrev),
            f.prs(Hit::RunPrev),
            GLYPH_PREV,
        );
        self.arrow(
            RUN_NEXT_HIT,
            f.run_sel,
            f.hov(Hit::RunNext),
            f.prs(Hit::RunNext),
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
        self.rround(M, 82, W, 96, 8, CARD);
        let card_label = if f.live() && !f.empty() && gs.pre_boss() {
            "STAGE"
        } else if f.live() && !f.empty() && gs.mode == Mode::Intermission {
            "RUN"
        } else {
            "BOSS"
        };
        self.text(M + 12, 88, W - 24, F_LABEL, DIM, DT_LEFT, card_label);
        let has_fights = f.view_run.is_some_and(|i| !gs.runs[i].fights.is_empty());
        if has_fights {
            self.arrow(
                FIGHT_PREV_HIT,
                f.view_group.map_or(f.group_pages > 1, |i| i > 0),
                f.hov(Hit::FightPrev),
                f.prs(Hit::FightPrev),
                GLYPH_PREV,
            );
            self.arrow(
                FIGHT_NEXT_HIT,
                f.group_sel,
                f.hov(Hit::FightNext),
                f.prs(Hit::FightNext),
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
            return self.draw_boss_history(f, f.view_run.map(|i| &gs.runs[i]), hist);
        }
        match (gs.boss.as_ref().filter(|_| !f.empty()), &gs.target) {
            (Some(boss), target) => {
                let shown = boss_name(base_name(boss));
                self.text(M + 60, 88, 166, F_BOSS, TEXT, DT_LEFT, shown);
                self.text(M + 12, 114, W - 24, F_LABEL, DIM, DT_LEFT, "TARGET");
                let mut right = W - 24;
                if !f.sound_on {
                    self.text(M + 12, 114, right, F_GLYPH, DIM, DT_RIGHT, GLYPH_MUTE);
                    right -= 20;
                }
                let pn = phase_num(boss);
                if pn > 1 {
                    let hint = format!("phase {pn}");
                    self.text(M + 12, 114, right, F_TINY, DIM, DT_RIGHT, &hint);
                }
                let color = mix(ACCENT, TEXT, ease_out_cubic(f.flash_t));
                match target {
                    Some(t) => {
                        if f.hov(Hit::Target) || f.prs(Hit::Target) {
                            let (tx, ty, tw, th) = TARGET_HIT;
                            let tint = if f.prs(Hit::Target) { ACCENT } else { CARD_HI };
                            self.rround(tx - 6, ty - 2, tw + 6, th, 6, tint);
                        }
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
                    M + 60,
                    88,
                    166,
                    F_BOSS,
                    TEXT,
                    DT_LEFT,
                    stage_name(&gs.stage),
                );
                self.text(M + 12, 114, W - 24, F_LABEL, DIM, DT_LEFT, "CLEARING");
                let since = fmt_dur(gs.stage_secs(now));
                self.text(M + 12, 128, W - 24, F_BOSS, AMBER, DT_LEFT, &since);
                if let Some((got, total)) = gs.tokens_shown() {
                    self.text_rect(
                        M + 12,
                        128,
                        W - 24,
                        22,
                        F_TINY,
                        DIM,
                        DT_RIGHT | DT_VCENTER,
                        &format!("{got}/{total} tokens"),
                    );
                }
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
                        "{} bosses down   {} in the run",
                        r.kills(),
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

    fn draw_boss_history(&self, f: &Frame, run: Option<&Run>, hist: Option<&FightGroup>) {
        let Some(h) = hist else {
            let Some(r) = run else {
                return self.text(M + 12, 112, W - 24, F_BOSS, DIM, DT_LEFT, "no boss fights");
            };
            self.text(M + 60, 88, 166, F_BOSS, TEXT, DT_LEFT, stage_name(&r.stage));
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
            M + 60,
            88,
            166,
            F_BOSS,
            TEXT,
            DT_LEFT,
            boss_name(base_name(h.name)),
        );
        self.text(M + 12, 114, W - 24, F_LABEL, DIM, DT_LEFT, "RESULT");
        if h.n_phases > 1 {
            let hint = format!("{} phases", h.n_phases);
            self.text(M + 12, 114, W - 24, F_TINY, DIM, DT_RIGHT, &hint);
        }
        let res = h.result();
        let color = match res {
            "lost" => DANGER,
            "killed" => GOOD,
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

    fn draw_dealt_card(&self, f: &Frame, gs: &GameState, hist: Option<&FightGroup>) {
        let now = f.now;
        self.rround(M, 186, W, 92, 8, CARD);
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
            if matches!(kind, Live::Fight) {
                self.bar(M + 12, 233, W - 24, 3, f.dps_frac_shown, ACCENT);
            }
            match gs.last_kill_total().filter(|_| !f.empty()) {
                Some((boss, strike, other)) => {
                    let line = format!(
                        "last kill  {}   {} strike + {} other",
                        boss_name(&boss),
                        group_digits(strike),
                        group_digits(other)
                    );
                    self.text(M + 12, 242, W - 24, F_BODY, TEXT, DT_LEFT, &line);
                }
                None => self.text(M + 12, 242, W - 24, F_BODY, DIM, DT_LEFT, "no kills yet"),
            }
        } else if let Some(h) = hist {
            let dur = h.duration(now);
            self.stat(192, 0, "DMG", &group_digits(h.dmg), AMBER);
            self.stat(192, 1, "DPS", &h.dps(now).to_string(), AMBER);
            self.stat(192, 2, "TIME", &fmt_dur(dur), AMBER);
            match h.kill {
                Some((s, ns)) => {
                    let line = format!(
                        "kill  {} strike + {} other",
                        group_digits(s),
                        group_digits(ns)
                    );
                    self.text(M + 12, 242, W - 24, F_BODY, TEXT, DT_LEFT, &line);
                }
                None => {
                    self.text(
                        M + 12,
                        242,
                        W - 24,
                        F_BODY,
                        DIM,
                        DT_LEFT,
                        "no kill recorded",
                    );
                }
            }
        } else {
            self.text(M + 12, 208, W - 24, F_BODY, DIM, DT_LEFT, "no data");
        }
    }

    fn draw_settings(&self, f: &Frame) {
        self.rround(M, 82, W, 184, 8, CARD_HI);
        let rows = [
            (
                "SIZE",
                format!("{}%", (f.scale * 100.0).round() as i32),
                (Hit::ScaleDown, SCALE_DOWN_HIT, f.scale > MIN_SCALE + 0.001),
                (Hit::ScaleUp, SCALE_UP_HIT, f.scale < MAX_SCALE - 0.001),
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
        for (label, value, down, up) in rows {
            let y = down.1 .1;
            self.text_rect(
                M + 12,
                y,
                150,
                24,
                F_LABEL,
                DIM,
                DT_LEFT | DT_VCENTER,
                label,
            );
            self.text_rect(226, y, 80, 24, F_BOSS, TEXT, DT_CENTER | DT_VCENTER, &value);
            for (hit, rect, on) in [down, up] {
                let glyph = if hit == down.0 {
                    GLYPH_MINUS
                } else {
                    GLYPH_PLUS
                };
                self.arrow(rect, on, f.hov(hit), f.prs(hit), glyph);
            }
        }
    }

    fn draw_taken_card(&self, f: &Frame, gs: &GameState, hist: Option<&FightGroup>) {
        let now = f.now;
        let card = mix(CARD, DANGER, 0.22 * (1.0 - ease_out_cubic(f.taken_flash_t)));
        self.rround(M, 286, W, 276, 8, card);
        self.text(M + 12, 292, W - 24, F_LABEL, DIM, DT_LEFT, "DAMAGE TAKEN");
        let live_deaths = gs.live_run().map_or(0, |r| r.deaths);
        if f.live() && !f.empty() && live_deaths > 0 {
            self.text(
                M + 12,
                292,
                W - 24,
                F_TINY,
                DIM,
                DT_RIGHT,
                &fmt_run_deaths(live_deaths),
            );
        }
        if f.live() {
            let t = LiveTaken::of(f, gs);
            self.stat(312, 0, &t.l0, &t.v0, DANGER);
            self.stat(308, 1, t.l1, &t.v1, DANGER);
            self.stat(308, 2, t.l2, &t.v2, DANGER);
            self.small_stat(350, 0, "BIGGEST HIT", &t.big);
            self.small_stat(356, 1, "AVG HIT", &t.avg);
            self.small_stat(350, 2, "TAKEN/S", &t.rate);
            if matches!(live_kind(f, gs), Live::Fight) {
                self.bar_on(M + 12, 396, W - 24, 3, f.taken_frac_shown, DANGER, BG);
            }
            match gs.taken.back().filter(|_| !f.empty()) {
                Some(hit) => {
                    let (who, attack) = describe_source(&hit.source);
                    let line = format!("last hit  {}   {who}   {attack}", hit.amount);
                    self.text_rect(
                        M + 12,
                        402,
                        W - 80,
                        20,
                        F_BODY,
                        TEXT,
                        DT_LEFT | DT_VCENTER,
                        &line,
                    );
                    self.text_rect(
                        M + 12,
                        402,
                        W - 24,
                        20,
                        F_TINY,
                        DIM,
                        DT_RIGHT | DT_VCENTER,
                        &fmt_ago(now, hit.ts),
                    );
                }
                None => self.text(M + 12, 404, W - 24, F_BODY, DIM, DT_LEFT, "no hits yet"),
            }
            if t.attacks.is_empty() {
                self.text_rect(
                    M + 12,
                    430,
                    W - 24,
                    120,
                    F_BODY,
                    DIM,
                    DT_CENTER | DT_VCENTER,
                    t.empty,
                );
            } else {
                self.breakdown(430, &t.attacks, t.total);
            }
        } else if let Some(h) = hist {
            let dur = h.duration(now);
            self.stat(312, 0, "TAKEN", &group_digits(h.taken), DANGER);
            self.stat(312, 1, "HITS", &h.hits.to_string(), DANGER);
            self.stat(
                312,
                2,
                "TAKEN/S",
                &(h.taken / dur.max(1)).to_string(),
                DANGER,
            );
            let avg = if h.hits > 0 {
                h.taken / h.hits as u64
            } else {
                0
            };
            let big = if h.taken > 0 {
                avg.to_string()
            } else {
                DASH.to_string()
            };
            self.small_stat(356, 0, "AVG HIT", &big);
            let attacks = merge_tallies(h.fights.iter().map(|f| f.attacks.as_slice()));
            self.breakdown(400, &attacks, h.taken);
        } else {
            self.text(M + 12, 324, W - 24, F_BODY, DIM, DT_LEFT, "no data");
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
        let (dx, _, _, _) = DISCORD_HIT;
        self.dot(dx + 4, fy + 4, 8, discord_color);
        let label = if f.hov(Hit::Discord) { TEXT } else { DIM };
        self.text(dx + 18, fy, 60, F_TINY, label, DT_LEFT, "DISCORD");
        let (utext, ucolor) = match &f.update {
            Badge::None => (VERSION.to_string(), DIM),
            Badge::Ready(tag) => (format!("update {tag}"), ACCENT),
            Badge::Installing => ("updating".to_string(), AMBER),
            Badge::Failed => ("update failed".to_string(), DANGER),
        };
        let ucolor = if f.hov(Hit::Update) && matches!(f.update, Badge::Ready(_)) {
            TEXT
        } else {
            ucolor
        };
        self.text(M, fy, W, F_TINY, ucolor, DT_RIGHT, &utext);
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
