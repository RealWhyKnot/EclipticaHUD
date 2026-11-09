use super::*;

#[test]
fn close_btn_inside_hit() {
    let (hx, hy, hw, hh) = CLOSE_HIT;
    let (bx, by, bw, bh) = CLOSE_BTN;
    assert_eq!(hx + hw, LOGICAL_W);
    assert_eq!(hy, 0);
    assert!(bx >= hx && by >= hy);
    assert!(bx + bw <= hx + hw && by + bh <= hy + hh);
    assert!(by >= 8);
    assert!(LOGICAL_W - (bx + bw) >= 8);
    let (lx, ly, lw, lh) = LOG_HIT;
    let (bx, by, bw, bh) = LOG_BTN;
    assert_eq!(lx + lw, hx);
    assert!(bx >= lx && by >= ly && bx + bw <= lx + lw && by + bh <= ly + lh);
}

#[test]
fn toolbar_ink_gaps() {
    let r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    r.fill(0, 0, LOGICAL_W, LOGICAL_H, BG);
    let buttons = [
        (PARTY_BTN, GLYPH_PARTY),
        (SETTINGS_BTN, GLYPH_SETTINGS),
        (PIN_BTN, GLYPH_PIN),
        (LOG_BTN, GLYPH_LOG),
        (CLOSE_BTN, GLYPH_CLOSE),
    ];
    let mut spans = Vec::new();
    for (b, g) in buttons {
        r.glyph_button(b, Ctl::Rest, g);
        // SAFETY: GdiFlush takes no arguments and only waits for pending drawing.
        unsafe { GdiFlush() };
        let (bx, by, bw, bh) = b;
        let inked = |x: i32| (by..by + bh).any(|y| pix(&r, x, y) != BG);
        let xs: Vec<i32> = (bx..bx + bw).filter(|&x| inked(x)).collect();
        let ys: Vec<i32> = (by..by + bh)
            .filter(|&y| (bx..bx + bw).any(|x| pix(&r, x, y) != BG))
            .collect();
        spans.push((xs[0], xs[xs.len() - 1], ys[0], ys[ys.len() - 1]));
    }
    let gaps: Vec<i32> = spans.windows(2).map(|w| w[1].0 - w[0].1).collect();
    crate::console::say(format_args!("toolbar ink spans {spans:?} gaps {gaps:?}"));
    let lo = gaps.iter().min().unwrap();
    let hi = gaps.iter().max().unwrap();
    assert!(hi - lo <= 2, "ink spans {spans:?} gaps {gaps:?}");
}

#[test]
fn footer_text_fits_its_regions() {
    let r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    let end = |x: i32, s: &str| x + text_w(&r, F_TINY, s);
    let region = |i: Info| INFO_REGIONS.iter().find(|(j, _)| *j == i).unwrap().1;
    let (vx, _, vw, _) = region(Info::Vr);
    let (lx, _, lw, _) = region(Info::LogDot);
    let (dx, _, dw, _) = DISCORD_DOT;
    assert!(end(28, "STEAMVR") <= vx + vw);
    assert!(end(104, "LOG") <= lx + lw);
    assert!(lx + lw <= dx);
    assert!(end(dx + 18, "DISCORD") <= dx + dw);
    for badge in [
        "update v2026.12.31.10",
        "v2026.12.31.10-beta",
        "update failed",
    ] {
        assert!(
            LOGICAL_W - 14 - text_w(&r, F_TINY, badge) >= UPDATE_HIT.0,
            "{badge}"
        );
    }
}

#[test]
fn easing() {
    assert_eq!(ease_out_cubic(0.0), 0.0);
    assert_eq!(ease_out_cubic(1.0), 1.0);
    let mut prev = 0.0;
    for i in 1..=10 {
        let v = ease_out_cubic(crate::num::to_f32(i) / 10.0);
        assert!(v >= prev);
        prev = v;
    }
    assert_eq!(lerp(2.0, 6.0, 0.0), 2.0);
    assert_eq!(lerp(2.0, 6.0, 1.0), 6.0);
}

#[test]
fn hit_test_regions() {
    let renderer = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    let closed = |x, y| renderer.hit_test_where(x, y, |hit| !settings_hit(hit));
    let (tx, ty, tw, th) = Hit::Mute.rect();
    assert!(tx >= 14 && tx + tw <= LOGICAL_W - 14);
    assert!(ty >= 82 && ty + th <= 178);
    assert_eq!(closed(tx, ty), Some(Hit::Mute));
    assert_eq!(closed(tx + tw - 1, ty + th - 1), Some(Hit::Mute));
    assert_ne!(closed(tx - 1, ty), Some(Hit::Mute));
    assert_ne!(closed(tx, ty + th), Some(Hit::Mute));
    assert_eq!(closed(tx + tw, ty), None);
    assert_eq!(closed(200, 140), Some(Hit::Info(Info::Result)));
    assert_eq!(renderer.hit_test(LOGICAL_W - 1, 0), Some(Hit::Close));
    assert_eq!(renderer.hit_test(324, 35), Some(Hit::Close));
    assert_eq!(renderer.hit_test(323, 35), Some(Hit::Log));
    assert_eq!(renderer.hit_test(292, 0), Some(Hit::Log));
    assert_eq!(renderer.hit_test(291, 0), Some(Hit::Pin));
    assert_eq!(renderer.hit_test(260, 35), Some(Hit::Pin));
    assert_eq!(renderer.hit_test(259, 0), Some(Hit::Settings));
    assert_eq!(renderer.hit_test(228, 35), Some(Hit::Settings));
    assert_eq!(renderer.hit_test(227, 0), Some(Hit::Party));
    assert_eq!(renderer.hit_test(196, 35), Some(Hit::Party));
    assert_eq!(renderer.hit_test(195, 0), None);
    assert_eq!(SETTINGS_HIT.0 + SETTINGS_HIT.2, PIN_HIT.0);
    assert_eq!(PARTY_HIT.0 + PARTY_HIT.2, SETTINGS_HIT.0);
    for (rect, hit) in [
        (Hit::ScaleDown.rect(), Hit::ScaleDown),
        (Hit::ScaleUp.rect(), Hit::ScaleUp),
        (Hit::AlphaDown.rect(), Hit::AlphaDown),
        (Hit::AlphaUp.rect(), Hit::AlphaUp),
        (Hit::WindowDown.rect(), Hit::WindowDown),
        (Hit::WindowUp.rect(), Hit::WindowUp),
        (Hit::Sound.rect(), Hit::Sound),
        (Hit::Discord.rect(), Hit::Discord),
        (Hit::Usage.rect(), Hit::Usage),
    ] {
        let (x, y, width, height) = rect;
        assert!(settings_hit(hit));
        assert_eq!(renderer.hit_test(x, y), Some(hit));
        assert_eq!(renderer.hit_test(x + width - 1, y + height - 1), Some(hit));
        let (cx, cy, cw, ch) = SETTINGS_CARD;
        let cy = cy + BODY_DY;
        assert!(x >= cx + 12 && x + width <= cx + cw - 12, "{hit:?}");
        assert!(y >= cy + 12 && y + height <= cy + ch - 12, "{hit:?}");
    }
    let (px_, py_, pw_, ph_) = PIN_BTN;
    let (hx_, hy_, hw_, hh_) = PIN_HIT;
    assert!(px_ >= hx_ && py_ >= hy_ && px_ + pw_ <= hx_ + hw_ && py_ + ph_ <= hy_ + hh_);
    assert_eq!(hx_ + hw_, LOG_HIT.0);
    assert_eq!(renderer.hit_test(323, 36), Some(Hit::Info(Info::Status)));
    assert_eq!(renderer.hit_test(230, LOGICAL_H - 32), Some(Hit::Update));
    assert_eq!(renderer.hit_test(229, LOGICAL_H - 1), None);
    let (dx, dy, dw, dh) = DISCORD_DOT;
    let dot = Some(Hit::Info(Info::Discord));
    assert_eq!(renderer.hit_test(dx, dy), dot);
    assert_eq!(renderer.hit_test(dx + dw - 1, dy + dh - 1), dot);
    assert_ne!(renderer.hit_test(dx + dw, dy), dot);
    assert!(dx + dw <= UPDATE_HIT.0);
    for (rect, hit) in [
        (Hit::RunPrev.rect(), Hit::RunPrev),
        (Hit::RunNext.rect(), Hit::RunNext),
        (Hit::FightPrev.rect(), Hit::FightPrev),
        (Hit::FightNext.rect(), Hit::FightNext),
    ] {
        let (x, y, width, height) = rect;
        assert_eq!(
            renderer.hit_test_where(x, y, |found| found == hit),
            Some(hit)
        );
        assert_eq!(
            renderer.hit_test_where(x + width - 1, y + height - 1, |found| found == hit),
            Some(hit)
        );
        assert_ne!(renderer.hit_test(x + width, y + height), Some(hit));
    }
}

#[test]
fn log_hit_test_regions() {
    let r = Renderer::new(96, LOG_W, LOG_H);
    assert_eq!(r.log_hit_test(LOG_W - 1, 0, None), Some(LogHit::Close));
    assert_eq!(r.log_hit_test(379, 10, None), None);
    for (filter, _, x, w) in LOG_TABS {
        assert_eq!(
            r.log_hit_test(x, LOG_TAB_Y, None),
            Some(LogHit::Tab(filter))
        );
        assert_eq!(
            r.log_hit_test(x + w - 1, LOG_TAB_Y + LOG_TAB_H - 1, None),
            Some(LogHit::Tab(filter))
        );
    }
    assert_eq!(r.log_hit_test(20, LOG_TAB_Y + 2, None), Some(LogHit::Title));
    assert_eq!(
        r.log_hit_test(100, LOG_TAB_Y + 2, None),
        Some(LogHit::Count)
    );
    assert!(!LogHit::Count.clickable());
    assert!(LogHit::Close.clickable());
    let (tx, ty, _, th) = LOG_TRACK;
    assert_eq!(r.log_hit_test(tx, ty, None), None);
    assert_eq!(
        r.log_hit_test(tx, ty + 10, Some((0, 40))),
        Some(LogHit::Thumb)
    );
    assert_eq!(
        r.log_hit_test(tx, ty + 40, Some((0, 40))),
        Some(LogHit::Track)
    );
    assert_eq!(r.log_hit_test(tx, ty + th, Some((0, 40))), None);
    let (bx, by, bw, bh) = LOG_BODY;
    assert!(bx + bw <= tx);
    assert_eq!(by + bh, LOG_H - 10);
}

#[test]
fn mix_endpoints() {
    assert_eq!(mix(ACCENT, TEXT, 0.0), ACCENT);
    assert_eq!(mix(ACCENT, TEXT, 1.0), TEXT);
}

#[test]
fn draw_smoke_bars() {
    const P: &str = "2026.09.07 09:12:28 Debug      -  ";
    let mut gs = smoke_gs();
    let mut r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    let f = frame();
    r.draw_main(&gs, &f);
    assert_eq!(pix(&r, 0, 0), BG);
    assert_eq!(pix(&r, 100, 55), GOOD);
    assert_eq!(pix(&r, 300, 55), CARD);
    assert_eq!(pix(&r, 100, 256), mix(CARD, BG, 0.55));
    assert_eq!(pix(&r, 20, 96), mix(CARD, DANGER, 0.08));
    assert_eq!(pix(&r, 336, 11), CTRL_HOVER);
    assert_eq!(pix(&r, 20, 346), CARD);
    let tk = TAKEN_CARD_Y + GRAPH_DY + BODY_DY;
    let hot = mix(CARD, DANGER, 0.7);
    assert_eq!(pix(&r, 196, tk + 199), hot);
    assert_eq!(pix(&r, 254, tk + 221), BG);
    assert_eq!(pix(&r, 254, tk + 243), CARD);
    assert_eq!(pix(&r, 20, 166), mix(CARD, DANGER, 0.08));
    let warned = Frame {
        warn_t: 0.5,
        ..frame()
    };
    r.draw_main(&gs, &warned);
    assert_eq!(pix(&r, 20, 166), mix(BG, AMBER, 0.18));
    assert!(text_w(&r, F_BIG, "COLLECT YOUR TOKENS") <= LOGICAL_W - 28);
    r.draw_main(&gs, &frame());
    assert_eq!(pix(&r, 20, 166), mix(CARD, DANGER, 0.08));

    for env in [Env::NotInWorld, Env::NoVrchat] {
        let away = Frame { env, ..frame() };
        r.draw_main(&gs, &away);
        assert_eq!(pix(&r, 100, 55), BG);
        assert_eq!(pix(&r, 100, 240), CARD);
        assert_eq!(pix(&r, 196, tk + 199), CARD);
    }
    gs.feed(&format!("{P}ECLIPTICA - now in lobby"));
    let ended = Frame {
        view_run: None,
        view_group: None,
        pages: 2,
        view_page: 1,
        ..frame()
    };
    r.draw_main(&gs, &ended);
    assert_eq!(pix(&r, 100, 55), BG);
    assert_eq!(pix(&r, 100, 240), CARD);
    assert_eq!(pix(&r, 196, tk + 199), CARD);
    r.draw_main(&gs, &frame());
    let settings = Frame {
        flags: frame().flags.with(Flag::SettingsOpen, true),
        scale: 2.0,
        alpha: 30,
        ..frame()
    };
    r.draw_main(&gs, &settings);
    assert_eq!(pix(&r, 120, 106), CARD_HI);
    assert_eq!(pix(&r, 120, 156), CARD_HI);
    assert_eq!(pix(&r, 120, 242), CARD_HI);
    r.draw_main(&gs, &frame());
    assert_eq!(pix(&r, 20, 106), CARD);
    assert_eq!(pix(&r, 120, 238), CARD);
}

#[test]
fn draw_smoke_cards() {
    const P: &str = "2026.09.07 09:12:28 Debug      -  ";
    let tk = TAKEN_CARD_Y + GRAPH_DY + BODY_DY;
    let hot = mix(CARD, DANGER, 0.7);
    let mut r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    let mut gs = smoke_gs();
    gs.feed(&format!("{P}ECLIPTICA - now in lobby"));
    let mut pre = GameState::default();
    pre.feed(&format!(
        "{P}ECLIPTICA - now in stage: Stage_Test on phase: 0.5 as class: Blade"
    ));
    pre.feed(&format!("{P}Dealing 100 STRIKE damage"));
    pre.feed(&format!(
        "{P}damage has been taken: 12, from source: machinegunShooter1"
    ));
    r.draw_main(&pre, &frame());
    assert_eq!(pix(&r, 100, 240), CARD);
    assert_eq!(pix(&r, 196, tk + 199), hot);
    assert_eq!(pix(&r, 254, tk + 221), CARD);
    pre.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.5"
    ));
    pre.feed(&format!("{P}Boss Kakarot dead, personal damage dealt: "));
    pre.feed(&format!("{P}STRIKE DMG: 300"));
    pre.feed(&format!("{P}NON-STRIKE DMG: 0"));
    pre.feed(&format!("{P}ECLIPTICA - now in intermission"));
    r.draw_main(&pre, &frame());
    assert_eq!(pix(&r, 100, 240), CARD);
    let flashing = Frame {
        taken_flash_t: 0.0,
        hover: Some(Hit::Log),
        pressed: Some(Hit::RunPrev),
        ..frame()
    };
    r.draw_main(&gs, &flashing);
    assert_ne!(pix(&r, 20, 346), CARD);
    assert_eq!(pix(&r, 304, 11), CTRL_HOVER);

    let group = Frame {
        flags: frame()
            .flags
            .with(Flag::RunSelected, true)
            .with(Flag::GroupSelected, true),

        ..frame()
    };
    r.draw_main(&gs, &group);
    assert_eq!(pix(&r, 196, tk + 199), hot);
    assert_eq!(pix(&r, 254, tk + 243), CARD);

    let tipped = Frame {
        tip: Some(Hit::Log),
        ..frame()
    };
    r.draw_main(&gs, &tipped);
    assert_eq!(pix(&r, 300, 45), CARD_HI);
    assert!(!tip_ready(None));
    assert!(!tip_ready(Some(std::time::Instant::now())));
}

#[test]
fn info_regions_stay_apart() {
    let r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    for (info, (x, y, w, h)) in INFO_REGIONS {
        let hit = Hit::Info(info);
        assert!(x >= 14 && x + w <= LOGICAL_W - 14, "{info:?}");
        assert!(y + h <= LOGICAL_H, "{info:?}");
        let only = |h: Hit| h == hit;
        assert_eq!(r.hit_test_where(x, y, only), Some(hit), "{info:?}");
        assert_eq!(
            r.hit_test_where(x + w - 1, y + h - 1, only),
            Some(hit),
            "{info:?}"
        );
        assert!(!hit.clickable());
        for live in [true, false] {
            let tip = info.tip(live);
            let unused = (info.live_only() && !live) || (info.history_only() && live);
            assert!(unused || !tip.is_empty(), "{info:?} live={live}");
            assert!(tip.chars().count() <= 70, "{info:?} live={live}");
        }
    }
    for (a, b) in INFO_REGIONS
        .iter()
        .flat_map(|a| INFO_REGIONS.iter().map(move |b| (a, b)))
    {
        if a.0 == b.0 {
            continue;
        }
        let both_live = !a.0.history_only() && !b.0.history_only();
        let both_hist = !a.0.live_only() && !b.0.live_only();
        let (ax, ay, aw, ah) = a.1;
        let (bx, by, bw, bh) = b.1;
        let overlap = ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah;
        assert!(
            !(overlap && (both_live || both_hist)),
            "{:?} overlaps {:?}",
            a.0,
            b.0
        );
    }
    assert!(Hit::Log.clickable());
    assert_eq!(r.hit_test(150, 36), Some(Hit::Info(Info::Status)));
    assert_eq!(r.hit_test(200, 30), Some(Hit::Party));
    assert_eq!(r.hit_test(200, 40), Some(Hit::Info(Info::Status)));
    assert_eq!(r.hit_test(200, 295), Some(Hit::Info(Info::LastKill)));
    assert_eq!(r.hit_test(200, 250), Some(Hit::Info(Info::DealtGraph)));
    assert_eq!(r.hit_test(30, 200), Some(Hit::Info(Info::Dealt(0))));
    assert_eq!(r.hit_test(300, 470), Some(Hit::Info(Info::TakenMore(2))));
    assert_eq!(r.hit_test(320, 200), Some(Hit::Stages));
    assert_eq!(r.hit_test(300, 205), Some(Hit::Info(Info::Dealt(2))));
    assert_eq!(r.hit_test(260, 205), Some(Hit::Info(Info::Dealt(2))));
    let only_splits = |x, y| r.hit_test_where(x, y, |hit| matches!(hit, Hit::Splits));
    assert_eq!(only_splits(20, 196), Some(Hit::Splits));
    assert_eq!(only_splits(340, 600), Some(Hit::Splits));
    assert_eq!(only_splits(20, 191), None);
    assert_eq!(only_splits(20, 603), None);
}

#[test]
fn tooltips_name_what_a_click_does() {
    let ctx = tip_ctx();
    assert!(Hit::Pin.tip(ctx).contains("keep it on top"));
    assert!(Hit::Discord.tip(ctx).starts_with("Show your run"));
    assert!(
        Hit::Discord
            .tip(TipCtx {
                flags: ctx.flags.with(Flag::Discord, true),
                ..ctx
            })
            .ends_with("click to stop")
    );
    assert!(Hit::Discord.tip(ctx).chars().count() <= 70);
    assert!(Hit::Settings.tip(ctx).chars().count() <= 70);
    for c in [
        ctx,
        TipCtx {
            flags: ctx.flags.with(Flag::Stages, true),
            ..ctx
        },
    ] {
        for h in [Hit::Stages, Hit::Splits] {
            assert!(h.tip(c).chars().count() <= 70, "{h:?}");
        }
    }
    assert!(
        Hit::Stages
            .tip(TipCtx {
                flags: ctx.flags.with(Flag::Stages, true),
                ..ctx
            })
            .starts_with("Back")
    );
    assert!(Hit::Usage.tip(ctx).starts_with("Send boss"));
    let on = TipCtx {
        flags: ctx
            .flags
            .with(Flag::Usage, true)
            .with(Flag::UsageFailing, false),

        ..ctx
    };
    assert!(Hit::Usage.tip(on).ends_with("click to stop"));
    for c in [ctx, on] {
        assert!(Hit::Usage.tip(c).chars().count() <= 70);
    }
}

#[test]
fn board_tooltips_follow_its_state() {
    let ctx = tip_ctx();
    let r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    let board = |board_on, board_status| TipCtx {
        flags: ctx.flags.with(Flag::Board, board_on),
        board_status,
        ..ctx
    };
    assert!(Hit::Board.tip(ctx).contains("ecliptica.whyknot.dev"));
    assert!(Hit::Board.tip(ctx).contains("VRChat name"));
    assert!(Hit::Board.tip(board(true, 0)).ends_with("click to stop"));
    assert!(
        Hit::Board
            .tip(board(true, crate::board::FAILING))
            .starts_with("Can't reach")
    );
    assert!(
        Hit::Board
            .tip(board(true, crate::board::TAKEN))
            .starts_with("Another PC")
    );
    assert_eq!(
        Hit::Board.tip(board(false, crate::board::TAKEN)),
        Hit::Board.tip(ctx)
    );
    for (on, st) in [(false, 0), (true, 0), (true, 1), (true, 2)] {
        let tip = Hit::Board.tip(board(on, st));
        assert!(tip.chars().count() <= 70, "{tip}");
        assert!(!tip.contains(['\u{2013}', '\u{2014}']), "{tip}");
    }
    assert!(Hit::ScaleUp.tip(ctx).contains("bigger"));
    assert!(Hit::Mute.tip(ctx).starts_with("Unmute"));
    assert_eq!(Hit::Sound.tip(ctx), Hit::Mute.tip(ctx));
    assert!(Hit::Party.tip(ctx).starts_with("Open"));
    assert!(
        Hit::Party
            .tip(TipCtx {
                flags: ctx.flags.with(Flag::PartyOpen, true),
                ..ctx
            })
            .starts_with("Close")
    );
    assert!(Hit::Log.tip(ctx).starts_with("Close"));
    assert_eq!(
        r.hit_test_where(340, LOGICAL_H - 20, |hit| hit != Hit::Update),
        Some(Hit::Info(Info::Version))
    );
}

#[test]
fn draw_log_smoke() {
    let mut gs = GameState::default();
    let mut r = Renderer::new(96, LOG_W, LOG_H);
    let mut lv = LogView {
        scroll: 0.0,
        filter: Filter::All,
        hover: None,
        dragging: false,
        thumb_t: 0.0,
        slide: 0.0,
        tip: None,
        thumb: None,
    };
    r.draw_log(timeline::live(&gs), &lv);
    assert_eq!(pix(&r, 0, 0), BG);
    let (_, _, ax, aw) = LOG_TABS[0];
    assert_eq!(pix(&r, ax + aw / 2, LOG_TAB_Y + 2), ACCENT);
    let (tx, ty, _, _) = LOG_TRACK;
    assert_eq!(pix(&r, tx + 3, ty + 3), BG);
    for i in 0..40 {
        gs.feed(&format!(
            "2026.09.07 09:12:{:02} Debug      -  damage has been taken: {i}, from source: attack_Spit",
            i % 60
        ));
    }
    gs.feed(
        "2026.09.07 09:13:00 Debug      -  ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0",
    );
    gs.feed("2026.09.07 09:13:01 Debug      -  ownership of Yuki transferred to Alice");
    lv.filter = Filter::Targets;
    r.draw_log(timeline::live(&gs), &lv);
    assert_eq!(pix(&r, tx + 3, ty + 3), BG);
    let (bx, by, _, _) = LOG_BODY;
    assert_eq!(pix(&r, bx + 1, by + 12), ACCENT);
    lv.filter = Filter::All;
    lv.scroll = timeline::max_scroll(42, Renderer::body_h());
    r.draw_log(timeline::live(&gs), &lv);
    assert_eq!(pix(&r, tx + 3, ty + 3), CARD);
    assert_eq!(pix(&r, tx + 3, LOG_H - 12), EDGE);
    assert_eq!(pix(&r, bx + 1, by + 12), DANGER);
}
