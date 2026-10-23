use super::anim::*;
use super::App;
use crate::game::names::boss_name;
use crate::game::run::base_name;
use crate::game::state::GameState;
use crate::hud::render::{
    ease_out_cubic, tip_ready, Env, PartyFight, PartyHit, PartyPane, PartyView,
};
use crate::relay::party::Party;
use crate::relay::proto::{member_hash, room_hash, Member, Room};
use crate::relay::{self, Link, Poll, Relay, Want};
use std::time::Instant;

pub(super) struct PartyState {
    table: Party,
    pane: PartyPane,
    epoch: Instant,
    session: u64,
    room: Option<String>,
    room_hash: Option<Room>,
    own_usr: Option<String>,
    own: Option<Member>,
    fight: u64,
    roster_seq: Option<u64>,
    pub(super) relay: &'static dyn Relay,
}

impl PartyState {
    pub(super) fn new() -> Self {
        PartyState {
            table: Party::default(),
            pane: PartyPane::default(),
            epoch: Instant::now(),
            session: 0,
            room: None,
            room_hash: None,
            own_usr: None,
            own: None,
            fight: 0,
            roster_seq: None,
            relay: &relay::HUB,
        }
    }

    #[cfg(test)]
    pub(super) fn pane(&self) -> &PartyPane {
        &self.pane
    }

    fn ms(&self, at: Instant) -> u64 {
        at.saturating_duration_since(self.epoch).as_millis() as u64
    }
}

impl App {
    pub fn set_party(&mut self, on: bool) {
        self.party_on = on;
        if !on {
            self.party.table.clear();
        }
    }

    pub fn poll_party(&mut self) -> bool {
        if !self.party_on && !self.party.pane.on {
            return false;
        }
        let in_world = self.vrc_running && self.gs.in_ecliptica();
        let want = relay::want(
            self.party_on,
            in_world,
            self.gs.location.as_deref(),
            self.gs.me_usr.as_deref(),
        );
        let dmg = want.as_ref().and(self.gs.relay_dmg());
        let relay = self.party.relay;
        relay.sync(want.clone(), dmg);
        let poll = relay.poll();
        self.party_step(Instant::now(), want.as_ref(), Some(poll))
    }

    pub(super) fn party_step(
        &mut self,
        at: Instant,
        want: Option<&Want>,
        poll: Option<Poll>,
    ) -> bool {
        let clock = self.now();
        let gs = &self.gs;
        let p = &mut self.party;
        let now = p.ms(at);
        let room = gs.location.as_deref().filter(|l| l.starts_with("wrld_"));
        let moved = room != p.room.as_deref();
        if moved {
            p.table.reset_room();
            p.room = room.map(str::to_string);
            p.room_hash = room.map(room_hash);
            p.roster_seq = None;
        }
        if moved || p.own_usr != gs.me_usr {
            p.own_usr = gs.me_usr.clone();
            p.own = room
                .zip(gs.me_usr.as_deref())
                .map(|(loc, usr)| member_hash(loc, usr));
        }
        if gs.fight_start != p.fight {
            if gs.fight_start != 0 {
                p.table.clear();
            }
            p.fight = gs.fight_start;
        }
        if p.roster_seq != Some(gs.roster_seq) {
            p.roster_seq = Some(gs.roster_seq);
            let names = match room {
                Some(loc) => gs
                    .roster
                    .iter()
                    .map(|(usr, name)| (member_hash(loc, usr), name.clone()))
                    .collect(),
                None => Vec::new(),
            };
            p.table.set_names(names);
        }
        let (link, session, snaps) = match poll {
            Some(poll) => (poll.link, poll.session, poll.snaps),
            None => (Link::Off, p.session, Vec::new()),
        };
        if p.pane.link == Link::Connected && (link != Link::Connected || session != p.session) {
            p.table.disconnected(now);
        }
        p.session = session;
        let same_room = want.is_some_and(|w| room == Some(w.location.as_str()));
        if link == Link::Connected && same_room {
            for snap in snaps {
                if Some(snap.room) == p.room_hash {
                    p.table.apply_snap(p.ms(snap.at), &snap.entries, p.own);
                }
            }
        }
        if let Some(own) = p.own.filter(|_| self.party_on) {
            p.table.set_own(now, own, gs.relay_dmg());
        }
        let mut rows = if self.party_on {
            p.table.view(now)
        } else {
            Vec::new()
        };
        let me = gs.me.as_deref().unwrap_or("You");
        for row in rows.iter_mut().filter(|r| r.you) {
            row.name = me.to_string();
        }
        let pane = PartyPane {
            on: self.party_on,
            ready: want.is_some(),
            link,
            here: gs.roster.len(),
            fight: party_fight(gs, clock),
            rows,
        };
        let changed = pane != p.pane;
        p.pane = pane;
        changed
    }

    pub fn party_toggle(&mut self) {
        if self.party_win.visible {
            self.party_close();
        } else {
            self.party_open();
        }
    }

    pub fn party_open(&mut self) {
        let w = &mut self.party_win;
        w.visible = true;
        w.alpha = 0;
        w.alpha_shown = 0;
        w.opened_at = Some(Instant::now());
    }

    pub fn party_close(&mut self) {
        let w = &mut self.party_win;
        w.visible = false;
        w.hover = None;
        w.pressed = None;
    }

    pub fn party_hit(&self, x: i32, y: i32) -> Option<PartyHit> {
        self.party_win.renderer.party_hit_test(x, y, false)
    }

    pub fn party_set_hover(&mut self, over: Option<PartyHit>) -> bool {
        let w = &mut self.party_win;
        if over == w.hover {
            return false;
        }
        w.hover = over;
        w.hover_at = over.map(|_| Instant::now());
        true
    }

    pub fn party_tip(&self) -> Option<PartyHit> {
        let w = &self.party_win;
        w.hover.filter(|_| tip_ready(w.hover_at))
    }

    pub fn party_activate(&mut self, hit: PartyHit) -> bool {
        match hit {
            PartyHit::Close => {
                self.party_close();
                true
            }
            PartyHit::Toggle => {
                self.set_party(!self.party_on);
                self.poll_party();
                true
            }
            PartyHit::Status | PartyHit::Rank | PartyHit::Board => false,
        }
    }

    pub(super) fn party_vr_shown(&self) -> bool {
        self.party_win.visible && self.party_on && self.env() == Env::InWorld
    }

    pub fn render_party(&mut self) {
        let view = PartyView {
            pane: &self.party.pane,
            hover: self.party_win.hover,
            pressed: self.party_win.pressed,
            tip: self.party_tip(),
            vr: false,
        };
        self.party_win.renderer.draw_party(&view);
    }

    fn render_party_vr(&mut self) {
        let view = PartyView {
            pane: &self.party.pane,
            hover: None,
            pressed: None,
            tip: None,
            vr: true,
        };
        let w = &mut self.party_win;
        w.renderer.draw_party(&view);
        w.renderer.rgba(&mut w.vr_rgba);
    }

    pub(super) fn tick_party(&mut self, changed: bool) -> (bool, bool) {
        let vr_shown = self.party_vr_shown();
        let w = &self.party_win;
        let tip_pending = w.hover.is_some() && self.party_tip().is_none();
        let fade = timed(w.opened_at, FADE_MS);
        let alpha = (ease_out_cubic(fade) * self.alpha_byte() as f32) as u8;
        let tip = self.party_tip();
        let tip_changed = tip != w.last_tip;
        let anim = w.visible && (fade < 1.0 || tip_pending);
        let redraw = changed || anim || tip_changed;
        let vr_frame = vr_shown && (changed || self.party_win.vr_rgba.is_empty());
        if vr_frame {
            self.render_party_vr();
        }
        let w = &mut self.party_win;
        w.alpha = alpha;
        w.last_tip = tip;
        let visible = w.visible;
        if visible && (redraw || vr_frame) {
            self.render_party();
        }
        let w = &self.party_win;
        self.vr.submit_party(
            &w.vr_rgba,
            w.renderer.width as u32,
            w.renderer.height as u32,
            vr_frame,
            vr_shown,
            self.alpha as f32 / 100.0,
            self.scale,
        );
        (anim, visible && (redraw || vr_frame))
    }
}

fn party_fight(gs: &GameState, now: u64) -> Option<PartyFight> {
    if gs.fight_start == 0 {
        return None;
    }
    let last = gs.runs.last().and_then(|r| r.fights.last());
    let name = gs.boss.as_deref().or(last.map(|f| f.name.as_str()))?;
    let live = gs.boss.is_some();
    let end = match (live, last.and_then(|f| f.end_ts)) {
        (false, Some(end)) => end,
        _ => now,
    };
    Some(PartyFight {
        name: boss_name(base_name(name)).to_string(),
        secs: end.saturating_sub(gs.fight_start),
        live,
        down: !live && last.is_some_and(|f| f.end_ts.is_some()),
    })
}
