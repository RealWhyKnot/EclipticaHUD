use super::*;

#[test]
fn past_run_tags_and_no_boss_page() {
    let feed = |gs: &mut GameState, t: &str, m: &str| {
        gs.feed(&format!("2026.09.15 {t} Debug      -  {m}"));
    };
    let mut gs = GameState::default();
    feed(
        &mut gs,
        "01:09:15",
        "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Thaumaturge",
    );
    feed(&mut gs, "01:10:00", "Dealing 1688 STRIKE damage");
    feed(&mut gs, "01:22:49", "[Behaviour] OnLeftRoom");
    feed(
        &mut gs,
        "01:30:00",
        "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Thaumaturge",
    );
    feed(
        &mut gs,
        "01:32:00",
        "ECLIPTICA - now fighting boss: DarkMouth(Clone) on phase: 0",
    );
    feed(
        &mut gs,
        "01:34:00",
        "Boss DarkMouth dead, personal damage dealt: ",
    );
    feed(&mut gs, "01:34:00", "STRIKE DMG: 900");
    feed(&mut gs, "01:34:00", "NON-STRIKE DMG: 0");
    feed(&mut gs, "01:34:00", "ECLIPTICA - now in lobby");
    let page = |i: usize| Frame {
        pages: 3,
        view_page: i,
        view_run: Some(i),
        view_group: gs.runs[i].groups().len().checked_sub(1),
        group_pages: gs.runs[i].groups().len(),
        flags: frame().flags.with(Flag::RunSelected, true),
        ..frame()
    };
    let ink = |r: &Renderer, color: u32, y0: i32, y1: i32| {
        (y0..y1)
            .flat_map(|y| (0..LOGICAL_W).map(move |x| (x, y)))
            .filter(|&(x, y)| pix(r, x, y) == color)
            .count()
    };
    let mut r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    r.draw_main(&gs, &page(0));
    assert_eq!(ink(&r, DANGER, 54, 78), 0);
    assert!(ink(&r, AMBER, 200, 240) > 0);
    r.draw_main(&gs, &page(1));
    assert!(ink(&r, DANGER, 54, 78) > 0);
    assert!(ink(&r, DANGER, 128, 150) > 0);
}

#[test]
fn damage_graph_draws_fight_columns() {
    let (gs, now) = fight_log(30, false);
    let mut r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    r.draw_main(&gs, &Frame { now, ..frame() });
    let (gx, gy, gw, gh) = GRAPH_RECT;
    assert_eq!(gx + gw, LOGICAL_W - M - 12);
    assert!(gy - 2 > 228 && gy + gh + 2 < 288);
    let gy = gy + BODY_DY;
    let well = mix(CARD, BG, 0.55);
    let x0 = gx + 28 * gw / 30;
    assert_eq!(pix(&r, x0 + 2, gy), AMBER);
    assert_eq!(pix(&r, x0 + 2, gy + gh - 1), mix(CARD, AMBER, 0.45));
    assert_eq!(pix(&r, gx + gw - 2, gy + gh - 1), well);
    assert_eq!(pix(&r, gx + 2, gy + gh - 1), well);
    assert_eq!(pix(&r, gx - 3, gy + 10), CARD);
    assert_eq!(pix(&r, gx + gw + 2, gy + 10), CARD);
    assert_eq!(pix(&r, 20, 106), mix(CARD, DANGER, 0.08));
}

#[test]
fn damage_graph_fits_any_fight_length() {
    let (gs, now) = fight_log(9000, false);
    let run = gs.live_run().unwrap();
    let g = run
        .group(0)
        .unwrap()
        .graph(now, super::super::graph::GRAPH_COLS);
    assert!(g.cols.len() <= super::super::graph::GRAPH_COLS);
    let mut r = Renderer::new(192, LOGICAL_W, LOGICAL_H);
    r.draw_main(&gs, &Frame { now, ..frame() });
    let (gx, gy, gw, gh) = GRAPH_RECT;
    let gy = gy + BODY_DY;
    for y in [gy - 4, gy + gh + 3] {
        for x in (gx..gx + gw).step_by(7) {
            assert_eq!(pix(&r, x * 2, y * 2), CARD, "{x},{y}");
        }
    }
    assert_eq!(pix(&r, 20 * 2, 306 * 2), CARD);
}

#[test]
fn boss_card_marks_you_as_target() {
    let mut r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    let (gs, now) = fight_log(10, true);
    assert!(gs.targets_me());
    r.draw_main(&gs, &Frame { now, ..frame() });
    assert_eq!(pix(&r, 20, 100), mix(CARD, DANGER, 0.16));
    assert!(
        text_w(&r, F_LABEL, "TARGETING YOU") + 20 + text_w(&r, F_TINY, "phase 3") + 8
            <= LOGICAL_W - 2 * M - 24
    );
    let (other, now) = fight_log(10, false);
    assert!(!other.targets_me());
    r.draw_main(&other, &Frame { now, ..frame() });
    assert_eq!(pix(&r, 20, 100), mix(CARD, DANGER, 0.08));
    let away = Frame {
        now,
        env: Env::NotInWorld,
        ..frame()
    };
    r.draw_main(&gs, &away);
    assert_eq!(pix(&r, 20, 100), CARD);
}

#[test]
fn update_prompt_covers_panel_below_status() {
    let mut r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    let (gs, now) = fight_log(10, false);
    let tag = "v2026.10.29.0".to_string();
    r.draw_main(
        &gs,
        &Frame {
            now,
            ..prompt_frame(Badge::Ready(tag.clone()))
        },
    );
    let (top, h) = PROMPT_AREA;
    assert!(top > 57 && top < 67);
    assert!(top + h < LOGICAL_H - 26);
    assert_eq!(pix(&r, 20, 100), BG);
    assert_eq!(pix(&r, 20, 500), BG);
    assert_eq!(pix(&r, 100, 55), GOOD);
    let (nx, ny, nw, nh) = UPDATE_NOW_HIT;
    let (lx, ly, lw, lh) = UPDATE_LATER_HIT;
    assert_eq!(pix(&r, nx + 6, ny + nh / 2), mix(BG, GOOD, 0.25));
    assert_eq!(pix(&r, lx + 6, ly + lh / 2), BG);
    assert_eq!(pix(&r, lx, ly + lh / 2), EDGE);
    assert_eq!(nx - M, LOGICAL_W - M - (lx + lw));
    assert!(nx + nw < lx && ny == ly && nh == lh);
    assert_eq!(pix(&r, 180, PROMPT_T - 4), BG);
    assert!(text_w(&r, F_BIG, "Update available") <= W - 24);
    let (sx, sy, sw, _) = UPDATE_SKIP_HIT;
    assert!(text_w(&r, F_TINY, "Skip this version") <= sw);
    assert_eq!(sx + sw / 2, LOGICAL_W / 2);
    assert!(sy >= ny + nh + 4);
    let hovered = Frame {
        now,
        hover: Some(Hit::UpdateNow),
        ..prompt_frame(Badge::Ready(tag.clone()))
    };
    r.draw_main(&gs, &hovered);
    assert_eq!(pix(&r, nx + 6, ny + nh / 2), mix(BG, GOOD, 0.4));
    r.draw_main(
        &gs,
        &Frame {
            now,
            ..prompt_frame(Badge::Installing(tag.clone()))
        },
    );
    assert_eq!(pix(&r, nx + 6, ny + nh / 2), BG);
    r.draw_main(
        &gs,
        &Frame {
            now,
            ..prompt_frame(Badge::Failed(tag))
        },
    );
    assert_eq!(pix(&r, nx + 6, ny + nh / 2), mix(BG, GOOD, 0.25));
    r.draw_main(&gs, &Frame { now, ..frame() });
    assert_eq!(pix(&r, 20, 100), mix(CARD, DANGER, 0.08));
}

#[test]
fn update_prompt_clips_long_versions() {
    let tag = format!("v{}", "9".repeat(400));
    let (gs, now) = fight_log(10, false);
    for dpi in [96, 192] {
        let mut r = Renderer::new(dpi, LOGICAL_W, LOGICAL_H);
        let s = dpi.cast_signed() / 96;
        let f = Frame {
            now,
            ..prompt_frame(Badge::Ready(tag.clone()))
        };
        r.draw_main(&gs, &f);
        for y in PROMPT_T..UPDATE_NOW_HIT.1 {
            for x in [0, M + 11, LOGICAL_W - M - 12, LOGICAL_W - 1] {
                assert_eq!(pix(&r, x * s, y * s), BG, "{x},{y} at {dpi}");
            }
        }
    }
}

#[test]
fn update_prompt_hit_regions() {
    let renderer = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    for (rect, hit) in [
        (UPDATE_NOW_HIT, Hit::UpdateNow),
        (UPDATE_LATER_HIT, Hit::UpdateLater),
        (UPDATE_SKIP_HIT, Hit::UpdateSkip),
    ] {
        let (x, y, width, height) = rect;
        let only = |found| found == hit;
        assert_eq!(renderer.hit_test_where(x, y, only), Some(hit));
        assert_eq!(
            renderer.hit_test_where(x + width - 1, y + height - 1, only),
            Some(hit)
        );
        assert_eq!(renderer.hit_test_where(x + width, y + height, only), None);
        assert!(y >= PROMPT_AREA.0 && y + height <= PROMPT_AREA.0 + PROMPT_AREA.1);
    }
}

#[test]
fn log_tabs_fit_their_labels() {
    for dpi in [96, 120, 144, 192] {
        let r = Renderer::new(dpi, LOG_W, LOG_H);
        for (_, label, _, w) in LOG_TABS {
            let tw = text_w(&r, F_LABEL, label);
            assert!(
                tw + r.px(14) <= r.px(w),
                "{label} {tw} vs {} at {dpi}",
                r.px(w)
            );
        }
        for status in ["999 events", "nothing logged yet"] {
            assert!(
                text_w(&r, F_TINY, status) <= r.px(LOG_COUNT.2),
                "{status} at {dpi}"
            );
        }
    }
    let r = Renderer::new(96, LOG_W, LOG_H);
    for (_, label, _, w) in LOG_TABS {
        assert_eq!(w - text_w(&r, F_LABEL, label), 14, "{label}");
    }
    assert!(LOG_TABS[0].2 >= LOG_COUNT.0 + LOG_COUNT.2 + 2);
    let (_, _, lx, lw) = LOG_TABS[LOG_TABS.len() - 1];
    assert_eq!(lx + lw, LOG_CLOSE_HIT.0 - 8);
    for pair in LOG_TABS.windows(2) {
        assert_eq!(pair[0].2 + pair[0].3 + 4, pair[1].2);
    }
    let last = LOG_TABS[LOG_TABS.len() - 1];
    assert!(last.2 + last.3 + 4 <= LOG_CLOSE_HIT.0);
    assert!(LOG_TABS.iter().all(|t| t.2 % 2 == 0 && t.3 % 2 == 0));
}

#[test]
fn tooltips_that_fit_keep_one_line() {
    for dpi in [96, 192] {
        let r = Renderer::new(dpi, LOGICAL_W, LOGICAL_H);
        for tip in [
            "Close the HUD (Esc)",
            "Make the HUD smaller",
            "Download the new version and restart the HUD",
        ] {
            let (w, n) = tip_fits(&r, tip, LOGICAL_W);
            assert_eq!(n, 1, "{tip} at {dpi}");
            assert_eq!(
                w,
                (crate::num::to_i32(tip.chars().count()) * 6 + 20).min(LOGICAL_W - 16)
            );
        }
        for tip in [
            "Incoming damage right now against the worst 10 seconds this fight",
            "Run number, start time, stage and deaths; LOST means a boss survived",
        ] {
            tip_fits(&r, tip, LOGICAL_W);
            tip_fits(&r, tip, LOG_W);
        }
    }
}

#[test]
fn compact_numbers() {
    use super::super::fmt::compact;
    for (n, s) in [
        (0, "0"),
        (999, "999"),
        (1000, "1.00K"),
        (1049, "1.05K"),
        (12_345, "12.3K"),
        (999_499, "999K"),
        (999_500, "1.00M"),
        (12_345_678, "12.3M"),
        (123_456_789_012, "123B"),
        (MAX_DMG, "1.10T"),
        (1_000_000_000_000_000, "1.00Qa"),
        (u64::MAX, "18.4Qi"),
    ] {
        assert_eq!(compact(n), s, "{n}");
    }
    let mut n = 1u64;
    while let Some(next) = n.checked_mul(3) {
        for v in [n, n - 1, n + n / 2] {
            let s = compact(v);
            assert!(s.len() <= 6, "{v} {s}");
            let unit = s.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
            let k = match unit {
                "" => 0,
                u => {
                    crate::num::to_i32(
                        super::super::fmt::UNITS
                            .iter()
                            .position(|x| *x == u)
                            .unwrap(),
                    ) + 1
                }
            };
            let num: f64 = s[..s.len() - unit.len()].parse().unwrap();
            let back = num * 1000f64.powi(k);
            assert!(
                ((back - crate::num::to_f64(v)) / crate::num::to_f64(v.max(1))).abs() < 0.006,
                "{v} {s}"
            );
        }
        n = next;
    }
}

#[test]
fn png_writer_matches_known_crc() {
    assert_eq!(crc32(b"IEND"), 0xae42_6082);
    let p = png(1, 1, &[255, 0, 0]);
    assert_eq!(&p[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&p[p.len() - 12..], b"\0\0\0\0IEND\xaeB`\x82");
}

#[test]
#[ignore = "writes images for review"]
fn write_tooltip_wrap_images() {
    let ctx = TipCtx {
        flags: Flags::default()
            .with(Flag::Live, false)
            .with(Flag::Topmost, false)
            .with(Flag::Sound, false)
            .with(Flag::LogOpen, false)
            .with(Flag::Discord, false)
            .with(Flag::Usage, false)
            .with(Flag::UsageFailing, false)
            .with(Flag::Board, false)
            .with(Flag::PartyOpen, false)
            .with(Flag::Stages, false),

        board_status: 0,
        board_delete: 0,
    };
    let cases = [
        (96, Hit::Party),
        (192, Hit::Party),
        (96, Hit::Info(Info::TopAttacks)),
        (96, Hit::Info(Info::RunRow)),
    ];
    let gs = GameState::default();
    let (gap, label_h) = (16u32, 20);
    let bands: Vec<(i32, i32)> = cases
        .iter()
        .map(|&(_, hit)| {
            let near = hit.rect();
            let top = (near.1 - 8).max(0);
            (top, (near.1 + near.3 + 6 + 40 + 8).min(LOGICAL_H) - top)
        })
        .collect();
    let height: u32 = cases
        .iter()
        .zip(&bands)
        .map(|(&(dpi, _), &(_, height))| ((label_h + height) * dpi / 96).cast_unsigned() + gap)
        .sum();
    let mut sheet = Sheet::new(gap * 3 + crate::num::to_u32(LOGICAL_W) * 4, gap + height);
    let mut y = gap;
    for (&(dpi, hit), &(top, height)) in cases.iter().zip(&bands) {
        let factor = dpi / 96;
        let near = hit.rect();
        let text = hit.tip(ctx);
        for (col, name) in ["A  before: one line", "B  after: wraps to fit"]
            .iter()
            .enumerate()
        {
            let x = gap
                + crate::num::to_u32(col) * (LOGICAL_W * factor).cast_unsigned()
                + crate::num::to_u32(col) * gap;
            let label = Renderer::new(dpi.cast_unsigned(), LOGICAL_W, label_h);
            label.rround(0, 0, LOGICAL_W, label_h, 0, BG);
            label.text_rect(
                (4, 0, LOGICAL_W - 8, label_h),
                F_LABEL,
                TEXT,
                DT_VCENTER,
                name,
            );
            // SAFETY: GdiFlush takes no arguments and only waits for pending drawing.
            unsafe { GdiFlush() };
            sheet.put(&label, x, y, (0, 0, LOGICAL_W, label_h));
            let mut renderer = Renderer::new(dpi.cast_unsigned(), LOGICAL_W, LOGICAL_H);
            renderer.draw_main(
                &gs,
                &Frame {
                    flags: main_frame()
                        .flags
                        .with(Flag::SettingsOpen, hit == Hit::Party),
                    hover: Some(hit),
                    tip: None,
                    ..main_frame()
                },
            );
            if col == 0 {
                let width = (crate::num::to_i32(text.chars().count()) * 6 + 20).min(LOGICAL_W - 16);
                let tx = near.0.clamp(8, LOGICAL_W - 8 - width);
                let ty = near.1 + near.3 + 6;
                renderer.rround(tx, ty, width, 24, 6, ACCENT);
                renderer.rround(tx + 1, ty + 1, width - 2, 22, 5, CARD_HI);
                renderer.text_rect(
                    (tx, ty, width, 24),
                    F_TINY,
                    TEXT,
                    DT_CENTER | DT_VCENTER,
                    text,
                );
            } else {
                renderer.tooltip(near, text, LOGICAL_W, LOGICAL_H);
            }
            // SAFETY: GdiFlush takes no arguments and only waits for pending drawing.
            unsafe { GdiFlush() };
            sheet.put(
                &renderer,
                x,
                y + (label_h * factor).cast_unsigned(),
                (0, top, LOGICAL_W, height),
            );
        }
        y += ((label_h + height) * factor).cast_unsigned() + gap;
    }
    sheet.save("tooltip_wrap.png");
}

#[test]
fn counts_use_the_singular_for_one() {
    use super::super::fmt::fmt_count;
    assert_eq!(fmt_count(0, "boss", "bosses"), "0 bosses");
    assert_eq!(fmt_count(1, "boss", "bosses"), "1 boss");
    assert_eq!(fmt_count(2, "token", "tokens"), "2 tokens");
}

#[test]
fn kind_tag_leaves_room_for_the_title() {
    for dpi in [96, 120, 144, 192] {
        let r = Renderer::new(dpi, LOGICAL_W, LOGICAL_H);
        for tag in ["STAGE 12", "BOSS 12"] {
            let tx = (M + 10 + r.unscale(r.text_w(F_LABEL, tag)) + 12 + 8).max(M + 60);
            let room = r.px(M + 226 - tx);
            for name in ["Jacked O' Lantern", "Bringer's Desert", "Melgor Johnson"] {
                assert!(r.text_w(F_BOSS, name) <= room, "{dpi} {tag} {name}");
            }
        }
    }
}

#[test]
fn page_strip_stays_inside_the_card() {
    use super::super::main_panel::{STRIP_MIN, STRIP_W, strip_layout};
    for dpi in [96, 120, 144, 192] {
        let r = Renderer::new(dpi, LOGICAL_W, LOGICAL_H);
        assert!(r.unscale(r.text_w(F_TINY, "B99")) + 4 <= STRIP_MIN, "{dpi}");
    }
    for n in 1..120 {
        for cur in 0..n {
            let (shown, sw) = strip_layout(n, cur);
            let k = crate::num::to_i32(shown.len());
            assert!(shown.contains(&cur) && shown.end <= n, "{n} {cur}");
            assert!(
                sw >= STRIP_MIN && k * sw + (k - 1) * 4 <= STRIP_W,
                "{n} {cur}"
            );
        }
    }
}
