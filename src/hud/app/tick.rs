use super::anim::{
    ANIM_MS, DELETE_ARM, FADE_MS, FLASH_MS, SLIDE_MS, TAKEN_FLASH_MS, THUMB_HOLD_MS, WARN_MS,
    approach, approach_rate, timed,
};
use super::{App, BLIP, TOKENS, YOU, play};
use crate::discord;
use crate::hud::flags::{Flag, Flags};
use crate::hud::render::{Env, Frame, LogHit};
use crate::update;
use std::time::Instant;

#[derive(Clone, Copy)]
pub(crate) struct Redraw {
    pub main: bool,
    pub log: bool,
    pub party: bool,
}

pub(crate) struct Tick {
    pub redraw: Redraw,
    pub timer_ms: Option<u32>,
    pub quit: bool,
}

const VRC_CHECK_SECS: u64 = 2;

impl App {
    pub(super) fn now(&self) -> u64 {
        if self.env() == Env::InWorld {
            self.last_ts + self.last_ts_at.elapsed().as_secs()
        } else {
            self.last_ts
        }
    }

    pub(crate) fn render(&mut self) {
        if self.flags.has(Flag::Stages) {
            self.split_track();
        }
        let env = self.env();
        let frame = Frame {
            now: self.now(),
            env,
            pages: self.pages(),
            view_page: self.viewed_page(),
            flags: Flags::default()
                .with(Flag::SettingsOpen, self.flags.has(Flag::SettingsOpen))
                .with(
                    Flag::LogOk,
                    env != Env::NoVrchat && self.watch.path.is_some(),
                )
                .with(Flag::LogOpen, self.log.visible)
                .with(Flag::PromptOpen, self.prompt_shown())
                .with(Flag::Sound, self.flags.has(Flag::Sound))
                .with(Flag::Topmost, self.flags.has(Flag::Topmost))
                .with(Flag::Discord, self.flags.has(Flag::Discord))
                .with(Flag::Usage, self.flags.has(Flag::Usage))
                .with(Flag::UsageFailing, self.flags.has(Flag::UsageFailing))
                .with(Flag::Board, self.flags.has(Flag::Board))
                .with(Flag::UsageAsk, self.usage_prompt_shown())
                .with(Flag::PartyOpen, self.party_win.visible)
                .with(Flag::RunSelected, self.sel_run.is_some())
                .with(Flag::GroupSelected, self.sel_group.is_some())
                .with(Flag::StagesButton, self.stages_ok())
                .with(Flag::Stages, self.stages_shown()),
            scale: self.shown_scale(),
            scale_max: self.scale_max(),
            alpha: self.alpha,
            window: self.gs.win(),
            flash_t: timed(self.flash_at, FLASH_MS),
            taken_flash_t: timed(self.taken_flash_at, TAKEN_FLASH_MS),
            warn_t: timed(self.warn_at, WARN_MS),
            dead_pulse: self.dead_pulse(),
            hover: self.hover,
            pressed: self.pressed,
            tip: self.tip(),
            vr: self.vr.status(),

            progress_shown: self.progress_shown,
            dps_shown: self.dps_shown,
            fight_dps_shown: self.fight_dps_shown,
            taken_shown: self.taken_shown,
            taken_rate_shown: self.taken_rate_shown,
            update: self.badge.clone(),

            discord: self.link,
            board_status: self.board_status,
            board_delete: if self.flags.has(Flag::BoardDeleting) {
                crate::hud::render::DELETE_BUSY
            } else if self.board_armed.is_some() {
                crate::hud::render::DELETE_ARMED
            } else {
                0
            },

            view_run: self.shown_run().map(|(i, _)| i),
            view_group: self.viewed_group().map(|(i, _)| i),
            group_pages: self.group_pages(),

            split_top: self.split_top,
            sync: self.run_sync(std::time::Instant::now()),
        };
        self.renderer.draw_main(&self.gs, &frame);
        self.renderer.rgba(&mut self.rgba);
        self.vr.submit(&crate::vr::Panel {
            rgba: &self.rgba,
            width: self.renderer.width.cast_unsigned(),
            height: self.renderer.height.cast_unsigned(),
            changed: true,
            visible: env == Env::InWorld,
            alpha: f32::from(self.alpha) / 100.0,
            scale: self.scale,
        });
    }

    pub(crate) fn tick(&mut self) -> Tick {
        let rotated = self.pump_log();
        let (env, env_changed) = self.poll_env();
        let now = self.now();
        self.gs.poll_token_alert(now);
        self.cue_effects();
        let services_changed = self.poll_services(now);
        let party_changed = self.poll_party();
        let (party_anim, redraw_party) = self.tick_party(party_changed);
        let anim = self.animate(env, now);
        let dead = self.dead_since.is_some();
        let tip = self.tip();
        let tip_changed = tip != self.last_tip;
        self.last_tip = tip;
        let sync = self.run_sync(std::time::Instant::now());
        let sync_changed = sync != self.last_sync;
        self.last_sync = sync;
        let redraw = tip_changed
            || sync_changed
            || self.gs.changed
            || anim
            || (env == Env::InWorld && self.gs.live_run().is_some())
            || services_changed
            || env_changed
            || dead != self.flags.has(Flag::WasDead);
        self.flags.set(Flag::WasDead, dead);

        let changed = self.gs.changed;
        self.gs.changed = false;
        let (log_anim, redraw_log) = if self.log.visible {
            self.tick_log(rotated || env_changed || changed)
        } else {
            (false, false)
        };
        self.flags.set(Flag::Primed, true);
        if redraw {
            self.render();
        } else {
            self.vr.submit(&crate::vr::Panel {
                rgba: &self.rgba,
                width: self.renderer.width.cast_unsigned(),
                height: self.renderer.height.cast_unsigned(),
                changed: false,
                visible: env == Env::InWorld,
                alpha: f32::from(self.alpha) / 100.0,
                scale: self.scale,
            });
        }
        let want: u32 = if anim || log_anim || party_anim {
            ANIM_MS
        } else {
            1000
        };
        let timer_ms = (want != self.timer_ms).then_some(want);
        self.timer_ms = want;
        Tick {
            redraw: Redraw {
                main: redraw,
                log: redraw_log,
                party: redraw_party,
            },
            timer_ms,
            quit: update::restart_pending() && self.can_restart(),
        }
    }

    pub(crate) fn board_header(&self) -> crate::board::Header {
        let gs = &self.gs;
        crate::board::Header {
            name: gs.me.clone().unwrap_or_default(),
            party: crate::num::to_u32(gs.roster.len().max(1)),
            room: gs
                .location
                .as_deref()
                .map_or_else(String::new, |l| crate::usage::hash(&["eboard room", l], 2)),
            share_name: self.flags.has(Flag::Board),
        }
    }

    fn pump_log(&mut self) -> bool {
        let gs = &mut self.gs;
        let mut newest = 0u64;
        let mut kept = Vec::new();
        let rotated = self.watch.poll(|line| {
            if let Some((ts, keep)) = gs.feed_kept(line) {
                newest = newest.max(ts);
                if let Some(at) = keep {
                    kept.push((ts, line[at..].to_string()));
                }
            }
        });
        let w = &self.watch;
        self.archive.flush(w.path.as_deref(), w.pass, &kept, newest);
        crate::board::feed(&kept, || self.board_header());
        if rotated {
            self.gs.log_rotated();
        }
        self.archive.settle(self.gs.live_run().is_some());
        if newest > self.last_ts {
            self.last_ts = newest;
            self.last_ts_at = Instant::now();
        }
        rotated
    }

    fn poll_env(&mut self) -> (Env, bool) {
        if cfg!(not(test))
            && self
                .vrc_checked
                .is_none_or(|t| t.elapsed().as_secs() >= VRC_CHECK_SECS)
        {
            self.vrc_checked = Some(Instant::now());
            self.flags
                .set(Flag::VrcRunning, crate::process::running("VRChat.exe"));
        }
        let env = self.env();
        let env_changed = self.last_env.is_some_and(|e| e != env);
        self.last_env = Some(env);
        (env, env_changed)
    }

    fn cue_effects(&mut self) {
        if self.gs.target_since != self.last_target_since {
            self.last_target_since = self.gs.target_since;
            if self.gs.target.is_some() {
                self.flash_at = Some(Instant::now());
                if self.flags.has(Flag::Sound) {
                    play(if self.gs.targets_me() { YOU } else { BLIP });
                }
            }
        }
        if self.gs.token_alerts != self.token_alerts_seen {
            self.token_alerts_seen = self.gs.token_alerts;
            if self.flags.has(Flag::Primed) {
                self.warn_at = Some(Instant::now());
                if self.flags.has(Flag::Sound) {
                    play(TOKENS);
                }
            }
        }
        let taken_seq = self.gs.taken.back().map_or(0, |e| e.seq);
        if taken_seq != self.last_taken_seq {
            self.last_taken_seq = taken_seq;
            if self.flags.has(Flag::Primed) && taken_seq != 0 {
                self.taken_flash_at = Some(Instant::now());
            }
        }
    }

    fn poll_services(&mut self, now: u64) -> bool {
        self.feed_presence(now);
        let link = discord::link();
        let link_changed = link != self.link;
        self.link = link;
        let failing = self.flags.has(Flag::Usage)
            && crate::usage::FAILING.load(std::sync::atomic::Ordering::Relaxed);
        let failing_changed = failing != self.flags.has(Flag::UsageFailing);
        self.flags.set(Flag::UsageFailing, failing);
        let board = crate::board::STATUS.load(std::sync::atomic::Ordering::Relaxed);
        let board_changed = board != self.board_status;
        self.board_status = board;
        let deleting = self.flags.has(Flag::BoardDeleting)
            && crate::board::DELETING.load(std::sync::atomic::Ordering::Relaxed);
        let expired = self.board_armed.is_some_and(|t| t.elapsed() >= DELETE_ARM);
        if expired {
            self.board_armed = None;
        }
        let delete_changed = expired || deleting != self.flags.has(Flag::BoardDeleting);
        self.flags.set(Flag::BoardDeleting, deleting);
        let badge_changed = self.poll_badge();
        link_changed || failing_changed || board_changed || delete_changed || badge_changed
    }

    pub(crate) fn poll_badge(&mut self) -> bool {
        let badge = update::badge();
        let badge_changed = badge != self.badge;
        if badge_changed {
            if matches!(badge, update::Badge::Ready(_)) && self.badge == update::Badge::None {
                self.flags.set(Flag::PromptOpen, true);
            }
            self.badge = badge;
        }
        badge_changed
    }

    fn animate(&mut self, env: Env, now: u64) -> bool {
        let rolling = self.gs.rolling_dps(now);
        let rolling_taken = self.gs.rolling_taken(now);
        let tip_pending = self.hover.is_some() && self.tip().is_none();
        let mut anim = approach(&mut self.progress_shown, self.gs.progress, 0.002);
        anim |= tip_pending;
        anim |= approach(&mut self.dps_shown, crate::num::to_f32(rolling), 0.5);
        anim |= approach(
            &mut self.fight_dps_shown,
            crate::num::to_f32(self.gs.fight_dps(now)),
            0.5,
        );
        anim |= approach(
            &mut self.taken_shown,
            crate::num::to_f32(rolling_taken),
            0.5,
        );
        anim |= approach(
            &mut self.taken_rate_shown,
            crate::num::to_f32(self.gs.fight_taken_rate(now)),
            0.5,
        );
        anim |= self.flash_at.is_some() && timed(self.flash_at, FLASH_MS) < 1.0;
        anim |= self.taken_flash_at.is_some() && timed(self.taken_flash_at, TAKEN_FLASH_MS) < 1.0;
        anim |= self.warn_at.is_some() && timed(self.warn_at, WARN_MS) < 1.0;
        let dead = env == Env::InWorld && self.gs.is_dead(now);
        if dead && self.dead_since.is_none() {
            self.dead_since = Some(Instant::now());
        } else if !dead {
            self.dead_since = None;
        }
        anim | dead
    }

    fn tick_log(&mut self, dirty: bool) -> (bool, bool) {
        let log_tip_pending = self.log.hover.is_some() && self.log_tip().is_none();
        let rows = self.log_rows();
        if rows != self.log.rows {
            if rows > self.log.rows && self.flags.has(Flag::Primed) && self.log.scroll_shown < 1.0 {
                self.log.slide_at = Some(Instant::now());
            }
            self.log.rows = rows;
            self.log_clamp();
        }
        let mut log_anim = approach_rate(&mut self.log.scroll_shown, self.log.scroll, 0.5, 0.4);
        let thumb_lit = self.log.drag.is_some()
            || matches!(self.log.hover, Some(LogHit::Thumb | LogHit::Track))
            || self
                .log
                .thumb_seen
                .is_some_and(|t| t.elapsed().as_millis() < THUMB_HOLD_MS);
        log_anim |= approach(
            &mut self.log.thumb_t,
            if thumb_lit { 1.0 } else { 0.0 },
            0.01,
        );
        log_anim |= self.log.slide_at.is_some() && timed(self.log.slide_at, SLIDE_MS) < 1.0;
        let fade = timed(self.log.opened_at, FADE_MS);
        self.log.alpha = crate::num::to_u8(
            crate::hud::render::ease_out_cubic(fade) * f32::from(self.alpha_byte()),
        );
        log_anim |= fade < 1.0;
        log_anim |= log_tip_pending;
        let log_tip = self.log_tip();
        let log_tip_changed = log_tip != self.last_log_tip;
        self.last_log_tip = log_tip;
        let redraw_log = dirty || log_anim || log_tip_changed;
        if redraw_log {
            self.render_log();
        }
        (log_anim, redraw_log)
    }

    pub(crate) fn nudge_timer(&mut self) {
        self.timer_ms = ANIM_MS;
    }
}
