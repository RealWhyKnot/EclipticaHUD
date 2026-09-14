use super::anim::*;
use super::{play, App, BLIP, TOKENS};
use crate::discord;
use crate::game::run::base_name;
use crate::hud::render::{Env, Frame, LogHit};
use crate::update;
use std::time::Instant;

pub struct Tick {
    pub redraw: bool,
    pub redraw_log: bool,
    pub timer_ms: Option<u32>,
    pub quit: bool,
}

pub(super) const STALE_SECS: u64 = 120;
const VRC_CHECK_SECS: u64 = 2;

impl App {
    pub(super) fn now(&self) -> u64 {
        let elapsed = self.last_ts_at.elapsed().as_secs();
        if self.env() == Env::InWorld && elapsed <= STALE_SECS {
            self.last_ts + elapsed
        } else {
            self.last_ts
        }
    }

    pub fn render(&mut self) {
        let env = self.env();
        let frame = Frame {
            now: self.now(),
            env,
            pages: self.pages(),
            view_page: self.viewed_page(),
            settings_open: self.settings_open,
            scale: self.scale,
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
            log_ok: env != Env::NoVrchat && self.watch.path.is_some(),
            log_open: self.log.visible,
            progress_shown: self.progress_shown,
            dps_frac_shown: self.dps_frac_shown,
            taken_frac_shown: self.taken_frac_shown,
            dps_shown: self.dps_shown,
            fight_dps_shown: self.fight_dps_shown,
            taken_shown: self.taken_shown,
            taken_rate_shown: self.taken_rate_shown,
            update: self.badge.clone(),
            sound_on: self.sound_on,
            topmost: self.topmost,
            discord: self.link,
            discord_on: self.discord_on,
            view_run: self.shown_run().map(|(i, _)| i),
            view_group: self.viewed_group().map(|(i, _)| i),
            group_pages: self.group_pages(),
            run_sel: self.sel_run.is_some(),
            group_sel: self.sel_group.is_some(),
        };
        self.renderer.draw_main(&self.gs, &frame);
        self.renderer.rgba(&mut self.rgba);
        self.vr.submit(
            &self.rgba,
            self.renderer.width as u32,
            self.renderer.height as u32,
            true,
            env == Env::InWorld,
            self.alpha as f32 / 100.0,
            self.scale,
        );
    }

    pub fn tick(&mut self) -> Tick {
        let rotated = self.pump_log();
        let (env, env_changed) = self.poll_env();
        let now = self.now();
        self.cue_effects(now);
        let services_changed = self.poll_services(now);
        let anim = self.animate(env, now);
        let dead = self.dead_since.is_some();
        let tip = self.tip();
        let tip_changed = tip != self.last_tip;
        self.last_tip = tip;
        let redraw = tip_changed
            || self.gs.changed
            || anim
            || (env == Env::InWorld && self.gs.boss.is_some())
            || services_changed
            || env_changed
            || dead != self.was_dead;
        self.was_dead = dead;

        let changed = self.gs.changed;
        self.gs.changed = false;
        let (log_anim, redraw_log) = if self.log.visible {
            self.tick_log(rotated, env_changed, changed)
        } else {
            (false, false)
        };
        self.primed = true;
        if redraw {
            self.render();
        } else {
            self.vr.submit(
                &self.rgba,
                self.renderer.width as u32,
                self.renderer.height as u32,
                false,
                env == Env::InWorld,
                self.alpha as f32 / 100.0,
                self.scale,
            );
        }
        let want: u32 = if anim || log_anim { ANIM_MS } else { 1000 };
        let timer_ms = (want != self.timer_ms).then_some(want);
        self.timer_ms = want;
        Tick {
            redraw,
            redraw_log,
            timer_ms,
            quit: update::restart_pending(),
        }
    }

    fn pump_log(&mut self) -> bool {
        let gs = &mut self.gs;
        let mut newest = 0u64;
        let rotated = self.watch.poll(|line| {
            if let Some(ts) = gs.feed(line) {
                newest = newest.max(ts);
            }
        });
        if rotated {
            self.gs.log_rotated();
        }
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
            self.vrc_running = crate::vr::process_running("VRChat.exe");
        }
        let env = self.env();
        let env_changed = self.last_env.is_some_and(|e| e != env);
        self.last_env = Some(env);
        (env, env_changed)
    }

    fn cue_effects(&mut self, now: u64) {
        if self.gs.target_since != self.last_target_since {
            self.last_target_since = self.gs.target_since;
            if self.gs.target.is_some() {
                self.flash_at = Some(Instant::now());
                if self.sound_on {
                    play(BLIP);
                }
            }
        }
        let stage_key = self.gs.stage_stats.start_ts;
        if self.warned_stage != stage_key
            && self.gs.wave_idle(now)
            && self.gs.tokens_missing().is_some()
        {
            self.warned_stage = stage_key;
            self.warn_at = Some(Instant::now());
            if self.sound_on {
                play(TOKENS);
            }
        }
        let taken_seq = self.gs.taken.back().map_or(0, |e| e.seq);
        if taken_seq != self.last_taken_seq {
            self.last_taken_seq = taken_seq;
            if self.primed && taken_seq != 0 {
                self.taken_flash_at = Some(Instant::now());
            }
        }
        let dps_base = self.gs.boss.as_deref().map(base_name);
        if dps_base != self.dps_boss.as_deref() {
            self.dps_boss = dps_base.map(str::to_string);
            self.dps_peak = 0;
            self.taken_peak = 0;
        }
    }

    fn poll_services(&mut self, now: u64) -> bool {
        self.feed_presence(now);
        let link = discord::link();
        let link_changed = link != self.link;
        self.link = link;
        let badge = update::badge();
        let badge_changed = badge != self.badge;
        if badge_changed {
            self.badge = badge;
        }
        link_changed || badge_changed
    }

    fn animate(&mut self, env: Env, now: u64) -> bool {
        let rolling = self.gs.rolling_dps(now);
        self.dps_peak = self.dps_peak.max(rolling);
        let dps_target = if self.gs.boss.is_some() && self.dps_peak > 0 {
            rolling as f32 / self.dps_peak as f32
        } else {
            0.0
        };
        let rolling_taken = self.gs.rolling_taken(now);
        self.taken_peak = self.taken_peak.max(rolling_taken);
        let taken_target = if self.gs.boss.is_some() && self.taken_peak > 0 {
            rolling_taken as f32 / self.taken_peak as f32
        } else {
            0.0
        };
        let tip_pending = self.hover.is_some() && self.tip().is_none();
        let mut anim = approach(&mut self.progress_shown, self.gs.progress, 0.002);
        anim |= tip_pending;
        anim |= approach(&mut self.dps_frac_shown, dps_target, 0.002);
        anim |= approach(&mut self.taken_frac_shown, taken_target, 0.002);
        anim |= approach(&mut self.dps_shown, rolling as f32, 0.5);
        anim |= approach(
            &mut self.fight_dps_shown,
            self.gs.fight_dps(now) as f32,
            0.5,
        );
        anim |= approach(&mut self.taken_shown, rolling_taken as f32, 0.5);
        anim |= approach(
            &mut self.taken_rate_shown,
            self.gs.fight_taken_rate(now) as f32,
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

    fn tick_log(&mut self, rotated: bool, env_changed: bool, changed: bool) -> (bool, bool) {
        let log_tip_pending = self.log.hover.is_some() && self.log_tip().is_none();
        let rows = self.log_rows();
        if rows != self.log.rows {
            if rows > self.log.rows && self.primed && self.log.scroll_shown < 1.0 {
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
        self.log.alpha =
            (crate::hud::render::ease_out_cubic(fade) * self.alpha_byte() as f32) as u8;
        log_anim |= fade < 1.0;
        log_anim |= log_tip_pending;
        let log_tip = self.log_tip();
        let log_tip_changed = log_tip != self.last_log_tip;
        self.last_log_tip = log_tip;
        let redraw_log = changed || log_anim || rotated || log_tip_changed || env_changed;
        if redraw_log {
            self.render_log();
        }
        (log_anim, redraw_log)
    }

    pub fn nudge_timer(&mut self) {
        self.timer_ms = ANIM_MS;
    }
}
