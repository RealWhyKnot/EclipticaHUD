use super::*;

#[test]
fn control_colours_meet_contrast_minimums() {
    for surface in [BG, CARD, CARD_HI] {
        assert!(contrast(EDGE, surface) >= 3.0, "{surface:06x}");
        assert!(contrast(INK_OFF, surface) < 3.0, "{surface:06x}");
    }
    assert!(contrast(TEXT, CTRL_HOVER) >= 4.5);
    assert!(contrast(BG, ACCENT) >= 4.5);
    assert!(contrast(BG, mix(ACCENT, TEXT, 0.2)) >= 4.5);
}

#[test]
fn every_control_state_draws_differently() {
    let btn = (20, 20, 56, 24);
    let mut seen = Vec::new();
    for ctl in [
        Ctl::Off,
        Ctl::Rest,
        Ctl::Hover,
        Ctl::Pressed,
        Ctl::On,
        Ctl::OnHover,
    ] {
        let r = Renderer::new(96, 100, 64);
        r.fill(0, 0, 100, 64, CARD);
        r.text_button(btn, ctl, "Off");
        // SAFETY: GdiFlush takes no arguments and only waits for pending drawing.
        unsafe { GdiFlush() };
        let edge = pix(&r, 20, 32);
        let fill = pix(&r, 24, 24);
        match ctl {
            Ctl::Off => assert_eq!((edge, fill), (CARD, CARD)),
            Ctl::Rest => assert_eq!((edge, fill), (EDGE, CARD)),
            Ctl::Hover => assert_eq!((edge, fill), (EDGE, CTRL_HOVER)),
            Ctl::Pressed | Ctl::On => assert_eq!(fill, ACCENT),
            Ctl::OnHover => assert_eq!(fill, mix(ACCENT, TEXT, 0.2)),
        }
        seen.push(block(&r, 20, 20, 76, 44));
    }
    assert_eq!(seen[3], seen[4]);
    for i in 0..seen.len() {
        for j in i + 1..seen.len() {
            if (i, j) != (3, 4) {
                assert_ne!(seen[i], seen[j], "{i} vs {j}");
            }
        }
    }
}

#[test]
fn empty_text_draws_nothing() {
    let r = Renderer::new(96, 40, 40);
    r.fill(0, 0, 40, 40, BG);
    r.text_rect((0, 0, 40, 40), F_BODY, TEXT, DT_LEFT, "");
    // SAFETY: GdiFlush takes no arguments and only waits for pending drawing.
    unsafe { GdiFlush() };
    assert!(block(&r, 0, 0, 40, 40).iter().all(|&c| c == BG));
}

#[test]
fn ordinals() {
    for (n, s) in [
        (1, "1st"),
        (2, "2nd"),
        (3, "3rd"),
        (4, "4th"),
        (11, "11th"),
        (12, "12th"),
        (13, "13th"),
        (21, "21st"),
        (22, "22nd"),
        (23, "23rd"),
        (32, "32nd"),
        (101, "101st"),
        (111, "111th"),
        (112, "112th"),
    ] {
        assert_eq!(ordinal(n), s);
    }
}

#[test]
fn board_keeps_you_in_view() {
    let idx = |p: &PartyPane| p.board().iter().map(|(i, _)| *i).collect::<Vec<_>>();
    assert_eq!(idx(&ranked(5, Some(2))), [0, 1, 2, 3, 4]);
    assert_eq!(idx(&ranked(32, Some(3))), (0..10).collect::<Vec<_>>());
    assert_eq!(idx(&ranked(32, Some(9))), (0..10).collect::<Vec<_>>());
    let mut tail: Vec<usize> = (0..9).collect();
    tail.push(10);
    assert_eq!(idx(&ranked(32, Some(10))), tail);
    tail[9] = 31;
    assert_eq!(idx(&ranked(32, Some(31))), tail);
    assert_eq!(idx(&ranked(32, None)), (0..10).collect::<Vec<_>>());
    assert!(ranked(32, Some(31)).board()[9].1.you);
}

#[test]
fn gap_line_names_the_nearest_rival() {
    let two = |a: (&str, bool, u64), b: (&str, bool, u64)| {
        pane(vec![
            party_row(a.0, a.1, a.2, 0, None, 0),
            party_row(b.0, b.1, b.2, 0, None, 0),
        ])
    };
    assert_eq!(
        two(("Alice", false, 1300), ("Me", true, 100))
            .gap_line()
            .as_deref(),
        Some("1.20K behind Alice")
    );
    assert_eq!(
        two(("Me", true, 300), ("Bob", false, 100))
            .gap_line()
            .as_deref(),
        Some("200 ahead of Bob")
    );
    assert_eq!(
        two(("Me", true, 300), ("Bob", false, 300))
            .gap_line()
            .as_deref(),
        Some("tied with Bob")
    );
    assert_eq!(
        two(("Alice", false, 300), ("Bob", false, 100)).gap_line(),
        None
    );
    assert_eq!(ranked(1, Some(0)).gap_line(), None);
}

#[test]
fn status_words_follow_the_link() {
    let mut p = PartyPane::default();
    assert_eq!(p.status(), ("off", DIM));
    p.on = true;
    assert_eq!(p.status(), ("waiting for Ecliptica", DIM));
    p.ready = true;
    for (link, word, color) in [
        (Link::Off, "connecting", AMBER),
        (Link::Connecting, "connecting", AMBER),
        (Link::Connected, "connected", GOOD),
        (Link::Retrying, "can't reach the server", DANGER),
        (Link::Full, "sharing is full", DANGER),
        (Link::Taken, "already sharing", DANGER),
    ] {
        p.link = link;
        assert_eq!(p.status(), (word, color));
    }
}

#[test]
fn party_layout_lines_up() {
    let (c1x, c1y, c1w, c1h) = PARTY_CARD1;
    let (c2x, c2y, c2w, c2h) = PARTY_CARD2;
    assert_eq!((c1x, c2x, c1w, c2w), (M, M, W, W));
    assert_eq!(c1y + c1h + 8, c2y);
    let (ox, oy, ow, oh) = PARTY_OLDER;
    let (px, py, pw, ph) = PARTY_PAGE;
    let (nx, ny, nw, nh) = PARTY_NEWER;
    assert_eq!(c2y + c2h + 8, oy);
    assert_eq!(oy + oh + M, PARTY_H);
    assert_eq!((py, ny, ph, nh), (oy, oy, oh, oh));
    assert_eq!((ox + ow, px + pw), (px, nx));
    assert_eq!(ox, PARTY_W - (nx + nw));
    assert_eq!(c1y, PARTY_CLOSE_HIT.1 + PARTY_CLOSE_HIT.3 + 8);
    let rows_end = PARTY_ROW_Y + crate::num::to_i32(PARTY_ROWS) * PARTY_ROW_H;
    assert!(rows_end <= c2y + c2h - 4, "{rows_end}");
    let cols = [PARTY_RANK, PARTY_NAME, PARTY_BAR, PARTY_DMG, PARTY_TAKEN];
    assert!(cols[0].0 >= c2x + 12);
    for pair in cols.windows(2) {
        assert!(pair[0].0 + pair[0].1 + 6 <= pair[1].0, "{pair:?}");
    }
    assert_eq!(PARTY_TAKEN.0 + PARTY_TAKEN.1, c2x + c2w - 12);
    let (tx, ty, tw, th) = PARTY_TOGGLE;
    assert_eq!(tx + tw + 8, PARTY_CLOSE_BTN.0);
    assert_eq!((ty, th), (PARTY_CLOSE_BTN.1, PARTY_CLOSE_BTN.3));
    assert_eq!(PARTY_CLOSE_HIT.0 + PARTY_CLOSE_HIT.2, PARTY_W);
    assert_eq!((PARTY_W, LOGICAL_W), (360, 360));
}

#[test]
fn party_hit_regions() {
    for dpi in DPIS {
        let r = Renderer::new(dpi, PARTY_W, PARTY_H);
        let at = |x: i32, y: i32, vr: bool| {
            let pointer = if vr { Pointer::Laser } else { Pointer::Mouse };
            r.party_hit_test(r.px(x), r.px(y), pointer, false)
        };
        assert_eq!(at(PARTY_W - 1, 0, false), Some(PartyHit::Close));
        assert_eq!(at(324, 35, false), Some(PartyHit::Close));
        assert_eq!(at(322, 20, false), None);
        let (tx, ty, tw, th) = PARTY_TOGGLE;
        assert_eq!(at(tx, ty, false), Some(PartyHit::Toggle));
        assert_eq!(at(tx + tw - 1, ty + th - 1, false), Some(PartyHit::Toggle));
        assert_eq!(at(tx - 1, ty, false), None);
        assert_eq!(at(300, 50, false), Some(PartyHit::Status));
        assert_eq!(at(100, 90, false), Some(PartyHit::Rank));
        assert_eq!(at(100, 200, false), Some(PartyHit::Board));
        assert_eq!(at(4, 4, false), None);
        assert_eq!(at(100, 20, false), None);
        assert_eq!(at(100, 120, false), None);
        assert_eq!(at(100, PARTY_H - 4, false), None);
        assert_eq!(at(PARTY_W - 1, 0, true), None);
        assert_eq!(at(tx + 4, ty + 4, true), None);
        assert_eq!(at(100, 200, true), Some(PartyHit::Board));
    }
    assert!(PartyHit::Close.clickable() && PartyHit::Toggle.clickable());
    for h in [PartyHit::Status, PartyHit::Rank, PartyHit::Board] {
        assert!(!h.clickable());
    }
    for on in [false, true] {
        for h in [
            PartyHit::Close,
            PartyHit::Toggle,
            PartyHit::Status,
            PartyHit::Rank,
            PartyHit::Board,
        ] {
            let tip = h.tip(on);
            assert!(!tip.is_empty() && tip.chars().count() <= 70, "{tip}");
            assert!(!tip.contains(['\u{2014}', '\u{2013}']), "{tip}");
        }
    }
}

#[test]
fn party_tooltips_fit_the_window() {
    for dpi in DPIS {
        for on in [false, true] {
            for hit in [
                PartyHit::Close,
                PartyHit::Toggle,
                PartyHit::Status,
                PartyHit::Rank,
                PartyHit::Board,
            ] {
                for link in [Link::Connected, Link::Retrying, Link::Full, Link::Taken] {
                    tip_inside(dpi, hit, on, link);
                }
            }
        }
    }
}

#[test]
fn party_window_stress_never_leaves_its_columns() {
    for dpi in DPIS {
        let short = pane(stress_party(Stress::Short));
        let names = pane(stress_party(Stress::Names));
        let huge = pane(stress_party(Stress::Huge));
        let rs = draw(dpi, &short, false);
        let rn = draw(dpi, &names, false);
        let rh = draw(dpi, &huge, false);
        let s = |v: i32| rs.px(v);
        let (y0, y1) = (
            s(PARTY_ROW_Y),
            s(PARTY_ROW_Y + crate::num::to_i32(PARTY_ROWS) * PARTY_ROW_H),
        );
        let cols = |r: &Renderer| {
            [PARTY_BAR, PARTY_DMG, PARTY_TAKEN]
                .iter()
                .map(|&(x, w)| block(r, s(x), y0, s(x + w), y1))
                .collect::<Vec<_>>()
        };
        assert_eq!(cols(&rs), cols(&rn), "long names moved columns at {dpi}");
        for c in [1, 2] {
            assert_ne!(
                cols(&rs)[c],
                cols(&rh)[c],
                "huge numbers not drawn in column {c} at {dpi}"
            );
        }
        let gaps = [
            (PARTY_RANK.0 + PARTY_RANK.1, PARTY_NAME.0, "rank"),
            (PARTY_NAME.0 + PARTY_NAME.1, PARTY_BAR.0, "name"),
            (PARTY_BAR.0 + PARTY_BAR.1, PARTY_DMG.0, "bar"),
            (PARTY_DMG.0 + PARTY_DMG.1, PARTY_TAKEN.0, "dealt"),
            (PARTY_TAKEN.0 + PARTY_TAKEN.1, PARTY_W - M, "taken"),
        ];
        for (r, what) in [(&rs, "short"), (&rn, "names"), (&rh, "huge")] {
            for y in y0..y1 {
                for &(a, b, col) in &gaps {
                    for x in s(a)..s(b) {
                        let c = pix(r, x, y);
                        assert!(!is_ink(c), "{what} {col} ink {c:06x} at {x},{y} ({dpi})");
                    }
                }
            }
            assert_outside_cards_blank(r, &format!("{what} at {dpi}"));
        }
        for row in huge.board() {
            let n = row.1.dmg;
            assert!(
                rs.text_w(F_BODY, &compact(n)) <= s(PARTY_DMG.1),
                "{n} at {dpi}"
            );
            let t = row.1.taken;
            assert!(
                rs.text_w(F_BODY, &compact(t)) <= s(PARTY_TAKEN.1),
                "{t} taken at {dpi}"
            );
        }
        let (cx, _, cw, _) = PARTY_CARD1;
        let rank = ordinal(32);
        let of = "of 32 sharing";
        let need = rs.unscale(rs.text_w(F_BIG, &rank)) + 10 + rs.unscale(rs.text_w(F_BODY, of));
        assert!(need <= cw - 24, "{need} at {dpi}");
        assert!(cx + 12 + need <= PARTY_W - M - 12);
    }
}
