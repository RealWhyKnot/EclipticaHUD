use super::Renderer;
use super::fmt::{fmt_anim, group_digits};
use super::frame::{Frame, RunSync};
use super::hit::{Hit, TipCtx};
use super::layout::{
    BODY_DY, CLOSE_BTN, DISCORD_DOT, GRAPH_DY, LOG_BTN, LOGICAL_H, LOGICAL_W, M, PARTY_BTN,
    PIN_BTN, SETTINGS_BTN, STAGES_BTN, UPDATE_HIT, W,
};
use super::theme::{
    ACCENT, AMBER, BG, DANGER, DIM, F_BIG, F_BODY, F_BOSS, F_LABEL, F_TINY, GLYPH_CLOSE, GLYPH_LOG,
    GLYPH_PARTY, GLYPH_PIN, GLYPH_SETTINGS, GLYPH_TABLE, GLYPH_UNPIN, GOOD, TEXT, ease_out_cubic,
    lerp, mix,
};
use crate::discord::Link;
use crate::game::run::{Run, Tally};
use crate::game::state::{GameState, Mode};
use crate::hud::flags::{Flag, Flags};
use crate::update::{Badge, VERSION};
use crate::vr::VrStatus;
use windows_sys::Win32::Graphics::Gdi::{DT_CENTER, DT_LEFT, DT_RIGHT, DT_VCENTER, GdiFlush};

mod boss;
mod cards;
mod settings;
mod splits;
mod status;

const DASH: &str = "-";
pub(super) const SYNCING: &str = "syncing with party";
pub(super) const SYNCED: &str = "synced with party";

fn sync_tag(f: &Frame) -> Option<(&'static str, u32)> {
    match f.sync {
        RunSync::Waiting => Some((SYNCING, ACCENT)),
        RunSync::Done => Some((SYNCED, GOOD)),
        RunSync::Idle => None,
    }
}

pub(super) const MISSED: &str = "before you joined";
pub(super) const MISSED_FROM: &str = "result and time from your party";
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
    if f.empty() { DASH.to_string() } else { s }
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
        (taken / u64::from(hits)).to_string()
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
    let fit = crate::num::to_usize((STRIP_W + STRIP_GAP) / (STRIP_MIN + STRIP_GAP));
    let shown = n.min(fit).max(1);
    let start = cur.saturating_sub(shown / 2).min(n.saturating_sub(shown));
    let k = crate::num::to_i32(shown);
    let sw = ((STRIP_W - STRIP_GAP * (k - 1)) / k).min(40);
    (start..(start + shown).min(n), sw)
}

impl Renderer {
    pub(crate) fn draw_main(&mut self, gs: &GameState, f: &Frame) {
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
            if f.flags.has(Flag::Stages) {
                self.draw_splits(f, gs);
            } else {
                self.draw_dealt_card(f, gs, hist.as_ref());
            }
            if f.flags.has(Flag::StagesButton) && !f.flags.has(Flag::SettingsOpen) {
                let ctl = f.ctl(Hit::Stages).lit(f.flags.has(Flag::Stages));
                self.glyph_button(STAGES_BTN, ctl, GLYPH_TABLE);
            }
        });
        if !f.flags.has(Flag::Stages) {
            self.shifted(GRAPH_DY + BODY_DY, || {
                self.draw_taken_card(f, gs, hist.as_ref());
            });
        }
        if f.flags.has(Flag::SettingsOpen) {
            self.shifted(BODY_DY, || self.draw_settings(f));
        }
        self.draw_footer(f);
        if f.flags.has(Flag::PromptOpen) {
            self.draw_update_prompt(f);
        } else if f.flags.has(Flag::UsageAsk) {
            self.draw_usage_prompt(f);
        }
        if f.warn_t < 1.0 {
            self.draw_token_warning(f, gs);
        }
        if let Some(hit) = f.tip {
            let ctx = TipCtx {
                flags: Flags::default()
                    .with(Flag::Live, f.live())
                    .with(Flag::Topmost, f.flags.has(Flag::Topmost))
                    .with(Flag::Sound, f.flags.has(Flag::Sound))
                    .with(Flag::LogOpen, f.flags.has(Flag::LogOpen))
                    .with(Flag::Discord, f.flags.has(Flag::Discord))
                    .with(Flag::Usage, f.flags.has(Flag::Usage))
                    .with(Flag::UsageFailing, f.flags.has(Flag::UsageFailing))
                    .with(Flag::Board, f.flags.has(Flag::Board))
                    .with(Flag::PartyOpen, f.flags.has(Flag::PartyOpen))
                    .with(Flag::Stages, f.flags.has(Flag::Stages)),
                board_status: f.board_status,
                board_delete: f.board_delete,
            };
            self.tooltip(hit.rect(), hit.tip(ctx), LOGICAL_W, LOGICAL_H);
        }
        // SAFETY: GdiFlush takes no arguments and only waits for pending drawing.
        unsafe { GdiFlush() };
    }

    fn stat(&self, y: i32, i: i32, label: &str, value: &str, color: u32) {
        let x = M + 12 + i * COL;
        self.text((x, y, COL), F_LABEL, DIM, DT_LEFT, label);
        self.text((x, y + 16, COL), F_BOSS, color, DT_LEFT, value);
    }

    fn small_stat(&self, y: i32, i: i32, label: &str, value: &str) {
        let x = M + 12 + i * COL;
        self.text((x, y, COL), F_LABEL, DIM, DT_LEFT, label);
        self.text((x, y + 15, COL), F_BODY, TEXT, DT_LEFT, value);
    }

    fn draw_titlebar(&self, f: &Frame) {
        self.text_rect(
            (M, 8, W, 24),
            F_LABEL,
            ACCENT,
            DT_LEFT | DT_VCENTER,
            "ECLIPTICA HUD",
        );
        let free = !f.flags.has(Flag::PromptOpen);
        self.glyph_button(
            PARTY_BTN,
            f.ctl(Hit::Party).lit(f.flags.has(Flag::PartyOpen)),
            GLYPH_PARTY,
        );
        self.glyph_button(
            SETTINGS_BTN,
            f.ctl(Hit::Settings)
                .lit(f.flags.has(Flag::SettingsOpen))
                .usable(free),
            GLYPH_SETTINGS,
        );
        self.glyph_button(
            PIN_BTN,
            f.ctl(Hit::Pin),
            if f.flags.has(Flag::Topmost) {
                GLYPH_PIN
            } else {
                GLYPH_UNPIN
            },
        );
        self.glyph_button(
            LOG_BTN,
            f.ctl(Hit::Log).lit(f.flags.has(Flag::LogOpen)),
            GLYPH_LOG,
        );
        self.glyph_button(CLOSE_BTN, f.ctl(Hit::Close), GLYPH_CLOSE);
    }

    fn draw_footer(&self, f: &Frame) {
        let fy = LOGICAL_H - 26;
        let vr_color = match f.vr {
            VrStatus::On => GOOD,
            VrStatus::Off => DIM,
            VrStatus::Failing => DANGER,
        };
        self.dot(M, fy + 4, 8, vr_color);
        self.text((M + 14, fy, 80), F_TINY, DIM, DT_LEFT, "STEAMVR");
        let log_color = if f.flags.has(Flag::LogOk) {
            GOOD
        } else {
            AMBER
        };
        self.dot(M + 76, fy + 4, 8, log_color);
        self.text((M + 90, fy, 80), F_TINY, DIM, DT_LEFT, "LOG");
        let discord_color = match (f.flags.has(Flag::Discord), f.discord) {
            (false, _) | (true, Link::Off) => DIM,
            (true, Link::Waiting) => AMBER,
            (true, Link::Connected) => GOOD,
            (true, Link::Rejected) => DANGER,
        };
        let (dx, _, _, _) = DISCORD_DOT;
        self.dot(dx + 4, fy + 4, 8, discord_color);
        self.text((dx + 18, fy, 60), F_TINY, DIM, DT_LEFT, "DISCORD");
        let (utext, ucolor) = match &f.update {
            Badge::Ready(_) if f.flags.has(Flag::PromptOpen) => (VERSION.to_string(), DIM),
            Badge::None => (VERSION.to_string(), DIM),
            Badge::Ready(tag) => (format!("update {tag}"), ACCENT),
            Badge::Installing(_) => ("updating".to_string(), AMBER),
            Badge::Failed(_) => ("update failed".to_string(), DANGER),
        };
        if matches!(f.update, Badge::Ready(_) | Badge::Installing(_))
            && !f.flags.has(Flag::PromptOpen)
        {
            let color = if f.hov(Hit::Update) { TEXT } else { ucolor };
            let x = UPDATE_HIT.0;
            self.link(
                (x, fy - 3, LOGICAL_W - M - x, 20),
                F_TINY,
                color,
                DT_RIGHT,
                &utext,
            );
        } else {
            self.text((M, fy, W), F_TINY, ucolor, DT_RIGHT, &utext);
        }
    }

    fn draw_token_warning(&self, f: &Frame, gs: &GameState) {
        let (got, total) = gs.tokens_shown().unwrap_or((0, 0));
        let t = f.warn_t;
        let slide_in = ease_out_cubic((t / 0.08).min(1.0));
        let slide_out = ease_out_cubic(((t - 0.88) / 0.12).max(0.0));
        let y = crate::num::to_i32(lerp(-140.0, 96.0, slide_in) - slide_out * 236.0);
        let pulse = 0.5 + 0.5 * (t * std::f32::consts::TAU * 3.0).sin();
        self.rround(M, y, W, 128, 10, mix(AMBER, TEXT, pulse * 0.6));
        self.rround(M + 3, y + 3, W - 6, 122, 8, mix(BG, AMBER, 0.18));
        let vc = DT_CENTER | DT_VCENTER;
        self.text_rect((M, y + 14, W, 40), F_BIG, AMBER, vc, "COLLECT YOUR TOKENS");
        let line = format!("{got}/{total} picked up");
        self.text_rect((M, y + 58, W, 28), F_BOSS, TEXT, vc, &line);
        let hint = "grab the rest before you summon the boss";
        self.text_rect((M, y + 88, W, 24), F_BODY, DIM, vc, hint);
    }
}
