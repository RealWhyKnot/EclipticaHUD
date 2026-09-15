mod boss;
mod deaths;
mod kills;
mod lifecycle;
mod rates;
mod stage;
mod tokens;

use crate::game::event::{fmt_clock, split_line};
use crate::game::run::{merge_tallies, RunEnd, Tally};
use crate::game::state::{GameState, Mode};

const P: &str = "2026.09.07 09:12:28 Debug      -  ";

fn feed_at(gs: &mut GameState, t: &str, msg: &str) -> u64 {
    gs.feed(&format!("2026.09.08 {t} Debug      -  {msg}"))
        .unwrap()
}

const HALL: &str =
    "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer";

const SAVE: &str = "ECLIPTICA saving SESSION ID 2505";

const RESET: &str = "Retiring Enemy POOL ID0";

fn spawn_level(gs: &mut GameState, t: &str, stage: &str) {
    for _ in 0..3 {
        feed_at(gs, t, "spawn token, False, 0");
    }
    feed_at(gs, t, stage);
}

const DEAD: &str = "Local controller dead, switching off.";

const LOBBY: &str = "ECLIPTICA - now in lobby";

fn kill_at(gs: &mut GameState, t: &str, boss: &str, strike: u64) {
    feed_at(gs, t, &format!("Boss {boss} dead, personal damage dealt: "));
    feed_at(gs, t, &format!("STRIKE DMG: {strike}"));
    feed_at(gs, t, "NON-STRIKE DMG: 0");
}
