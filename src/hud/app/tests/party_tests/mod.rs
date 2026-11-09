use super::*;
use crate::relay::proto::{Totals, member_hash, room_hash};
use crate::relay::{self, Link, Poll, Snap, Want};
use std::time::{Duration, Instant};

mod fights;
mod sharing;

const W1: &str = "[Behaviour] Joining wrld_0fb88df3-2057-4c2f-8e06-e948864378fd:98473~hidden(usr_me)~region(use)";
const W2: &str = "[Behaviour] Joining wrld_0fb88df3-2057-4c2f-8e06-e948864378fd:11111~hidden(usr_me)~region(use)";

fn feed(app: &mut App, ts: &str, line: &str) {
    app.gs
        .feed(&format!("2026.09.07 {ts} Debug      -  {line}"));
}

fn party_app() -> (App, Want) {
    let mut app = headless();
    feed(&mut app, "09:12:29", "User Authenticated: Me (usr_me)");
    feed(&mut app, "09:12:30", W1);
    feed(
        &mut app,
        "09:12:31",
        "[Behaviour] OnPlayerJoined Me (usr_me)",
    );
    feed(
        &mut app,
        "09:12:31",
        "[Behaviour] OnPlayerJoined Alice (usr_a)",
    );
    app.set_party(true);
    let want = relay::want(true, app.gs.location.as_deref(), app.gs.me_usr.as_deref()).unwrap();
    (app, want)
}

fn poll(link: Link, session: u64, snaps: Vec<Snap>) -> Poll {
    Poll {
        link,
        session,
        snaps,
        runs: Vec::new(),
    }
}

fn snap(at: Instant, want: &Want, entries: &[(&str, u64)]) -> Snap {
    Snap {
        at,
        room: want.room(),
        entries: entries
            .iter()
            .map(|(usr, d)| (member_hash(&want.location, usr), Totals::from((*d, 0))))
            .collect(),
    }
}

fn rows(app: &App) -> Vec<(String, bool, bool, u64)> {
    app.party
        .pane()
        .rows
        .iter()
        .map(|r| (r.name.clone(), r.you, r.live, r.dmg))
        .collect()
}

fn there(name: &str, dmg: u64) -> (String, bool, bool, u64) {
    (name.to_string(), false, true, dmg)
}

fn gone(name: &str, dmg: u64) -> (String, bool, bool, u64) {
    (name.to_string(), false, false, dmg)
}

fn mine(dmg: u64) -> (String, bool, bool, u64) {
    ("Me".to_string(), true, true, dmg)
}
