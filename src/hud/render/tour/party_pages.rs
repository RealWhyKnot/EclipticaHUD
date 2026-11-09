use super::*;

pub(super) fn party_states() -> Vec<(&'static str, PartyPane)> {
    let mut all = party_live_states();
    all.extend(party_history_states());
    all
}

fn party_live_states() -> Vec<(&'static str, PartyPane)> {
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
        51_000,
        2_300,
        Some(100),
        1_200,
    )]);
    let four = pane(sample_party()[..4].to_vec());
    let mut big = pane(stress_party(Stress::Huge));
    big.here = 40;
    let lost = PartyPane {
        link: Link::Retrying,
        ..pane(sample_party())
    };
    let mut dead = pane(sample_party());
    dead.rows[2].life = 2;
    dead.rows[3].life = 3;
    let mut dead_you = pane(sample_party());
    dead_you.rows[1].life = 2;
    dead_you.rows[0].life = 3;
    dead_you.rows[0].name = "Leonhard von Ashcombe-Whitley".to_string();
    vec![
        ("off", off),
        ("deaths", dead),
        ("deaths_you", dead_you),
        ("not_in_world", not_ready),
        ("connecting", connecting),
        ("waiting", waiting),
        ("alone", alone),
        ("four", four),
        ("thirty_two", big),
        ("link_lost", lost),
        (
            "lost_empty",
            PartyPane {
                rows: Vec::new(),
                fight: None,
                link: crate::relay::Link::Retrying,
                ..pane(Vec::new())
            },
        ),
        (
            "full_empty",
            PartyPane {
                rows: Vec::new(),
                fight: None,
                link: crate::relay::Link::Full,
                ..pane(Vec::new())
            },
        ),
        (
            "taken_empty",
            PartyPane {
                rows: Vec::new(),
                fight: None,
                link: crate::relay::Link::Taken,
                ..pane(Vec::new())
            },
        ),
    ]
}

fn party_history_states() -> Vec<(&'static str, PartyPane)> {
    use crate::relay::Link;
    let four = pane(sample_party()[..4].to_vec());
    let mut down = pane(sample_party());
    down.fight = Some(PartyFight {
        name: "Ha'kli".to_string(),
        secs: 194,
        live: false,
        down: true,
    });
    vec![
        ("after_kill", down.clone()),
        ("hist_live", PartyPane { pages: 11, ..four }),
        (
            "hist_saved",
            PartyPane {
                saved: Some(saved_at()),
                page: 4,
                pages: 11,
                ..down.clone()
            },
        ),
        (
            "hist_saved_off",
            PartyPane {
                on: false,
                link: Link::Off,
                ready: false,
                saved: Some(saved_at()),
                page: 11,
                pages: 11,
                ..down
            },
        ),
        (
            "hist_off",
            PartyPane {
                pages: 3,
                ..PartyPane {
                    here: 6,
                    ..PartyPane::default()
                }
            },
        ),
        (
            "hist_waiting",
            PartyPane {
                rows: Vec::new(),
                fight: None,
                pages: 3,
                ..pane(Vec::new())
            },
        ),
        (
            "hist_huge",
            PartyPane {
                pages: 9998,
                page: 0,
                ..pane(stress_party(Stress::Huge))
            },
        ),
    ]
}

fn saved_at() -> u64 {
    crate::game::event::split_line("2026.12.28 23:59:00 Debug      -  x")
        .unwrap()
        .ts
}
