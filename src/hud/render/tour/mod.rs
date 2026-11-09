use super::tests::{Stress, frame, log_view, pane, party_row, png, sample_party, stress_party};
use super::*;
use crate::discord::Link;
use crate::game::state::GameState;
use crate::hud::flags::Flag;
use crate::hud::timeline::{self, Filter};
use crate::update::Badge;

mod main_pages;
mod party_pages;

use main_pages::main_states;
use party_pages::party_states;

fn line(t: u64, m: &str) -> String {
    format!(
        "2026.10.22 {:02}:{:02}:{:02} Debug      -  {m}",
        16 + t / 3600,
        t / 60 % 60,
        t % 60
    )
}

pub(super) fn tour_gs(me: bool, secs: u64) -> (GameState, u64) {
    let mut gs = GameState::default();
    let mut feed = |t: u64, m: &str| {
        gs.feed(&line(t, m));
    };
    feed(0, "User Authenticated: Me (Alt) (usr_1)");
    feed(0, "[Behaviour] Joining wrld_1:1~hidden(usr_1)~region(use)");
    feed(0, "[Behaviour] OnPlayerJoined Me (Alt) (usr_1)");
    feed(0, "[Behaviour] OnPlayerJoined Be\u{430}r (Hands) (usr_2)");
    feed(0, "[Behaviour] OnPlayerJoined Sora (usr_3)");
    feed(
        0,
        "ECLIPTICA - now in stage: Stage_OuterHeven on phase: 0.4 as class: Blade",
    );
    feed(
        1,
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.4",
    );
    for t in 2..40 {
        feed(t, &format!("Dealing {} STRIKE damage", 900 + t * 37 % 400));
        if t % 4 == 0 {
            feed(
                t,
                "damage has been taken: 140, from source: (Kakarot) attack_Kick",
            );
        }
    }
    feed(40, "Boss Kakarot dead, personal damage dealt: ");
    feed(
        44,
        "ECLIPTICA - now in stage: Stage_OuterHeven on phase: 0.7 as class: Blade",
    );
    feed(
        46,
        "ECLIPTICA - now fighting boss: Hakli(Clone) on phase: 0.7",
    );
    for t in 47..46 + secs {
        feed(t, &format!("Dealing {} STRIKE damage", 700 + t * 53 % 600));
        if t % 3 == 0 {
            feed(
                t,
                "damage has been taken: 95, from source: (Hakli) Edge Sweep",
            );
        }
        if t % 7 == 0 {
            feed(
                t,
                "damage has been taken: 210, from source: (Gildhorn) Rock Slam",
            );
        }
    }
    let who = if me { "Me (Alt)" } else { "Be\u{430}r (Hands)" };
    feed(
        46 + secs,
        &format!("ownership of Hakli transferred to {who}"),
    );
    let now = crate::game::event::split_line(&line(46 + secs, "x"))
        .unwrap()
        .ts;
    (gs, now)
}

fn inter_gs() -> (GameState, u64) {
    let mut gs = GameState::default();
    let mut feed = |t: u64, m: &str| {
        gs.feed(&line(t, m));
    };
    feed(0, "User Authenticated: Me (Alt) (usr_1)");
    feed(0, "[Behaviour] Joining wrld_1:1~hidden(usr_1)~region(use)");
    feed(
        0,
        "ECLIPTICA - now in stage: Stage_OuterHeven on phase: 0.4 as class: Blade",
    );
    feed(
        1,
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.4",
    );
    for t in 2..40 {
        feed(t, &format!("Dealing {} STRIKE damage", 900 + t * 37 % 400));
    }
    feed(40, "Boss Kakarot dead, personal damage dealt: ");
    feed(40, "STRIKE DMG: 900");
    feed(40, "NON-STRIKE DMG: 0");
    feed(44, "ECLIPTICA - now in intermission");
    let now = crate::game::event::split_line(&line(50, "x")).unwrap().ts;
    (gs, now)
}

fn out_dir() -> std::path::PathBuf {
    let tag = std::env::var("EHUD_TOUR").unwrap_or_else(|_| "after".to_string());
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".devflow")
        .join("ui-tour")
        .join(tag);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub(super) fn save(r: &Renderer, name: &str) {
    let (w, h) = (r.width.cast_unsigned(), r.height.cast_unsigned());
    // SAFETY: `bits` points at the DIB section's width * height * 4 bytes, which live as long as the renderer.
    let s = unsafe { std::slice::from_raw_parts(r.bits, crate::num::to_usize(w * h * 4)) };
    let mut rgb = Vec::with_capacity(crate::num::to_usize(w * h * 3));
    for p in s.chunks(4) {
        rgb.extend_from_slice(&[p[2], p[1], p[0]]);
    }
    std::fs::write(out_dir().join(format!("{name}.png")), png(w, h, &rgb)).unwrap();
}

fn history_gs(stages: u64) -> GameState {
    let mut gs = GameState::default();
    let mut feed = |t: u64, m: &str| {
        gs.feed(&line(t, m));
    };
    feed(0, "User Authenticated: Me (Alt) (usr_1)");
    feed(0, "[Behaviour] Joining wrld_1:1~hidden(usr_1)~region(use)");
    for i in 0..stages {
        let (start, phase) = (i * 130, if i % 2 == 0 { "0.2" } else { "0.6" });
        let (boss, hit) = if i % 2 == 0 {
            ("Kakarot", "(Kakarot) attack_Kick")
        } else {
            ("Hakli", "(Hakli) Edge Sweep")
        };
        feed(
            start,
            &format!(
                "ECLIPTICA - now in stage: Stage_OuterHeven on phase: {phase} as class: Blade"
            ),
        );
        for t in start + 1..start + 70 {
            feed(t, &format!("Dealing {} STRIKE damage", 300 + t * 41 % 500));
            if t % 5 == 0 {
                feed(
                    t,
                    "damage has been taken: 80, from source: (Gildhorn) Rock Slam",
                );
            }
        }
        feed(
            start + 72,
            &format!("ECLIPTICA - now fighting boss: {boss}(Clone) on phase: {phase}"),
        );
        for t in start + 73..start + 120 {
            feed(t, &format!("Dealing {} STRIKE damage", 900 + t * 37 % 400));
            if t % 4 == 0 {
                feed(
                    t,
                    &format!("damage has been taken: 140, from source: {hit}"),
                );
            }
        }
        feed(
            start + 120,
            &format!("Boss {boss} dead, personal damage dealt: "),
        );
    }
    gs
}

fn state_gs(kind: u8) -> GameState {
    match kind {
        0 => tour_gs(false, 60).0,
        1 => tour_gs(true, 60).0,
        3 => inter_gs().0,
        7 => history_gs(2),
        8 => history_gs(15),
        9 => {
            let mut gs = history_gs(2);
            gs.feed(&line(
                260,
                "ECLIPTICA - now in stage: Stage_CopiedCity on phase: 0.8 as class: Blade",
            ));
            for t in 261..300 {
                gs.feed(&line(t, "Dealing 420 STRIKE damage"));
            }
            gs
        }
        4..=6 => {
            let mut gs = tour_gs(false, 60).0;
            let dead = "Local controller dead, switching off.";
            if kind != 4 {
                gs.feed(&line(100, dead));
                gs.feed(&line(103, "Dealing 40 STRIKE damage"));
            }
            if kind != 6 {
                gs.feed(&line(106, dead));
            }
            gs
        }
        _ => GameState::default(),
    }
}

#[test]
#[ignore = "writes images for review"]
fn write_ui_tour() {
    for dpi in [96, 192] {
        let sfx = if dpi == 96 { "" } else { "@2x" };
        for (name, kind, f) in main_states() {
            let gs = state_gs(kind);
            let mut r = Renderer::new(dpi, LOGICAL_W, LOGICAL_H);
            r.draw_main(&gs, &f);
            save(&r, &format!("{name}{sfx}"));
        }
        let (gs, _) = tour_gs(false, 60);
        let src = timeline::live(&gs);
        let mut r = Renderer::new(dpi, LOG_W, LOG_H);
        r.draw_log(src, &log_view(Filter::All));
        save(&r, &format!("log_all{sfx}"));
        let mut r = Renderer::new(dpi, LOG_W, LOG_H);
        r.draw_log(None, &log_view(Filter::All));
        save(&r, &format!("log_empty{sfx}"));
        for (name, p) in party_states() {
            for vr in [false, true] {
                let mut r = Renderer::new(dpi, PARTY_W, PARTY_H);
                r.draw_party(&PartyView {
                    pane: &p,
                    hover: None,
                    pressed: None,
                    tip: None,
                    vr,
                });
                let tag = if vr { "_vr" } else { "" };
                save(&r, &format!("party_{name}{tag}{sfx}"));
            }
        }
        let p = pane(sample_party());
        let mut r = Renderer::new(dpi, PARTY_W, PARTY_H);
        r.draw_party(&PartyView {
            pane: &p,
            hover: Some(PartyHit::Toggle),
            pressed: None,
            tip: Some(PartyHit::Toggle),
            vr: false,
        });
        save(&r, &format!("party_hover_toggle{sfx}"));
        let p = PartyPane {
            rows: Vec::new(),
            fight: None,
            link: crate::relay::Link::Taken,
            ..pane(Vec::new())
        };
        let mut r = Renderer::new(dpi, PARTY_W, PARTY_H);
        r.draw_party(&PartyView {
            pane: &p,
            hover: Some(PartyHit::Status),
            pressed: None,
            tip: Some(PartyHit::Status),
            vr: false,
        });
        save(&r, &format!("party_taken_tip{sfx}"));
    }
}
