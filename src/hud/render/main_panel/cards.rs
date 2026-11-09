use super::super::Renderer;
use super::super::fmt::{fmt_ago, fmt_anim, fmt_dur, fmt_run_deaths, group_digits};
use super::super::frame::Frame;
use super::super::layout::{GRAPH_DY, GRAPH_RECT, M, TAKEN_CARD_H, TAKEN_CARD_Y, W};
use super::super::theme::{AMBER, CARD, DANGER, DIM, F_BODY, F_TINY, TEXT, ease_out_cubic, mix};
use super::{DASH, Live, LiveTaken, MISSED_FROM, avg_hit, fmt_deaths, live_kind, or_dash, or_none};
use crate::game::names::boss_name;
use crate::game::run::{FightGroup, merge_tallies};
use crate::game::source::describe_source;
use crate::game::state::GameState;
use windows_sys::Win32::Graphics::Gdi::{DT_LEFT, DT_RIGHT, DT_VCENTER};

impl Renderer {
    pub(super) fn draw_dealt_card(&self, f: &Frame, gs: &GameState, hist: Option<&FightGroup>) {
        let now = f.now;
        self.rround(M, 186, W, 92 + GRAPH_DY, 8, CARD);
        self.card_graph(f, gs, hist, GRAPH_RECT.1, false);
        let line_y = 288;
        if f.live() {
            self.dealt_live(f, gs, line_y);
        } else if let Some(h) = hist.filter(|h| h.missed) {
            self.stat(192, 0, "DMG", DASH, DIM);
            self.stat(192, 1, "DPS", DASH, DIM);
            self.stat(192, 2, "TIME", &fmt_dur(h.duration(now)), AMBER);
            self.text((M + 12, line_y, W - 24), F_BODY, DIM, DT_LEFT, MISSED_FROM);
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
                    self.text((M + 12, line_y, W - 24), F_BODY, TEXT, DT_LEFT, &line);
                }
                None => {
                    self.text(
                        (M + 12, line_y, W - 24),
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
                (M + 12, line_y, W - 24),
                F_BODY,
                DIM,
                DT_LEFT,
                "no boss fight",
            );
        } else {
            self.text((M + 12, 208, W - 24), F_BODY, DIM, DT_LEFT, "no data");
        }
    }

    fn dealt_live(&self, f: &Frame, gs: &GameState, line_y: i32) {
        let now = f.now;
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
                self.text((M + 12, line_y, W - 24), F_BODY, TEXT, DT_LEFT, &line);
            }
            None => self.text(
                (M + 12, line_y, W - 24),
                F_BODY,
                DIM,
                DT_LEFT,
                "no kills yet",
            ),
        }
    }

    pub(super) fn draw_taken_card(&self, f: &Frame, gs: &GameState, hist: Option<&FightGroup>) {
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
                Some(last) => {
                    let (who, attack) = describe_source(&last.source);
                    let line = format!("last hit  {}   {who}   {attack}", last.amount);
                    self.text_rect(
                        (M + 12, line_y, W - 80, 18),
                        F_BODY,
                        TEXT,
                        DT_LEFT | DT_VCENTER,
                        &line,
                    );
                    self.text_rect(
                        (M + 12, line_y, W - 24, 18),
                        F_TINY,
                        DIM,
                        DT_RIGHT | DT_VCENTER,
                        &fmt_ago(now, last.ts),
                    );
                }
                None => self.text(
                    (M + 12, line_y, W - 24),
                    F_BODY,
                    DIM,
                    DT_LEFT,
                    "no hits yet",
                ),
            }
            self.small_stat(small_y, 0, "HITS", &t.hits);
            self.small_stat(small_y, 1, "BIGGEST HIT", &t.big);
            self.small_stat(small_y, 2, "AVG HIT", &t.avg);
            let deaths = gs.live_run().map_or(0, |r| r.deaths);
            let note = (!f.empty() && deaths > 0).then(|| fmt_run_deaths(deaths));
            self.breakdown(list_y, &t.attacks, note.as_deref(), t.empty);
        } else if hist.is_some_and(|h| h.missed) {
            self.stat(stat_y, 0, "TAKEN", DASH, DIM);
            self.stat(stat_y, 1, "DTPS", DASH, DIM);
            self.stat(stat_y, 2, "HITS", DASH, DIM);
            self.card_graph(f, gs, hist, top + 54, true);
            self.small_stat(small_y, 0, "BIGGEST HIT", DASH);
            self.small_stat(small_y, 1, "AVG HIT", DASH);
            self.breakdown(list_y, &[], None, DASH);
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
            self.text(
                (M + 12, stat_y + 16, W - 24),
                F_BODY,
                DIM,
                DT_LEFT,
                "no data",
            );
        }
    }

    fn deaths_line(&self, y: i32, deaths: u32) {
        match deaths {
            0 => self.text((M + 12, y, W - 24), F_BODY, DIM, DT_LEFT, "no deaths"),
            n => self.text((M + 12, y, W - 24), F_BODY, TEXT, DT_LEFT, &fmt_deaths(n)),
        }
    }
}
