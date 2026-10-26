use super::enemies::*;
use super::tests::{frame, pix, png};
use super::*;
use crate::discord::Link;
use crate::game::state::GameState;
use windows_sys::Win32::Foundation::SIZE;

const DPIS: [u32; 6] = [48, 72, 96, 120, 144, 192];
const CARD_TOP: i32 = 82;
const CARD_BOTTOM: i32 = 178;

fn line(t: u64, m: &str) -> String {
    format!(
        "2026.10.22 {:02}:{:02}:{:02} Debug      -  {m}",
        16 + t / 3600,
        t / 60 % 60,
        t % 60
    )
}

fn ts(t: u64) -> u64 {
    crate::game::event::split_line(&line(t, "x")).unwrap().ts
}

const CASES: [(&str, usize, Option<u64>); 4] = [
    ("0", 1, Some(296)),
    ("3", 3, None),
    ("999plus", 1000, None),
    ("clear", 1, Some(250)),
];

fn stage_gs(spawns: usize, retire: Option<u64>) -> GameState {
    let mut gs = GameState::default();
    let mut feed = |t: u64, m: &str| {
        gs.feed(&line(t, m));
    };
    feed(0, "User Authenticated: Me (Alt) (usr_1)");
    feed(0, "[Behaviour] Joining wrld_1:1~hidden(usr_1)~region(use)");
    feed(
        1,
        "ECLIPTICA - now in stage: Stage_OuterHeven on phase: 0.2 as class: Blade",
    );
    feed(
        60,
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.2",
    );
    feed(120, "Boss Kakarot dead, personal damage dealt: ");
    feed(125, "ECLIPTICA - now in intermission");
    for id in 0..20 {
        feed(126, &format!("Retiring Enemy POOL ID{id}"));
    }
    for _ in 0..3 {
        feed(129, "spawn token, False, 0");
    }
    feed(
        130,
        "ECLIPTICA - now in stage: Stage_CopiedCity on phase: 0.8 as class: Blade",
    );
    for slot in 0..spawns {
        feed(
            140 + slot as u64 / 10,
            &format!("Initializing Enemy POOL ID{slot} as ENEMY ID 1"),
        );
    }
    if let Some(t) = retire {
        feed(t, "Retiring Enemy POOL ID0");
    }
    for t in 260..300 {
        feed(t, "Dealing 420 STRIKE damage");
    }
    gs
}

fn stage_frame() -> Frame {
    Frame {
        now: ts(300),
        view_run: Some(0),
        view_group: None,
        group_pages: 2,
        discord_on: true,
        discord: Link::Connected,
        vr: crate::vr::VrStatus::On,
        ..frame()
    }
}

fn text_size(r: &Renderer, font: usize, s: &str) -> (i32, i32) {
    let buf: Vec<u16> = s.encode_utf16().collect();
    let mut size = SIZE { cx: 0, cy: 0 };
    unsafe {
        SelectObject(r.dc, r.fonts[font] as _);
        GetTextExtentPoint32W(r.dc, buf.as_ptr(), buf.len() as i32, &mut size);
    }
    (size.cx, size.cy)
}

#[derive(Clone, Copy, Debug)]
struct Box2 {
    l: i32,
    t: i32,
    r: i32,
    b: i32,
}

fn ink(r: &Renderer, s: &Slot) -> (Box2, i32) {
    let (cx, cy) = text_size(r, s.font, &s.text);
    let (left, right) = (r.px(s.x), r.px(s.x + s.w));
    let (l, rr) = if s.flags & DT_RIGHT != 0 {
        (right - cx, right)
    } else {
        (left, left + cx)
    };
    let top = r.px(s.y);
    let t = if s.flags & DT_VCENTER != 0 {
        top + (r.px(s.h) - cy) / 2
    } else {
        top
    };
    (
        Box2 {
            l,
            t,
            r: rr,
            b: t + cy,
        },
        right - left - cx,
    )
}

fn gap(a: Box2, b: Box2) -> i32 {
    let dx = (b.l - a.r).max(a.l - b.r);
    let dy = (b.t - a.b).max(a.t - b.b);
    dx.max(dy)
}

const SHOWN: [Shown; 8] = [
    Shown { n: 0, clear: false },
    Shown { n: 0, clear: true },
    Shown { n: 1, clear: false },
    Shown { n: 3, clear: false },
    Shown {
        n: 20,
        clear: false,
    },
    Shown {
        n: 999,
        clear: false,
    },
    Shown {
        n: 1000,
        clear: false,
    },
    Shown {
        n: usize::MAX,
        clear: false,
    },
];

fn inputs() -> Vec<ClearingIn> {
    let mut v = Vec::new();
    for secs in [0, 59, 3_599, 359_999, u32::MAX as u64] {
        for tokens in [None, Some((0, 1)), Some((2, 3)), Some((999, 999))] {
            for e in SHOWN {
                v.push(ClearingIn {
                    secs,
                    tokens,
                    enemies: Some(e),
                });
            }
        }
    }
    v
}

struct Worst {
    edge: i32,
    pair: i32,
    slack: i32,
}

fn measure(r: &Renderer, c: &ClearingIn) -> Worst {
    let slots = clearing_slots(c);
    let (cl, ct, cr, cb) = (r.px(M), r.px(CARD_TOP), r.px(M + W), r.px(CARD_BOTTOM));
    let pager_b = r.px(FIGHT_PREV_HIT.1 + FIGHT_PREV_HIT.3);
    let mut boxes = Vec::new();
    let mut w = Worst {
        edge: i32::MAX,
        pair: i32::MAX,
        slack: i32::MAX,
    };
    for s in &slots {
        let (b, slack) = ink(r, s);
        w.slack = w.slack.min(slack);
        w.edge = w
            .edge
            .min(b.l - cl)
            .min(cr - b.r)
            .min(b.t - ct)
            .min(cb - b.b)
            .min(b.t - pager_b);
        boxes.push(b);
    }
    for i in 0..boxes.len() {
        for j in (i + 1).max(2)..boxes.len() {
            w.pair = w.pair.min(gap(boxes[i], boxes[j]));
        }
    }
    w
}

#[test]
fn enemy_count_caps_at_999_plus() {
    assert_eq!(enemy_num(0), "0");
    assert_eq!(enemy_num(999), "999");
    assert_eq!(enemy_num(1000), "999+");
    assert_eq!(enemy_num(usize::MAX), "999+");
}

#[test]
fn enemy_count_reads_naturally() {
    let t = |n, clear| enemies_text(Shown { n, clear });
    assert_eq!(t(0, false), "0 enemies left");
    assert_eq!(t(1, false), "1 enemy left");
    assert_eq!(t(3, false), "3 enemies left");
    assert_eq!(t(1000, false), "999+ enemies left");
    assert_eq!(t(0, true), "all clear");
}

#[test]
fn enemy_counter_never_clips_or_overlaps() {
    for dpi in DPIS {
        let r = Renderer::new(dpi, LOGICAL_W, LOGICAL_H);
        let min_gap = r.px(2).max(1);
        for c in inputs() {
            let w = measure(&r, &c);
            let ctx = format!("dpi {dpi} secs {} {:?} {:?}", c.secs, c.tokens, c.enemies);
            assert!(
                w.slack >= 0,
                "{ctx}: text wider than its slot by {}",
                -w.slack
            );
            assert!(w.edge >= 0, "{ctx}: {} px past the card edge", -w.edge);
            assert!(w.pair >= min_gap, "{ctx}: items {} px apart", w.pair);
        }
    }
}

#[test]
fn enemy_counter_does_not_move_anything_else() {
    for c in inputs() {
        let base = clearing_slots(&ClearingIn {
            secs: c.secs,
            tokens: c.tokens,
            enemies: None,
        });
        let with = clearing_slots(&c);
        assert_eq!(with.len(), base.len() + 1);
        assert_eq!(with[..base.len()], base[..]);
    }
}

#[test]
fn without_enemies_the_card_matches_the_old_layout() {
    let c = ClearingIn {
        secs: 75,
        tokens: Some((1, 3)),
        enemies: None,
    };
    let got: Vec<_> = clearing_slots(&c)
        .into_iter()
        .map(|s| (s.x, s.y, s.w, s.h, s.font, s.color, s.flags, s.text))
        .collect();
    assert_eq!(
        got,
        [
            (
                M + 12,
                114,
                W - 24,
                40,
                F_LABEL,
                DIM,
                DT_LEFT,
                "CLEARING".to_string()
            ),
            (
                M + 12,
                128,
                W - 24,
                40,
                F_BOSS,
                AMBER,
                DT_LEFT,
                "1m 15s".to_string()
            ),
            (
                M + 12,
                128,
                W - 24,
                22,
                F_TINY,
                DIM,
                DT_RIGHT | DT_VCENTER,
                "1/3 tokens".to_string()
            ),
        ]
    );
}

#[test]
fn enemy_counter_hides_outside_clearing() {
    let c = ClearingIn {
        secs: 10,
        tokens: None,
        enemies: None,
    };
    assert_eq!(clearing_slots(&c).len(), 2);
}

#[test]
fn live_state_reaches_the_card() {
    let at = ts(300);
    let shown = |i: usize| {
        let (_, n, retire) = CASES[i];
        enemies_shown(&stage_gs(n, retire), at)
    };
    assert_eq!(shown(0), Some(Shown { n: 0, clear: false }));
    assert_eq!(shown(1), Some(Shown { n: 3, clear: false }));
    assert_eq!(shown(2).map(|s| s.n), Some(1000));
    assert_eq!(shown(3), Some(Shown { n: 0, clear: true }));
}

fn out_dir() -> std::path::PathBuf {
    let dir = std::env::var("EHUD_ENEMIES_OUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("ehud-enemies").join("final"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn save_png(r: &Renderer, path: &std::path::Path) {
    unsafe { GdiFlush() };
    let (w, h) = (r.width as u32, r.height as u32);
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let c = pix(r, x, y);
            rgb.extend_from_slice(&[c as u8, (c >> 8) as u8, (c >> 16) as u8]);
        }
    }
    std::fs::write(path, png(w, h, &rgb)).unwrap();
}

#[test]
#[ignore]
fn write_enemy_counter_images() {
    let dir = out_dir();
    let mut boxes = String::new();
    for dpi in [48u32, 192] {
        let scale = format!("scale{:.1}", dpi as f32 / 96.0);
        for (name, n, retire) in CASES {
            let gs = stage_gs(n, retire);
            let f = stage_frame();
            let mut r = Renderer::new(dpi, LOGICAL_W, LOGICAL_H);
            r.draw_main(&gs, &f);
            save_png(&r, &dir.join(format!("A_{name}_{scale}.png")));
            let c = ClearingIn {
                secs: gs.stage_secs(f.now),
                tokens: gs.tokens_shown(),
                enemies: enemies_shown(&gs, f.now),
            };
            let slots = clearing_slots(&c);
            let (b, _) = ink(&r, slots.last().unwrap());
            let dy = r.px(BODY_DY);
            boxes.push_str(&format!(
                "A_{name}_{scale} {} {} {} {}\n",
                b.l,
                b.t + dy,
                b.r,
                b.b + dy
            ));
        }
    }
    std::fs::write(dir.join("counter_boxes.txt"), boxes).unwrap();
}

#[test]
#[ignore]
fn print_enemy_layout_numbers() {
    let probe = |n: usize, clear: bool, secs: u64, tokens: Option<(u32, u32)>| ClearingIn {
        secs,
        tokens,
        enemies: Some(Shown { n, clear }),
    };
    for dpi in DPIS {
        let r = Renderer::new(dpi, LOGICAL_W, LOGICAL_H);
        for (tag, c) in [
            ("0", probe(0, false, 75, Some((1, 3)))),
            ("3", probe(3, false, 75, Some((1, 3)))),
            ("999+", probe(1000, false, 75, Some((1, 3)))),
            ("clear", probe(0, true, 75, Some((1, 3)))),
            (
                "worst",
                probe(usize::MAX, false, u32::MAX as u64, Some((999, 999))),
            ),
        ] {
            let w = measure(&r, &c);
            let items: Vec<String> = clearing_slots(&c)
                .iter()
                .map(|s| {
                    let (b, _) = ink(&r, s);
                    format!("'{}' x{}..{} y{}..{}", s.text, b.l, b.r, b.t, b.b)
                })
                .collect();
            println!(
                "dpi {dpi:3} {tag:5} edge {:3}px pair {:3}px slack {:4}px | {}",
                w.edge,
                w.pair,
                w.slack,
                items.join(" ; ")
            );
        }
    }
}
