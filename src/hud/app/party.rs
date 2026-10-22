use super::App;
use crate::hud::render::PartyPane;
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
        let pane = PartyPane {
            on: self.party_on,
            link,
            rows: if self.party_on {
                p.table.view(now)
            } else {
                Vec::new()
            },
        };
        let changed = pane.on != p.pane.on || pane.link != p.pane.link || pane.rows != p.pane.rows;
        p.pane = pane;
        changed
    }
}
