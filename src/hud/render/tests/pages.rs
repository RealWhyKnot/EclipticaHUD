use super::*;

#[test]
fn stage_table_never_leaves_its_card() {
    use super::super::fmt::compact;
    use super::super::main_panel::{COL, SPLIT_COLS, fmt_split};
    use crate::game::run::{BossFight, Run};
    let stage = |i: u64| BossFight {
        name: if i.is_multiple_of(3) {
            "Stage_BringersDesert".into()
        } else {
            format!("Stage_SomeUnlistedPlaceWithAVeryLongName{i}")
        },
        start_ts: 1000 + i * 600,
        end_ts: Some(1000 + i * 600 + 599),
        dmg: 987_654_321_987 - i,
        taken: 98_765_432_198 + i,
        deaths: crate::num::to_u32(i % 4) * 33,
        ..Default::default()
    };
    let mut gs = GameState::default();
    gs.runs.push(Run {
        start_ts: 1000,
        end_ts: Some(1000 + 30 * 600),
        stages: (0..30).map(stage).collect(),
        ..Default::default()
    });
    let (cx, cy, cw, ch) = SPLITS_CARD;
    let (top, bottom) = (cy + BODY_DY, cy + BODY_DY + ch);
    let used = 26 + 18 + crate::num::to_i32(SPLIT_ROWS) * SPLIT_ROW_H + 20 + 6 + SPLIT_ROW_H;
    assert!(cy + used <= cy + ch - 6);
    assert_eq!(STAGES_BTN.0 + STAGES_BTN.2, cx + cw - 12);
    assert_eq!(SPLIT_COLS[4].0 + SPLIT_COLS[4].1, cx + cw - 12);
    for dpi in [96, 120, 144, 192] {
        let mut r = Renderer::new(dpi, LOGICAL_W, LOGICAL_H);
        let fits = |font, s: &str, w: i32| r.text_w(font, s) <= r.px(w);
        for (_, w, _, h) in SPLIT_COLS {
            assert!(fits(F_TINY, h, w), "{h} at {dpi}");
        }
        assert!(
            fits(F_BODY, &fmt_split(999 * 60 + 59), SPLIT_COLS[1].1),
            "{dpi}"
        );
        assert!(
            fits(F_BODY, &compact(u64::MAX / 2), SPLIT_COLS[2].1),
            "{dpi}"
        );
        assert!(
            fits(F_BODY, &compact(u64::MAX / 2), SPLIT_COLS[3].1),
            "{dpi}"
        );
        assert!(fits(F_BODY, "999", SPLIT_COLS[4].1), "{dpi}");
        let hint = "999 to 999 of 999 stages, scroll for the rest";
        assert!(fits(F_TINY, hint, W - 24), "{dpi}");
        for label in ["FIGHT DMG", "STAGE DMG", "BOSSES", "TIME"] {
            assert!(
                fits(F_LABEL, label, STAGES_BTN.0 - (M + 12 + 2 * COL) - 4),
                "{label} at {dpi}"
            );
        }
        assert!(
            fits(F_LABEL, "THIS RUN BY STAGE", STAGES_BTN.0 - M - 20),
            "{dpi}"
        );
        for split_top in [0, 7, 999] {
            let f = Frame {
                flags: frame()
                    .flags
                    .with(Flag::RunSelected, true)
                    .with(Flag::GroupSelected, true)
                    .with(Flag::StagesButton, true)
                    .with(Flag::Stages, true),

                view_group: Some(20),
                group_pages: 30,

                split_top,
                now: 1000 + 30 * 600,
                hover: None,
                ..frame()
            };
            r.draw_main(&gs, &f);
            for y in r.px(top)..r.px(bottom) {
                for x in (0..r.px(cx)).chain(r.px(cx + cw)..r.width) {
                    assert_eq!(pix(&r, x, y), BG, "margin {x},{y} at {dpi} top {split_top}");
                }
            }
            for y in r.px(bottom)..r.px(LOGICAL_H - 30) {
                for x in 0..r.width {
                    assert_eq!(pix(&r, x, y), BG, "below {x},{y} at {dpi} top {split_top}");
                }
            }
            let lit = mix(CARD, ACCENT, 0.18);
            let row_y = |i: i32| r.px(cy + BODY_DY + 44 + i * SPLIT_ROW_H + SPLIT_ROW_H / 2);
            let lit_at = (0..crate::num::to_i32(SPLIT_ROWS))
                .find(|&i| pix(&r, r.px(M + 8), row_y(i)) == lit);
            let shown = split_top.min(30 - SPLIT_ROWS);
            let want = (shown..shown + SPLIT_ROWS)
                .contains(&20)
                .then(|| crate::num::to_i32(20 - shown));
            assert_eq!(lit_at, want, "lit row at {dpi} top {split_top}");
        }
    }
}

#[test]
fn long_boss_names_stay_in_the_title_slot() {
    for dpi in [96, 120, 144, 192] {
        let draw = |boss: &str| {
            let mut r = Renderer::new(dpi, LOGICAL_W, LOGICAL_H);
            let (gs, now) = boss_fight(boss);
            r.draw_main(&gs, &Frame { now, ..frame() });
            let px = pixels(&r);
            (r, px)
        };
        let (r, short) = draw("Kakarot");
        let (x0, x1) = (r.px(M), r.px(M + 226));
        let (y0, y1) = (r.px(88), r.px(88) + r.px(40));
        let w = r.width;
        for boss in LONG_BOSSES {
            let id = boss.repeat(8);
            assert!(
                crate::game::names::boss_name(&id).chars().count() > 100,
                "{dpi} {boss}"
            );
            let (_, long) = draw(&id);
            let at = |x: i32, y: i32| crate::num::to_usize(y * w + x);
            for y in 0..r.height {
                for x in 0..w {
                    let inside = (x0..x1).contains(&x) && (y0..y1).contains(&y);
                    assert!(
                        inside || short[at(x, y)] == long[at(x, y)],
                        "{dpi} {boss} drew outside the title at {x},{y}"
                    );
                }
            }
            let tail =
                (x1 - r.px(24)..x1).any(|x| (y0..y1).any(|y| short[at(x, y)] != long[at(x, y)]));
            assert!(tail, "{dpi} {boss} left the end of the title slot empty");
        }
    }
}

#[test]
fn long_boss_names_stay_in_the_log_body() {
    let draw = |boss: &str| {
        let mut r = Renderer::new(96, LOG_W, LOG_H);
        let (gs, _) = boss_fight(boss);
        r.draw_log(timeline::live(&gs), &log_view(Filter::All));
        let px = pixels(&r);
        (r, px)
    };
    let (r, short) = draw("Kakarot");
    let (bx, by, bw, bh) = LOG_BODY;
    for boss in LONG_BOSSES {
        let (_, long) = draw(&boss.repeat(8));
        assert_ne!(short, long, "{boss}");
        for y in 0..r.height {
            for x in 0..r.width {
                let inside = (bx..bx + bw).contains(&x) && (by..by + bh).contains(&y);
                let i = crate::num::to_usize(y * r.width + x);
                assert!(inside || short[i] == long[i], "{boss} at {x},{y}");
            }
        }
    }
}

#[test]
fn missed_pages_show_no_damage_numbers() {
    use super::super::main_panel::{COL, MISSED, MISSED_FROM};
    let gs = caught_up();
    let run = &gs.runs[0];
    assert!(run.page(0).unwrap().missed);
    assert!(!run.page(1).unwrap().missed);
    let splits = gs.run_splits(0, u64::MAX);
    assert_eq!(splits.len(), 2);
    assert!(splits[0].missed);
    assert!(!splits[1].missed);
    for dpi in [96, 120, 144, 192] {
        let mut r = Renderer::new(dpi, LOGICAL_W, LOGICAL_H);
        assert!(text_w(&r, F_BODY, MISSED_FROM) <= r.px(W - 24), "{dpi}");
        assert!(text_w(&r, F_TINY, MISSED) <= r.px(GRAPH_RECT.2), "{dpi}");
        for f in missed_frames(&gs) {
            r.draw_main(&gs, &f);
            for y in r.px(BODY_DY + 110)..r.height - r.px(40) {
                for x in (0..r.px(M)).chain(r.px(M + W)..r.width) {
                    assert_eq!(pix(&r, x, y), BG, "margin {x},{y} at {dpi}");
                }
            }
        }
    }
    let [page, _] = missed_frames(&gs);
    let mut r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    r.draw_main(&gs, &page);
    let amber = (BODY_DY + 190..BODY_DY + 236)
        .flat_map(|y| (M..M + COL * 2).map(move |x| (x, y)))
        .filter(|&(x, y)| pix(&r, x, y) == AMBER)
        .count();
    assert_eq!(amber, 0);
}

#[test]
#[ignore = "writes images for review"]
fn write_missed_page_images() {
    let tag = std::env::var("EHUD_PICK").unwrap_or_else(|_| "after".into());
    let gs = caught_up();
    let label_h: i32 = 28;
    let (w, h) = (crate::num::to_u32(LOGICAL_W), crate::num::to_u32(LOGICAL_H));
    let mut sheet = Sheet::new(w * 2 + 12, h + label_h.cast_unsigned());
    for (i, f) in missed_frames(&gs).iter().enumerate() {
        let mut r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
        r.draw_main(&gs, f);
        sheet.put(
            &r,
            crate::num::to_u32(i) * (w + 12),
            label_h.cast_unsigned(),
            (0, 0, LOGICAL_W, LOGICAL_H),
        );
    }
    for (i, caption) in [
        format!("{tag}: missed boss page"),
        format!("{tag}: stage table"),
    ]
    .iter()
    .enumerate()
    {
        let r = Renderer::new(96, LOGICAL_W, label_h);
        r.fill(0, 0, LOGICAL_W, label_h, BG);
        r.text((8, 4, LOGICAL_W - 16), F_BOSS, TEXT, DT_LEFT, caption);
        sheet.put(
            &r,
            crate::num::to_u32(i) * (w + 12),
            0,
            (0, 0, LOGICAL_W, label_h),
        );
    }
    sheet.save(&format!("missed_{tag}.png"));
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".devflow")
        .join("pick");
    std::fs::write(
        dir.join(format!("missed_{tag}.rgb")),
        [
            &sheet.w.to_le_bytes()[..],
            &sheet.h.to_le_bytes(),
            &sheet.rgb,
        ]
        .concat(),
    )
    .unwrap();
}

#[test]
fn sync_tag_shows_in_the_run_line() {
    use super::super::main_panel::{SYNCED, SYNCING};
    for dpi in [96, 120, 144, 192] {
        let mut r = Renderer::new(dpi, LOGICAL_W, LOGICAL_H);
        let pager = r.px(LOGICAL_W - 88);
        for tag in [SYNCING, SYNCED] {
            let label = format!("RUN 99/99   stage 99   {tag}");
            assert!(text_w(&r, F_LABEL, &label) <= pager, "{dpi} {label}");
        }
        for (_, gs, sync) in sync_frames() {
            r.draw_main(
                &gs,
                &Frame {
                    now: sync_now(),
                    sync,
                    ..frame()
                },
            );
            let count = |c: u32| {
                (r.px(54)..r.px(78))
                    .flat_map(|y| (0..r.width).map(move |x| (x, y)))
                    .filter(|&(x, y)| pix(&r, x, y) == c)
                    .count()
            };
            let (accent, good) = (count(ACCENT), count(GOOD));
            match sync {
                RunSync::Waiting => assert!(accent > 0 && good == 0, "{dpi}"),
                RunSync::Done => assert!(good > 0 && accent == 0, "{dpi}"),
                RunSync::Idle => assert!(good == 0 && accent == 0, "{dpi}"),
            }
            for y in r.px(54)..r.px(78) {
                for x in (r.px(LOGICAL_W - 44)..r.width).chain(0..r.px(44)) {
                    let c = pix(&r, x, y);
                    assert!(c != ACCENT && c != GOOD, "{dpi} {x},{y}");
                }
            }
        }
    }
}

#[test]
#[ignore = "writes images for review"]
fn write_sync_images() {
    let label_h: i32 = 28;
    let (w, h) = (crate::num::to_u32(LOGICAL_W), crate::num::to_u32(LOGICAL_H));
    let mut sheet = Sheet::new(w * 3 + 24, h + label_h.cast_unsigned());
    for (i, (caption, gs, sync)) in sync_frames().iter().enumerate() {
        let x = crate::num::to_u32(i) * (w + 12);
        let mut r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
        r.draw_main(
            gs,
            &Frame {
                now: sync_now(),
                sync: *sync,
                ..frame()
            },
        );
        sheet.put(&r, x, label_h.cast_unsigned(), (0, 0, LOGICAL_W, LOGICAL_H));
        let r = Renderer::new(96, LOGICAL_W, label_h);
        r.fill(0, 0, LOGICAL_W, label_h, BG);
        r.text((8, 4, LOGICAL_W - 16), F_BOSS, TEXT, DT_LEFT, caption);
        sheet.put(&r, x, 0, (0, 0, LOGICAL_W, label_h));
    }
    sheet.save("sync.png");
}
