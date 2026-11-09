use super::fmt::{compact, group_digits};
use super::layout::*;
use super::party_panel::ordinal;
use super::tests::{Stress, frame, pane, party_row, sample_party, stress_party};
use super::theme::*;
use super::*;
use crate::game::state::GameState;
use crate::hud::flags::Flag;
use crate::relay::Link;
use windows_sys::Win32::Graphics::Gdi::*;

mod board;
mod states;

const DPIS: [u32; 4] = [96, 120, 144, 192];

fn pix(renderer: &Renderer, x: i32, y: i32) -> u32 {
    let bytes =
        // SAFETY: `bits` points at the DIB section's width * height * 4 bytes, which live as long as the renderer.
        unsafe { std::slice::from_raw_parts(renderer.bits, crate::num::to_usize(renderer.width * renderer.height * 4)) };
    let index = crate::num::to_usize((y * renderer.width + x) * 4);
    rgb(
        u32::from(bytes[index + 2]),
        u32::from(bytes[index + 1]),
        u32::from(bytes[index]),
    )
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
        let x = f64::from(v) / 255.0;
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

fn ranked(n: usize, you: Option<usize>) -> PartyPane {
    pane(
        (0..n)
            .map(|i| {
                party_row(
                    &format!("P{i}"),
                    Some(i) == you,
                    1000 - crate::num::to_u64(i),
                    10 * crate::num::to_u64(i),
                    None,
                    0,
                )
            })
            .collect(),
    )
}

fn tip_inside(dpi: u32, hit: PartyHit, on: bool, link: Link) {
    let p = PartyPane {
        on,
        link,
        ..pane(sample_party())
    };
    let tip = p.tip(hit);
    assert!(!tip.is_empty() && tip.chars().count() <= 70, "{tip}");
    assert!(!tip.contains(['\u{2014}', '\u{2013}']), "{tip}");
    let mut r = Renderer::new(dpi, PARTY_W, PARTY_H);
    r.draw_party(&PartyView {
        pane: &p,
        hover: Some(hit),
        pressed: None,
        tip: Some(hit),
        vr: false,
    });
    let (w, lines) = r.tip_lines(tip, PARTY_W);
    assert!(w <= PARTY_W - 16, "{hit:?} at {dpi}");
    assert!(lines.len() <= 2, "{hit:?} at {dpi}");
    for line in &lines {
        assert!(
            r.text_w(F_TINY, line) + r.px(TIP_INSET) <= r.px(w),
            "{line}"
        );
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
