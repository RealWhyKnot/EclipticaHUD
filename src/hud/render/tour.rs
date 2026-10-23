use super::tests::{frame, log_view, pane, party_row, png, sample_party, stress_party};
use super::*;
use crate::discord::Link;
use crate::game::state::GameState;
use crate::hud::timeline::{self, Filter};
use crate::update::Badge;

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
    let (w, h) = (r.width as u32, r.height as u32);
    let s = unsafe { std::slice::from_raw_parts(r.bits, (w * h * 4) as usize) };
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for p in s.chunks(4) {
        rgb.extend_from_slice(&[p[2], p[1], p[0]]);
    }
    std::fs::write(out_dir().join(format!("{name}.png")), png(w, h, &rgb)).unwrap();
}

fn state_gs(kind: u8) -> GameState {
    match kind {
        0 => tour_gs(false, 60).0,
        1 => tour_gs(true, 60).0,
        _ => GameState::default(),
    }
}

fn main_states() -> Vec<(&'static str, u8, Frame)> {
    let now = tour_gs(false, 60).1;
    let now_you = tour_gs(true, 60).1;
    let live = || Frame {
        now,
        hover: None,
        view_run: Some(0),
        view_group: Some(1),
        group_pages: 2,
        discord_on: true,
        discord: Link::Connected,
        vr: crate::vr::VrStatus::On,
        ..frame()
    };
    vec![
        ("main_fight", 0, live()),
        (
            "main_fight_you_muted",
            1,
            Frame {
                now: now_you,
                sound_on: false,
                ..live()
            },
        ),
        (
            "main_settings",
            0,
            Frame {
                settings_open: true,
                ..live()
            },
        ),
        (
            "main_hover_settings",
            0,
            Frame {
                hover: Some(Hit::Settings),
                ..live()
            },
        ),
        (
            "main_party_open",
            0,
            Frame {
                party_open: true,
                log_open: true,
                ..live()
            },
        ),
        (
            "main_settings_hover_sound",
            1,
            Frame {
                now: now_you,
                settings_open: true,
                sound_on: false,
                hover: Some(Hit::Sound),
                ..live()
            },
        ),
        (
            "main_history",
            0,
            Frame {
                run_sel: true,
                group_sel: true,
                view_group: Some(0),
                pages: 2,
                view_page: 0,
                ..live()
            },
        ),
        (
            "main_first_launch",
            2,
            Frame {
                env: Env::InWorld,
                view_run: None,
                view_group: None,
                group_pages: 0,
                discord_on: false,
                discord: Link::Off,
                ..live()
            },
        ),
        (
            "main_prompt",
            2,
            Frame {
                env: Env::InWorld,
                view_run: None,
                view_group: None,
                group_pages: 0,
                update: Badge::Ready("v2026.10.29.0".to_string()),
                prompt: true,
                ..live()
            },
        ),
    ]
}

#[test]
#[ignore]
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
    }
}

fn party_states() -> Vec<(&'static str, PartyPane)> {
    use crate::relay::Link;
    let off = PartyPane {
        here: 6,
        ..PartyPane::default()
    };
    let not_ready = PartyPane {
        on: true,
        ..PartyPane::default()
    };
    let connecting = PartyPane {
        rows: Vec::new(),
        fight: None,
        link: Link::Connecting,
        ..pane(Vec::new())
    };
    let waiting = PartyPane {
        rows: Vec::new(),
        fight: None,
        ..pane(Vec::new())
    };
    let alone = pane(vec![party_row(
        "Me (Alt)",
        true,
        true,
        51_000,
        Some(100),
        1_200,
    )]);
    let four = pane(sample_party()[..4].to_vec());
    let mut big = pane(stress_party(true, true));
    big.here = 40;
    let lost = PartyPane {
        link: Link::Retrying,
        ..pane(sample_party())
    };
    let mut down = pane(sample_party());
    down.fight = Some(PartyFight {
        name: "Ha'kli".to_string(),
        secs: 194,
        live: false,
        down: true,
    });
    vec![
        ("off", off),
        ("not_in_world", not_ready),
        ("connecting", connecting),
        ("waiting", waiting),
        ("alone", alone),
        ("four", four),
        ("thirty_two", big),
        ("link_lost", lost),
        ("after_kill", down),
    ]
}
