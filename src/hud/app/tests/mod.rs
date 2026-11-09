use super::anim::*;
use super::logwin::*;
use super::settings::*;
use super::tick::*;
use super::*;
use crate::game::run::{BossFight, Page, Run, RunEnd};
use crate::hud::flags::Flag;
use crate::hud::render::{
    BOARD_DELETE_HIT, BOARD_HIT, BODY_DY, MUTE_HIT, SPLIT_ROWS, UPDATE_LATER_HIT, UPDATE_NOW_HIT,
    UPDATE_SKIP_HIT, USAGE_NO_HIT, USAGE_YES_HIT,
};
use crate::hud::timeline;
use crate::relay::proto::Totals;
use std::io::Write as _;

const P: &str = "2026.09.07 09:12:28 Debug      -  ";
const ENTER: &str = "[Behaviour] Entering Room: Ecliptica - Demo Playtest";

#[derive(Default)]
struct FakeRelay {
    calls: std::sync::Mutex<Vec<(Option<crate::relay::Want>, Option<Totals>)>>,
}

impl crate::relay::Relay for FakeRelay {
    fn sync(&'static self, want: Option<crate::relay::Want>, dmg: Option<Totals>) {
        self.calls.lock().unwrap().push((want, dmg));
    }

    fn poll(&self) -> crate::relay::Poll {
        crate::relay::Poll {
            link: crate::relay::Link::Off,
            session: 0,
            snaps: Vec::new(),
            runs: Vec::new(),
        }
    }
}

impl FakeRelay {
    fn take(&self) -> Vec<(Option<crate::relay::Want>, Option<Totals>)> {
        std::mem::take(&mut *self.calls.lock().unwrap())
    }
}

fn fake_relay(app: &mut App) -> &'static FakeRelay {
    let fake: &'static FakeRelay = Box::leak(Box::default());
    app.party.relay = fake;
    fake
}

fn headless() -> App {
    let mut app = App::new(96);
    fake_relay(&mut app);
    app.watch = LogWatch::default();
    app.gs.feed(&format!("{P}{ENTER}"));
    app.gs.changed = false;
    app
}

const STAGE_A: &str =
    "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Blade";
const STAGE_B: &str =
    "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Twinmage";

mod party_tests;

mod pages;
mod replay;
mod windows;

fn snapshot(gs: &GameState) -> String {
    format!(
        "{:?}
{:?}
{:?}",
        gs.runs, gs.taken, gs.history
    )
}

fn temp_logs(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ehud_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const DEATHS: &str = "2026.01.01 10:00:00 Debug      -  [Behaviour] Entering Room: Ecliptica - Demo Playtest
2026.01.01 10:00:01 Debug      -  ECLIPTICA - now in stage: Stage_Forest on phase: 0.2 as class: Blade
2026.01.01 10:00:05 Debug      -  ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.2
2026.01.01 10:00:06 Debug      -  Dealing 25 STRIKE damage
2026.01.01 10:00:07 Debug      -  damage has been taken: 40, from source: Kakarot
2026.01.01 10:00:08 Debug      -  Local controller dead, switching off.
2026.01.01 10:00:09 Debug      -  some unrelated line
2026.01.01 10:00:10 Debug      -  Local controller dead, switching off.
2026.01.01 10:00:11 Debug      -  Local controller dead, switching off.
2026.01.01 10:00:20 Debug      -  Dealing 38 NON-STRIKE damage
2026.01.01 10:00:30 Debug      -  Boss Kakarot dead, personal damage dealt: 
2026.01.01 10:05:00 Debug      -  last unrelated line
";

fn kept_of(text: &str, old: bool) -> Vec<(u64, String)> {
    let mut gs = GameState::default();
    let mut kept = Vec::new();
    for line in text.lines() {
        if let Some((ts, Some(at))) = gs.feed_kept(line) {
            let msg = &line[at..];
            let world = matches!(
                crate::game::event::parse_msg(msg),
                Some(crate::game::event::Event::WorldLine(_))
            );
            if !(old && world) {
                kept.push((ts, msg.to_string()));
            }
        }
    }
    kept
}
