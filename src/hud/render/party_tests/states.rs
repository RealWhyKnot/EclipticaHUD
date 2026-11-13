use super::*;

#[test]
fn board_count_clears_column_labels() {
    use crate::relay::proto::MAX_MEMBERS;
    for dpi in DPIS {
        let r = Renderer::new(dpi, PARTY_W, PARTY_H);
        let w = |f: usize, s: &str| r.unscale(r.text_w(f, s));
        let count = format!("{MAX_MEMBERS} of {MAX_MEMBERS} here sharing");
        let label = PARTY_DMG.0 + PARTY_DMG.1 - w(F_LABEL, "DEALT");
        for head in ["THIS FIGHT", "LAST FIGHT", "BOSS DAMAGE"] {
            let end = PARTY_CARD2.0 + 12 + w(F_LABEL, head) + 10 + w(F_TINY, &count);
            assert!(
                end + 6 <= label,
                "{head} ends {end}, DEALT at {label}, {dpi}"
            );
        }
    }
}

#[test]
fn damage_column_uses_one_format_per_board() {
    for dpi in DPIS {
        let r = Renderer::new(dpi, PARTY_W, PARTY_H);
        let fits = |n: u64| r.text_w(F_BODY, &group_digits(n)) <= r.px(PARTY_DMG.1);
        assert!(fits(9_999_999), "{dpi}");
        assert!(!fits(u64::MAX), "{dpi}");
        let fits_t = |n: u64| r.text_w(F_BODY, &group_digits(n)) <= r.px(PARTY_TAKEN.1);
        assert!(fits_t(999_999), "{dpi}");
        let mixed = pane(vec![
            party_row("A", false, u64::MAX / 2, 0, None, 0),
            party_row("B", true, 1_234, 0, None, 0),
        ]);
        let only_b = pane(vec![
            party_row("A", false, 0, 0, None, 0),
            party_row("B", true, 1_234, 0, None, 0),
        ]);
        let rm = draw(dpi, &mixed, false);
        let rb = draw(dpi, &only_b, false);
        let row = |r: &Renderer| {
            let y = PARTY_ROW_Y + PARTY_ROW_H;
            block(
                r,
                r.px(PARTY_DMG.0),
                r.px(y),
                r.px(PARTY_DMG.0 + PARTY_DMG.1),
                r.px(y + PARTY_ROW_H),
            )
        };
        assert_ne!(row(&rm), row(&rb), "{dpi}");
    }
}

#[test]
fn your_row_is_marked_and_toggle_shows_state() {
    for dpi in DPIS {
        let p = pane(sample_party());
        let r = draw(dpi, &p, false);
        let at = |x: i32, y: i32| pix(&r, r.px(x), r.px(y));
        let you_y = PARTY_ROW_Y + PARTY_ROW_H;
        assert_eq!(at(PARTY_CARD2.0 + 8, you_y + PARTY_ROW_H / 2), CARD_HI);
        assert_eq!(at(PARTY_CARD2.0 + 8, PARTY_ROW_Y + PARTY_ROW_H / 2), CARD);
        let name_ink = block(
            &r,
            r.px(PARTY_NAME.0),
            r.px(you_y),
            r.px(PARTY_NAME.0 + PARTY_NAME.1),
            r.px(you_y + PARTY_ROW_H),
        );
        assert!(name_ink.contains(&ACCENT), "{dpi}");
        let bar_y = you_y + PARTY_ROW_H / 2 - 1;
        assert_eq!(at(PARTY_BAR.0 + 4, bar_y), ACCENT);
        assert_eq!(at(PARTY_BAR.0 + 4, PARTY_ROW_Y + PARTY_ROW_H / 2 - 1), EDGE);
        let (tx, ty, _, th) = PARTY_TOGGLE;
        assert_eq!(at(tx + 4, ty + th / 2), ACCENT, "on at {dpi}");
        let off = draw(dpi, &PartyPane::default(), false);
        let at_off = |x: i32, y: i32| pix(&off, off.px(x), off.px(y));
        assert_eq!(at_off(tx, ty + th / 2), EDGE, "off edge at {dpi}");
        assert_eq!(at_off(tx + 4, ty + th / 2), BG, "off fill at {dpi}");
        let (bx, by, bw, bh) = PARTY_CLOSE_BTN;
        assert_eq!(at_off(bx, by + bh / 2), EDGE);
        let vr = draw(dpi, &p, true);
        let s = |v: i32| vr.px(v);
        assert!(
            block(&vr, s(tx - 90), s(ty), s(bx + bw), s(by + bh))
                .iter()
                .all(|&c| c == BG)
        );
        let lost = PartyPane {
            link: Link::Retrying,
            ..p
        };
        let rl = draw(dpi, &lost, false);
        let other = |r: &Renderer| {
            block(
                r,
                r.px(PARTY_DMG.0),
                r.px(PARTY_ROW_Y),
                r.px(PARTY_DMG.0 + PARTY_DMG.1),
                r.px(PARTY_ROW_Y + PARTY_ROW_H),
            )
        };
        assert_ne!(other(&r), other(&rl), "stale rows not dimmed at {dpi}");
    }
}

#[test]
fn every_empty_state_says_something_different() {
    let base = |link: Link, ready: bool| PartyPane {
        on: true,
        ready,
        link,
        here: 3,
        fight: None,
        rows: Vec::new(),
        ..PartyPane::default()
    };
    let states = [
        PartyPane::default(),
        base(Link::Off, false),
        base(Link::Connecting, true),
        base(Link::Retrying, true),
        base(Link::Full, true),
        base(Link::Taken, true),
        base(Link::Connected, true),
    ];
    for dpi in DPIS {
        let shots: Vec<Vec<u32>> = states
            .iter()
            .map(|p| {
                let r = draw(dpi, p, false);
                let s = |v: i32| r.px(v);
                assert_outside_cards_blank(&r, &format!("{p:?} at {dpi}"));
                block(&r, 0, s(44), r.width, r.height)
            })
            .collect();
        for i in 0..shots.len() {
            for j in i + 1..shots.len() {
                assert_ne!(shots[i], shots[j], "states {i} and {j} at {dpi}");
            }
        }
    }
}

#[test]
fn off_explainer_fits_its_card() {
    for dpi in DPIS {
        let r = draw(dpi, &PartyPane::default(), false);
        let s = |v: i32| r.px(v);
        let (cx, cy, cw, ch) = PARTY_CARD2;
        let bottom = block(&r, s(cx + 12), s(cy + ch - 10), s(cx + cw - 12), s(cy + ch));
        assert!(
            bottom.iter().all(|&c| c == CARD),
            "text reaches the card bottom at {dpi}"
        );
        let body = block(&r, s(cx + 12), s(cy + 30), s(cx + cw - 12), s(cy + ch - 10));
        assert!(body.iter().any(|&c| is_ink(c)), "{dpi}");
    }
}

#[test]
fn settings_rows_fit_their_labels() {
    let gs = GameState::default();
    for dpi in DPIS {
        let mut r = Renderer::new(dpi, LOGICAL_W, LOGICAL_H);
        let label_w = r.px(SCALE_DOWN_HIT.0 - M - 20);
        for label in [
            "SIZE",
            "OPACITY",
            "STAT WINDOW",
            "AGGRO SOUND",
            "DISCORD STATUS",
            "SHARE FIGHT DATA",
        ] {
            assert!(r.text_w(F_LABEL, label) <= label_w, "{label} at {dpi}");
        }
        for value in ["200%", "100%", "30 s"] {
            assert!(
                r.text_w(F_BOSS, value) <= r.px(SETTING_VALUE.1),
                "{value} at {dpi}"
            );
        }
        for word in ["On", "Off"] {
            assert!(r.text_w(F_LABEL, word) + r.px(16) <= r.px(SOUND_HIT.2));
        }
        r.draw_main(
            &gs,
            &Frame {
                flags: frame()
                    .flags
                    .with(Flag::SettingsOpen, true)
                    .with(Flag::Sound, true)
                    .with(Flag::Discord, false),
                hover: None,

                view_run: None,
                view_group: None,
                ..frame()
            },
        );
        let at = |x: i32, y: i32| pix(&r, r.px(x), r.px(y));
        let (sx, sy, _, sh) = Hit::Sound.rect();
        assert_eq!(at(sx + 4, sy + sh / 2), ACCENT, "sound on at {dpi}");
        let (dx, dy, _, dh) = Hit::Discord.rect();
        assert_eq!(at(dx, dy + dh / 2), EDGE, "discord off at {dpi}");
        assert_eq!(at(dx + 4, dy + dh / 2), CARD_HI, "discord off at {dpi}");
        let (cx, cy, cw, ch) = SETTINGS_CARD;
        assert_eq!(at(cx + cw / 2, cy + BODY_DY + ch - 2), CARD_HI);
        assert_eq!(BOARD_DELETE_HIT.1 + BOARD_DELETE_HIT.3 + 16, cy + ch);
        assert_eq!(SCALE_DOWN_HIT.1 - cy, 16);
    }
}

#[test]
fn titlebar_buttons_are_evenly_spaced_and_outlined() {
    let gs = GameState::default();
    for dpi in DPIS {
        let mut r = Renderer::new(dpi, LOGICAL_W, LOGICAL_H);
        r.draw_main(
            &gs,
            &Frame {
                hover: None,
                view_run: None,
                view_group: None,
                ..frame()
            },
        );
        let at = |x: i32, y: i32| pix(&r, r.px(x), r.px(y));
        let btns = [PARTY_BTN, SETTINGS_BTN, PIN_BTN, LOG_BTN, CLOSE_BTN];
        for (bx, by, _, bh) in btns {
            assert_eq!(at(bx, by + bh / 2), EDGE, "{bx} at {dpi}");
        }
        for pair in btns.windows(2) {
            assert_eq!(pair[0].0 + pair[0].2 + 8, pair[1].0);
        }
    }
    let r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    let (px_, py_, pw, ph) = PARTY_HIT;
    assert_eq!(r.hit_test(px_, py_), Some(Hit::Party));
    assert_eq!(r.hit_test(px_ + pw - 1, py_ + ph - 1), Some(Hit::Party));
}

#[test]
fn rank_line_keeps_its_descenders() {
    let inked = |r: &Renderer| -> Vec<Vec<u32>> {
        (0..r.height)
            .map(|y| block(r, 0, y, r.width, y + 1))
            .filter(|row| row.iter().any(|&c| c != BG))
            .collect()
    };
    let mut old_clips = false;
    for dpi in DPIS {
        let r = Renderer::new(dpi, 200, 60);
        let of = "of 32 sharing";
        let vc = DT_LEFT | DT_VCENTER;
        let shot = |h: i32| {
            r.fill(0, 0, 200, 60, BG);
            r.text_rect((0, 30 - h / 2, 200, h), F_BODY, TEXT, vc, of);
            inked(&r)
        };
        let free = shot(60);
        assert_ne!(free.len(), 0);
        assert_eq!(shot(18), free, "{dpi}");
        old_clips |= shot(16) != free;
    }
    assert!(old_clips);
}

#[test]
fn empty_board_messages_fit_and_name_the_fight() {
    let base = |link: Link, ready: bool| PartyPane {
        on: true,
        ready,
        link,
        here: 3,
        fight: None,
        rows: Vec::new(),
        ..PartyPane::default()
    };
    let states = [
        base(Link::Off, false),
        base(Link::Connecting, true),
        base(Link::Off, true),
        base(Link::Retrying, true),
        base(Link::Full, true),
        base(Link::Taken, true),
        base(Link::Connected, true),
    ];
    let (_, _, cw, _) = PARTY_CARD2;
    for dpi in DPIS {
        let r = Renderer::new(dpi, PARTY_W, PARTY_H);
        for p in &states {
            let msg = p.board_empty().unwrap();
            assert!(r.text_w(F_BODY, msg) <= r.px(cw - 24), "{msg} at {dpi}");
        }
    }
    let fight = |live: bool| PartyFight {
        name: "Ha'kli".to_string(),
        secs: 83,
        live,
        down: !live,
    };
    let mut p = base(Link::Connected, true);
    assert_eq!(p.board_head(), "BOSS DAMAGE");
    p.fight = Some(fight(true));
    assert_eq!(p.board_head(), "THIS FIGHT");
    p.fight = Some(fight(false));
    assert_eq!(p.board_head(), "LAST FIGHT");
    p.rows = sample_party();
    assert_eq!(p.board_empty(), None);
}

#[test]
fn page_label_fits_five_digit_counts() {
    for dpi in DPIS {
        let r = Renderer::new(dpi, PARTY_W, PARTY_H);
        let need = r.unscale(r.text_w(F_TINY, "99999/99999"));
        assert!(need + 4 <= PARTY_PAGE.2, "{dpi}: {need}");
    }
}

#[test]
fn usage_question_fits_at_every_dpi() {
    for dpi in [96, 120, 144, 192] {
        let r = Renderer::new(dpi, LOGICAL_W, LOGICAL_H);
        let w = |f: usize, s: &str| r.unscale(r.text_w(f, s));
        let room = W - 24;
        for line in ASK_LINES {
            crate::console::say(format_args!(
                "{dpi} body {} of {room}: {line}",
                w(F_BODY, line)
            ));
            assert!(w(F_BODY, line) <= room, "{dpi}: {line}");
        }
        let title = w(F_BIG, "Share fight data?");
        let note = w(F_TINY, "You can change this in settings");
        let no = w(F_BODY, "No thanks");
        let yes = w(F_BODY, "Share");
        crate::console::say(format_args!(
            "{dpi} title {title} note {note} no {no} of {} yes {yes}",
            USAGE_NO_HIT.2
        ));
        assert!(title <= room && note <= room, "{dpi}");
        assert!(
            no + 16 <= USAGE_NO_HIT.2 && yes + 16 <= USAGE_YES_HIT.2,
            "{dpi}"
        );
    }
    let bottom = USAGE_YES_HIT.1 + USAGE_YES_HIT.3 + 10 + 20;
    assert!(bottom <= PROMPT_AREA.0 + PROMPT_AREA.1, "{bottom}");
    assert!(ASK_T >= PROMPT_AREA.0, "{ASK_T}");
    assert!(USAGE_YES_HIT.0 + USAGE_YES_HIT.2 < USAGE_NO_HIT.0);
}

#[test]
fn dead_tag_leaves_room_for_the_name() {
    for dpi in DPIS {
        let r = Renderer::new(dpi, PARTY_W, PARTY_H);
        let tag = r.text_w(F_LABEL, "DEAD") + r.px(12);
        let room = r.px(PARTY_NAME.1) - tag - r.px(6);
        assert!(room >= r.px(55), "{dpi}");
    }
}
