use super::hit::{TipCtx, INFO_REGIONS};
use super::layout::*;
use super::theme::*;
use super::*;
use crate::discord::Link;
use crate::game::state::GameState;
use crate::hud::timeline::{self, Filter};
use crate::update::Badge;
use crate::vr::VrStatus;
use windows_sys::Win32::Graphics::Gdi::*;

pub(super) fn frame() -> Frame {
    Frame {
        now: 0,
        env: Env::InWorld,
        pages: 1,
        view_page: 0,
        settings_open: false,
        scale: 1.0,
        scale_max: MAX_SCALE,
        alpha: 100,
        window: 10,
        flash_t: 1.0,
        taken_flash_t: 1.0,
        warn_t: 1.0,
        dead_pulse: 0.0,
        hover: Some(Hit::Close),
        pressed: None,
        tip: None,
        vr: VrStatus::Off,
        log_ok: true,
        log_open: false,
        topmost: true,
        progress_shown: 0.5,
        dps_shown: 0.0,
        fight_dps_shown: 0.0,
        taken_shown: 0.0,
        taken_rate_shown: 0.0,
        update: Badge::None,
        prompt: false,
        sound_on: true,
        discord: Link::Off,
        discord_on: false,
        usage_on: false,
        usage_failing: false,
        board_on: false,
        board_status: 0,
        board_delete: 0,
        ask_usage: false,
        party_open: false,
        view_run: Some(0),
        view_group: Some(0),
        group_pages: 1,
        run_sel: false,
        group_sel: false,
        stages_btn: false,
        stages: false,
        split_top: 0,
    }
}

pub(super) fn pix(r: &Renderer, x: i32, y: i32) -> u32 {
    let s = unsafe { std::slice::from_raw_parts(r.bits, (r.width * r.height * 4) as usize) };
    let i = ((y * r.width + x) * 4) as usize;
    rgb(s[i + 2] as u32, s[i + 1] as u32, s[i] as u32)
}

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

fn text_w(r: &Renderer, font: usize, s: &str) -> i32 {
    let buf: Vec<u16> = s.encode_utf16().collect();
    let mut size = windows_sys::Win32::Foundation::SIZE { cx: 0, cy: 0 };
    unsafe {
        SelectObject(r.dc, r.fonts[font] as _);
        GetTextExtentPoint32W(r.dc, buf.as_ptr(), buf.len() as i32, &mut size);
    }
    size.cx
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
    println!("toolbar ink spans {spans:?} gaps {gaps:?}");
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
        let v = ease_out_cubic(i as f32 / 10.0);
        assert!(v >= prev);
        prev = v;
    }
    assert_eq!(lerp(2.0, 6.0, 0.0), 2.0);
    assert_eq!(lerp(2.0, 6.0, 1.0), 6.0);
}

fn settings_hit(h: Hit) -> bool {
    matches!(
        h,
        Hit::ScaleDown
            | Hit::ScaleUp
            | Hit::AlphaDown
            | Hit::AlphaUp
            | Hit::WindowDown
            | Hit::WindowUp
            | Hit::Sound
            | Hit::Discord
            | Hit::Usage
    )
}

#[test]
fn hit_test_regions() {
    let r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    let closed = |x, y| r.hit_test_where(x, y, |h| !settings_hit(h));
    let (tx, ty, tw, th) = Hit::Mute.rect();
    assert!(tx >= 14 && tx + tw <= LOGICAL_W - 14);
    assert!(ty >= 82 && ty + th <= 178);
    assert_eq!(closed(tx, ty), Some(Hit::Mute));
    assert_eq!(closed(tx + tw - 1, ty + th - 1), Some(Hit::Mute));
    assert_ne!(closed(tx - 1, ty), Some(Hit::Mute));
    assert_ne!(closed(tx, ty + th), Some(Hit::Mute));
    assert_eq!(closed(tx + tw, ty), None);
    assert_eq!(closed(200, 140), Some(Hit::Info(Info::Result)));
    assert_eq!(r.hit_test(LOGICAL_W - 1, 0), Some(Hit::Close));
    assert_eq!(r.hit_test(324, 35), Some(Hit::Close));
    assert_eq!(r.hit_test(323, 35), Some(Hit::Log));
    assert_eq!(r.hit_test(292, 0), Some(Hit::Log));
    assert_eq!(r.hit_test(291, 0), Some(Hit::Pin));
    assert_eq!(r.hit_test(260, 35), Some(Hit::Pin));
    assert_eq!(r.hit_test(259, 0), Some(Hit::Settings));
    assert_eq!(r.hit_test(228, 35), Some(Hit::Settings));
    assert_eq!(r.hit_test(227, 0), Some(Hit::Party));
    assert_eq!(r.hit_test(196, 35), Some(Hit::Party));
    assert_eq!(r.hit_test(195, 0), None);
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
        let (x, y, w, h) = rect;
        assert!(settings_hit(hit));
        assert_eq!(r.hit_test(x, y), Some(hit));
        assert_eq!(r.hit_test(x + w - 1, y + h - 1), Some(hit));
        let (cx, cy, cw, ch) = SETTINGS_CARD;
        let cy = cy + BODY_DY;
        assert!(x >= cx + 12 && x + w <= cx + cw - 12, "{hit:?}");
        assert!(y >= cy + 12 && y + h <= cy + ch - 12, "{hit:?}");
    }
    let (px_, py_, pw_, ph_) = PIN_BTN;
    let (hx_, hy_, hw_, hh_) = PIN_HIT;
    assert!(px_ >= hx_ && py_ >= hy_ && px_ + pw_ <= hx_ + hw_ && py_ + ph_ <= hy_ + hh_);
    assert_eq!(hx_ + hw_, LOG_HIT.0);
    assert_eq!(r.hit_test(323, 36), Some(Hit::Info(Info::Status)));
    assert_eq!(r.hit_test(230, LOGICAL_H - 32), Some(Hit::Update));
    assert_eq!(r.hit_test(229, LOGICAL_H - 1), None);
    let (dx, dy, dw, dh) = DISCORD_DOT;
    let dot = Some(Hit::Info(Info::Discord));
    assert_eq!(r.hit_test(dx, dy), dot);
    assert_eq!(r.hit_test(dx + dw - 1, dy + dh - 1), dot);
    assert_ne!(r.hit_test(dx + dw, dy), dot);
    assert!(dx + dw <= UPDATE_HIT.0);
    for (rect, hit) in [
        (Hit::RunPrev.rect(), Hit::RunPrev),
        (Hit::RunNext.rect(), Hit::RunNext),
        (Hit::FightPrev.rect(), Hit::FightPrev),
        (Hit::FightNext.rect(), Hit::FightNext),
    ] {
        let (x, y, w, h) = rect;
        assert_eq!(r.hit_test_where(x, y, |h| h == hit), Some(hit));
        assert_eq!(
            r.hit_test_where(x + w - 1, y + h - 1, |h| h == hit),
            Some(hit)
        );
        assert_ne!(r.hit_test(x + w, y + h), Some(hit));
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
    let mut gs = GameState::default();
    gs.feed(&format!(
        "{P}ECLIPTICA - now in stage: Stage_Test on phase: 0.5 as class: Blade"
    ));
    gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.5"
    ));
    gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: KakarotPhase2(Clone) on phase: 0.5"
    ));
    gs.feed(&format!(
        "{P}damage has been taken: 12, from source: (Kakarot) attack_Kick"
    ));
    gs.feed(&format!("{P}damage has been taken: 3, from source: "));
    gs.feed(&format!("{P}Local controller dead, switching off."));
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
        settings_open: true,
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
        run_sel: true,
        group_sel: true,
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
    let only_splits = |x, y| r.hit_test_where(x, y, |h| matches!(h, Hit::Splits));
    assert_eq!(only_splits(20, 196), Some(Hit::Splits));
    assert_eq!(only_splits(340, 600), Some(Hit::Splits));
    assert_eq!(only_splits(20, 191), None);
    assert_eq!(only_splits(20, 603), None);
    let ctx = TipCtx {
        live: true,
        topmost: false,
        sound_on: false,
        log_open: true,
        discord_on: false,
        usage_on: false,
        usage_failing: false,
        board_on: false,
        board_status: 0,
        board_delete: 0,
        party_open: false,
        stages: false,
    };
    assert!(Hit::Pin.tip(ctx).contains("keep it on top"));
    assert!(Hit::Discord.tip(ctx).starts_with("Show your run"));
    assert!(Hit::Discord
        .tip(TipCtx {
            discord_on: true,
            ..ctx
        })
        .ends_with("click to stop"));
    assert!(Hit::Discord.tip(ctx).chars().count() <= 70);
    assert!(Hit::Settings.tip(ctx).chars().count() <= 70);
    for c in [
        ctx,
        TipCtx {
            stages: true,
            ..ctx
        },
    ] {
        for h in [Hit::Stages, Hit::Splits] {
            assert!(h.tip(c).chars().count() <= 70, "{h:?}");
        }
    }
    assert!(Hit::Stages
        .tip(TipCtx {
            stages: true,
            ..ctx
        })
        .starts_with("Back"));
    assert!(Hit::Usage.tip(ctx).starts_with("Send boss"));
    let on = TipCtx {
        usage_on: true,
        usage_failing: false,
        ..ctx
    };
    assert!(Hit::Usage.tip(on).ends_with("click to stop"));
    for c in [ctx, on] {
        assert!(Hit::Usage.tip(c).chars().count() <= 70);
    }
    let board = |board_on, board_status| TipCtx {
        board_on,
        board_status,
        ..ctx
    };
    assert!(Hit::Board.tip(ctx).contains("ecliptica.whyknot.dev"));
    assert!(Hit::Board.tip(ctx).contains("VRChat name"));
    assert!(Hit::Board.tip(board(true, 0)).ends_with("click to stop"));
    assert!(Hit::Board
        .tip(board(true, crate::board::FAILING))
        .starts_with("Can't reach"));
    assert!(Hit::Board
        .tip(board(true, crate::board::TAKEN))
        .starts_with("Another PC"));
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
    assert!(Hit::Party
        .tip(TipCtx {
            party_open: true,
            ..ctx
        })
        .starts_with("Close"));
    assert!(Hit::Log.tip(ctx).starts_with("Close"));
    assert_eq!(
        r.hit_test_where(340, LOGICAL_H - 20, |h| h != Hit::Update),
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
    lv.scroll = timeline::max_scroll(42, r.body_h());
    r.draw_log(timeline::live(&gs), &lv);
    assert_eq!(pix(&r, tx + 3, ty + 3), CARD);
    assert_eq!(pix(&r, tx + 3, LOG_H - 12), EDGE);
    assert_eq!(pix(&r, bx + 1, by + 12), DANGER);
}

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
        run_sel: true,
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

fn fight_log(secs: u64, me_targeted: bool) -> (GameState, u64) {
    let line = |t: u64, m: &str| {
        format!(
            "2026.09.07 {:02}:{:02}:{:02} Debug      -  {m}",
            9 + t / 3600,
            t / 60 % 60,
            t % 60
        )
    };
    let mut gs = GameState::default();
    gs.feed(&line(0, "User Authenticated: Me (Alt) (usr_1)"));
    gs.feed(&line(
        0,
        "ECLIPTICA - now in stage: Stage_Test on phase: 0.5 as class: Blade",
    ));
    gs.feed(&line(
        1,
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.5",
    ));
    for t in 2..secs {
        gs.feed(&line(t, &format!("Dealing {} STRIKE damage", t * 10)));
    }
    let who = if me_targeted { "Me (Alt)" } else { "Someone" };
    gs.feed(&line(
        secs,
        &format!("ownership of Kakarot transferred to {who}"),
    ));
    let now = crate::game::event::split_line(&line(secs, "x")).unwrap().ts;
    (gs, now)
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
    let g = run.group(0).unwrap().graph(now, super::graph::GRAPH_COLS);
    assert!(g.cols.len() <= super::graph::GRAPH_COLS);
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

fn prompt_frame(update: Badge) -> Frame {
    Frame {
        update,
        prompt: true,
        hover: None,
        ..frame()
    }
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
        let s = dpi as i32 / 96;
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
    let r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    for (rect, hit) in [
        (UPDATE_NOW_HIT, Hit::UpdateNow),
        (UPDATE_LATER_HIT, Hit::UpdateLater),
        (UPDATE_SKIP_HIT, Hit::UpdateSkip),
    ] {
        let (x, y, w, h) = rect;
        let only = |h| h == hit;
        assert_eq!(r.hit_test_where(x, y, only), Some(hit));
        assert_eq!(r.hit_test_where(x + w - 1, y + h - 1, only), Some(hit));
        assert_eq!(r.hit_test_where(x + w, y + h, only), None);
        assert!(y >= PROMPT_AREA.0 && y + h <= PROMPT_AREA.0 + PROMPT_AREA.1);
    }
}

fn main_frame() -> Frame {
    Frame {
        view_run: None,
        view_group: None,
        ..frame()
    }
}

pub(super) fn party_row(
    name: &str,
    you: bool,
    live: bool,
    dmg: u64,
    taken: u64,
    share: Option<u32>,
    dps: u64,
) -> PartyRow {
    PartyRow {
        name: name.to_string(),
        you,
        live,
        dmg,
        taken,
        life: 0,
        share,
        dps,
    }
}

pub(super) fn log_view(filter: Filter) -> LogView {
    LogView {
        scroll: 0.0,
        filter,
        hover: None,
        dragging: false,
        thumb_t: 0.0,
        slide: 0.0,
        tip: None,
        thumb: None,
    }
}

pub(super) fn pane(rows: Vec<PartyRow>) -> PartyPane {
    PartyPane {
        on: true,
        ready: true,
        link: crate::relay::Link::Connected,
        here: rows.len() + 2,
        fight: Some(PartyFight {
            name: "Ha'kli".to_string(),
            secs: 83,
            live: true,
            down: false,
        }),
        rows,
        ..PartyPane::default()
    }
}

pub(super) fn sample_party() -> Vec<PartyRow> {
    vec![
        party_row(
            "Be\u{430}r (Hands)",
            false,
            true,
            1_284_310,
            4_120,
            Some(41),
            18_420,
        ),
        party_row("Me (Alt)", true, true, 902_115, 18_904, Some(29), 12_007),
        party_row("Mira", false, true, 610_002, 655, Some(19), 9_880),
        party_row(
            "\u{6c17}\u{307e}\u{3050}\u{308c} Sora",
            false,
            false,
            230_440,
            31_377,
            Some(7),
            3_120,
        ),
        party_row("Tobias_77", false, true, 118_900, 0, Some(4), 2_500),
    ]
}

pub(super) fn stress_party(long_names: bool, long_numbers: bool) -> Vec<PartyRow> {
    let scripts = ["W", "\u{6c17}", "\u{430}", "\u{1d04}", "m", "\u{5d0}"];
    (0..MAX_MEMBERS)
        .map(|i| {
            let name = if long_names {
                scripts[i % scripts.len()].repeat(400)
            } else {
                format!("P{i}")
            };
            let (dmg, dps) = if long_numbers {
                (MAX_DMG - i as u64, u64::MAX - i as u64)
            } else {
                (5_000 - i as u64, 120)
            };
            party_row(&name, i == 3, i % 5 != 4, dmg, dmg, Some(100), dps)
        })
        .collect()
}

use crate::relay::party::PartyRow;
use crate::relay::proto::{MAX_DMG, MAX_MEMBERS};

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

fn tip_fits(r: &Renderer, tip: &str, win_w: i32) -> (i32, usize) {
    let (w, lines) = r.tip_lines(tip, win_w);
    assert!(w <= win_w - 16, "{tip}: {w}");
    assert!(lines.len() <= 2, "{tip}");
    assert_eq!(lines.join(" "), tip);
    for line in &lines {
        let lw = text_w(r, F_TINY, line);
        assert!(
            lw + r.px(TIP_INSET) <= r.px(w),
            "{line:?} is {lw} px in a {} px tip",
            r.px(w)
        );
    }
    (w, lines.len())
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
            assert_eq!(w, (tip.chars().count() as i32 * 6 + 20).min(LOGICAL_W - 16));
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
    use super::fmt::compact;
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
                u => super::fmt::UNITS.iter().position(|x| *x == u).unwrap() as i32 + 1,
            };
            let num: f64 = s[..s.len() - unit.len()].parse().unwrap();
            let back = num * 1000f64.powi(k);
            assert!(
                ((back - v as f64) / v.max(1) as f64).abs() < 0.006,
                "{v} {s}"
            );
        }
        n = next;
    }
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

pub(super) fn png(w: u32, h: u32, rgb: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((w * 3 + 1) as usize * h as usize);
    for row in rgb.chunks(w as usize * 3) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, b) in blocks.iter().enumerate() {
        z.push((i + 1 == blocks.len()) as u8);
        z.extend_from_slice(&(b.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(b.len() as u16)).to_le_bytes());
        z.extend_from_slice(b);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    out
}

struct Sheet {
    w: u32,
    h: u32,
    rgb: Vec<u8>,
}

impl Sheet {
    fn new(w: u32, h: u32) -> Self {
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for _ in 0..w * h {
            rgb.extend_from_slice(&[0x0b, 0x0b, 0x10]);
        }
        Sheet { w, h, rgb }
    }

    fn put(&mut self, r: &Renderer, x0: u32, y0: u32, clip: (i32, i32, i32, i32)) {
        let (cx, cy, cw, ch) = (r.px(clip.0), r.px(clip.1), r.px(clip.2), r.px(clip.3));
        for y in 0..ch {
            for x in 0..cw {
                let c = pix(r, cx + x, cy + y);
                let i = (((y0 + y as u32) * self.w + x0 + x as u32) * 3) as usize;
                self.rgb[i] = (c & 0xff) as u8;
                self.rgb[i + 1] = ((c >> 8) & 0xff) as u8;
                self.rgb[i + 2] = ((c >> 16) & 0xff) as u8;
            }
        }
    }

    fn save(&self, name: &str) {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".devflow")
            .join("pick");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), png(self.w, self.h, &self.rgb)).unwrap();
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
#[ignore]
fn write_tooltip_wrap_images() {
    let ctx = TipCtx {
        live: false,
        topmost: false,
        sound_on: false,
        log_open: false,
        discord_on: false,
        usage_on: false,
        usage_failing: false,
        board_on: false,
        board_status: 0,
        board_delete: 0,
        party_open: false,
        stages: false,
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
        .map(|(&(dpi, _), &(_, h))| ((label_h + h) * dpi / 96) as u32 + gap)
        .sum();
    let mut sheet = Sheet::new(gap * 3 + LOGICAL_W as u32 * 4, gap + height);
    let mut y = gap;
    for (&(dpi, hit), &(top, h)) in cases.iter().zip(&bands) {
        let s = dpi / 96;
        let near = hit.rect();
        let text = hit.tip(ctx);
        for (col, name) in ["A  before: one line", "B  after: wraps to fit"]
            .iter()
            .enumerate()
        {
            let x = gap + col as u32 * (LOGICAL_W * s) as u32 + col as u32 * gap;
            let l = Renderer::new(dpi as u32, LOGICAL_W, label_h);
            l.rround(0, 0, LOGICAL_W, label_h, 0, BG);
            l.text_rect(
                4,
                0,
                LOGICAL_W - 8,
                label_h,
                F_LABEL,
                TEXT,
                DT_VCENTER,
                name,
            );
            unsafe { GdiFlush() };
            sheet.put(&l, x, y, (0, 0, LOGICAL_W, label_h));
            let mut r = Renderer::new(dpi as u32, LOGICAL_W, LOGICAL_H);
            r.draw_main(
                &gs,
                &Frame {
                    settings_open: hit == Hit::Party,
                    hover: Some(hit),
                    tip: None,
                    ..main_frame()
                },
            );
            if col == 0 {
                let w = (text.chars().count() as i32 * 6 + 20).min(LOGICAL_W - 16);
                let tx = near.0.clamp(8, LOGICAL_W - 8 - w);
                let ty = near.1 + near.3 + 6;
                r.rround(tx, ty, w, 24, 6, ACCENT);
                r.rround(tx + 1, ty + 1, w - 2, 22, 5, CARD_HI);
                r.text_rect(tx, ty, w, 24, F_TINY, TEXT, DT_CENTER | DT_VCENTER, text);
            } else {
                r.tooltip(near, text, LOGICAL_W, LOGICAL_H);
            }
            unsafe { GdiFlush() };
            sheet.put(&r, x, y + (label_h * s) as u32, (0, top, LOGICAL_W, h));
        }
        y += ((label_h + h) * s) as u32 + gap;
    }
    sheet.save("tooltip_wrap.png");
}

#[test]
fn counts_use_the_singular_for_one() {
    use super::fmt::fmt_count;
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
    use super::main_panel::{strip_layout, STRIP_MIN, STRIP_W};
    for dpi in [96, 120, 144, 192] {
        let r = Renderer::new(dpi, LOGICAL_W, LOGICAL_H);
        assert!(r.unscale(r.text_w(F_TINY, "B99")) + 4 <= STRIP_MIN, "{dpi}");
    }
    for n in 1..120 {
        for cur in 0..n {
            let (shown, sw) = strip_layout(n, cur);
            let k = shown.len() as i32;
            assert!(shown.contains(&cur) && shown.end <= n, "{n} {cur}");
            assert!(
                sw >= STRIP_MIN && k * sw + (k - 1) * 4 <= STRIP_W,
                "{n} {cur}"
            );
        }
    }
}

#[test]
fn stage_table_never_leaves_its_card() {
    use super::fmt::compact;
    use super::main_panel::{fmt_split, COL, SPLIT_COLS};
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
        deaths: (i % 4) as u32 * 33,
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
    let used = 26 + 18 + SPLIT_ROWS as i32 * SPLIT_ROW_H + 20 + 6 + SPLIT_ROW_H;
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
                run_sel: true,
                group_sel: true,
                view_group: Some(20),
                group_pages: 30,
                stages_btn: true,
                stages: true,
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
            let lit_at = (0..SPLIT_ROWS as i32).find(|&i| pix(&r, r.px(M + 8), row_y(i)) == lit);
            let shown = split_top.min(30 - SPLIT_ROWS);
            let want = (shown..shown + SPLIT_ROWS)
                .contains(&20)
                .then(|| (20 - shown) as i32);
            assert_eq!(lit_at, want, "lit row at {dpi} top {split_top}");
        }
    }
}
