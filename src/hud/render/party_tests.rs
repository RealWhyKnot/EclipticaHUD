use super::fmt::{compact, group_digits};
use super::layout::*;
use super::party_panel::ordinal;
use super::tests::{frame, pane, party_row, sample_party, stress_party};
use super::theme::*;
use super::*;
use crate::game::state::GameState;
use crate::relay::Link;
use windows_sys::Win32::Graphics::Gdi::*;

const DPIS: [u32; 4] = [96, 120, 144, 192];

fn pix(r: &Renderer, x: i32, y: i32) -> u32 {
    let s = unsafe { std::slice::from_raw_parts(r.bits, (r.width * r.height * 4) as usize) };
    let i = ((y * r.width + x) * 4) as usize;
    rgb(s[i + 2] as u32, s[i + 1] as u32, s[i] as u32)
}

fn block(r: &Renderer, x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<u32> {
    let mut out = Vec::new();
    for y in y0..y1 {
        for x in x0..x1 {
            out.push(pix(r, x, y));
        }
    }
    out
}

fn draw(dpi: u32, p: &PartyPane, vr: bool) -> Renderer {
    let mut r = Renderer::new(dpi, PARTY_W, PARTY_H);
    r.draw_party(&PartyView {
        pane: p,
        hover: None,
        pressed: None,
        tip: None,
        vr,
    });
    r
}

fn luminance(c: u32) -> f64 {
    let ch = |v: u32| {
        let x = v as f64 / 255.0;
        if x <= 0.03928 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * ch(c & 0xff) + 0.7152 * ch((c >> 8) & 0xff) + 0.0722 * ch((c >> 16) & 0xff)
}

fn contrast(a: u32, b: u32) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

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
    r.text_rect(0, 0, 40, 40, F_BODY, TEXT, DT_LEFT, "");
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

fn ranked(n: usize, you: Option<usize>) -> PartyPane {
    pane(
        (0..n)
            .map(|i| {
                party_row(
                    &format!("P{i}"),
                    Some(i) == you,
                    true,
                    1000 - i as u64,
                    10 * i as u64,
                    None,
                    0,
                )
            })
            .collect(),
    )
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
            party_row(a.0, a.1, true, a.2, 0, None, 0),
            party_row(b.0, b.1, true, b.2, 0, None, 0),
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
    let rows_end = PARTY_ROW_Y + PARTY_ROWS as i32 * PARTY_ROW_H;
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
        let at = |x: i32, y: i32, vr: bool| r.party_hit_test(r.px(x), r.px(y), vr, false);
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

fn tip_inside(dpi: u32, hit: PartyHit, on: bool) {
    let p = PartyPane {
        on,
        ..pane(sample_party())
    };
    let mut r = Renderer::new(dpi, PARTY_W, PARTY_H);
    r.draw_party(&PartyView {
        pane: &p,
        hover: Some(hit),
        pressed: None,
        tip: Some(hit),
        vr: false,
    });
    let (w, lines) = r.tip_lines(hit.tip(on), PARTY_W);
    assert!(w <= PARTY_W - 16, "{hit:?} at {dpi}");
    assert!(lines.len() <= 2, "{hit:?} at {dpi}");
    for line in &lines {
        assert!(
            r.text_w(F_TINY, line) + r.px(TIP_INSET) <= r.px(w),
            "{line}"
        );
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
                tip_inside(dpi, hit, on);
            }
        }
    }
}

fn is_ink(c: u32) -> bool {
    c != BG && c != CARD && c != CARD_HI
}

fn assert_outside_cards_blank(r: &Renderer, what: &str) {
    let s = |v: i32| r.px(v);
    let (_, c1y, _, c1h) = PARTY_CARD1;
    let (_, c2y, _, c2h) = PARTY_CARD2;
    for y in s(44)..r.height {
        for x in (0..s(M)).chain(s(PARTY_W - M)..r.width) {
            assert_eq!(pix(r, x, y), BG, "{what}: margin at {x},{y}");
        }
    }
    for y in (s(c1y + c1h)..s(c2y)).chain(s(c2y + c2h)..r.height) {
        for x in 0..r.width {
            assert_eq!(pix(r, x, y), BG, "{what}: between cards at {x},{y}");
        }
    }
}

#[test]
fn party_window_stress_never_leaves_its_columns() {
    for dpi in DPIS {
        let short = pane(stress_party(false, false));
        let names = pane(stress_party(true, false));
        let huge = pane(stress_party(true, true));
        let rs = draw(dpi, &short, false);
        let rn = draw(dpi, &names, false);
        let rh = draw(dpi, &huge, false);
        let s = |v: i32| rs.px(v);
        let (y0, y1) = (
            s(PARTY_ROW_Y),
            s(PARTY_ROW_Y + PARTY_ROWS as i32 * PARTY_ROW_H),
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
            party_row("A", false, true, u64::MAX / 2, 0, None, 0),
            party_row("B", true, true, 1_234, 0, None, 0),
        ]);
        let only_b = pane(vec![
            party_row("A", false, true, 0, 0, None, 0),
            party_row("B", true, true, 1_234, 0, None, 0),
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
        assert!(block(&vr, s(tx - 90), s(ty), s(bx + bw), s(by + bh))
            .iter()
            .all(|&c| c == BG));
        let lost = PartyPane {
            link: Link::Retrying,
            ..p.clone()
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
                settings_open: true,
                hover: None,
                sound_on: true,
                discord_on: false,
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
        assert_eq!(USAGE_HIT.1 + USAGE_HIT.3 + 16, cy + ch);
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
            r.text_rect(0, 30 - h / 2, 200, h, F_BODY, TEXT, vc, of);
            inked(&r)
        };
        let free = shot(60);
        assert!(!free.is_empty());
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
