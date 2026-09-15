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

fn frame() -> Frame {
    Frame {
        now: 0,
        env: Env::InWorld,
        pages: 1,
        view_page: 0,
        settings_open: false,
        scale: 1.0,
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
        dps_frac_shown: 0.5,
        taken_frac_shown: 0.5,
        dps_shown: 0.0,
        fight_dps_shown: 0.0,
        taken_shown: 0.0,
        taken_rate_shown: 0.0,
        update: Badge::None,
        sound_on: true,
        discord: Link::Off,
        discord_on: false,
        view_run: Some(0),
        view_group: Some(0),
        group_pages: 1,
        run_sel: false,
        group_sel: false,
    }
}

fn pix(r: &Renderer, x: i32, y: i32) -> u32 {
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
        (SETTINGS_BTN, GLYPH_SETTINGS),
        (PIN_BTN, GLYPH_PIN),
        (LOG_BTN, GLYPH_LOG),
        (CLOSE_BTN, GLYPH_CLOSE),
    ];
    let mut spans = Vec::new();
    for (b, g) in buttons {
        r.glyph_button(b, false, false, false, g);
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
    let (dx, _, dw, _) = DISCORD_HIT;
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

#[test]
fn hit_test_regions() {
    let r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
    let (tx, ty, tw, th) = TARGET_HIT;
    assert!(tx >= 14 && tx + tw <= LOGICAL_W - 14);
    assert!(ty >= 82 && ty + th <= 178);
    assert_eq!(r.hit_test(tx, ty), Some(Hit::Target));
    assert_eq!(r.hit_test(tx + tw - 1, ty + th - 1), Some(Hit::Target));
    assert_eq!(r.hit_test(tx - 1, ty), None);
    assert_eq!(r.hit_test(tx, ty + th), None);
    assert_eq!(r.hit_test(tx + tw, ty), None);
    assert_eq!(r.hit_test(LOGICAL_W - 1, 0), Some(Hit::Close));
    assert_eq!(r.hit_test(324, 35), Some(Hit::Close));
    assert_eq!(r.hit_test(323, 35), Some(Hit::Log));
    assert_eq!(r.hit_test(292, 0), Some(Hit::Log));
    assert_eq!(r.hit_test(291, 0), Some(Hit::Pin));
    assert_eq!(r.hit_test(260, 35), Some(Hit::Pin));
    assert_eq!(r.hit_test(259, 0), Some(Hit::Settings));
    assert_eq!(r.hit_test(228, 35), Some(Hit::Settings));
    assert_eq!(r.hit_test(227, 0), None);
    assert_eq!(SETTINGS_HIT.0 + SETTINGS_HIT.2, PIN_HIT.0);
    for (rect, hit) in [
        (SCALE_DOWN_HIT, Hit::ScaleDown),
        (SCALE_UP_HIT, Hit::ScaleUp),
        (ALPHA_DOWN_HIT, Hit::AlphaDown),
        (ALPHA_UP_HIT, Hit::AlphaUp),
        (WINDOW_DOWN_HIT, Hit::WindowDown),
        (WINDOW_UP_HIT, Hit::WindowUp),
    ] {
        let (x, y, w, h) = rect;
        assert_eq!(r.hit_test(x, y), Some(hit));
        assert_eq!(r.hit_test(x + w - 1, y + h - 1), Some(hit));
        assert!(x >= 14 && x + w <= LOGICAL_W - 14 && y >= 82 && y + h <= 266);
    }
    let (px_, py_, pw_, ph_) = PIN_BTN;
    let (hx_, hy_, hw_, hh_) = PIN_HIT;
    assert!(px_ >= hx_ && py_ >= hy_ && px_ + pw_ <= hx_ + hw_ && py_ + ph_ <= hy_ + hh_);
    assert_eq!(hx_ + hw_, LOG_HIT.0);
    assert_eq!(r.hit_test(323, 36), Some(Hit::Info(Info::Status)));
    assert_eq!(r.hit_test(230, LOGICAL_H - 32), Some(Hit::Update));
    assert_eq!(r.hit_test(229, LOGICAL_H - 1), None);
    let (dx, dy, dw, dh) = DISCORD_HIT;
    assert_eq!(r.hit_test(dx, dy), Some(Hit::Discord));
    assert_eq!(r.hit_test(dx + dw - 1, dy + dh - 1), Some(Hit::Discord));
    assert_ne!(r.hit_test(dx + dw, dy), Some(Hit::Discord));
    assert!(dx + dw <= UPDATE_HIT.0);
    for (rect, hit) in [
        (RUN_PREV_HIT, Hit::RunPrev),
        (RUN_NEXT_HIT, Hit::RunNext),
        (FIGHT_PREV_HIT, Hit::FightPrev),
        (FIGHT_NEXT_HIT, Hit::FightNext),
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
    assert_eq!(pix(&r, 100, 49), GOOD);
    assert_eq!(pix(&r, 300, 49), CARD);
    assert_eq!(pix(&r, 100, 234), ACCENT);
    assert_eq!(pix(&r, 20, 90), CARD);
    assert_eq!(pix(&r, 336, 11), CARD_HI);
    assert_eq!(pix(&r, 20, 300), CARD);
    assert_eq!(pix(&r, 100, 397), DANGER);
    assert_eq!(pix(&r, 300, 397), BG);
    assert_eq!(pix(&r, 30, 449), DANGER);
    assert_eq!(pix(&r, 20, 160), CARD);
    let warned = Frame {
        warn_t: 0.5,
        ..frame()
    };
    r.draw_main(&gs, &warned);
    assert_eq!(pix(&r, 20, 160), mix(BG, AMBER, 0.18));
    assert!(text_w(&r, F_BIG, "COLLECT YOUR TOKENS") <= LOGICAL_W - 28);
    r.draw_main(&gs, &frame());
    assert_eq!(pix(&r, 20, 160), CARD);

    for env in [Env::NotInWorld, Env::NoVrchat] {
        let away = Frame { env, ..frame() };
        r.draw_main(&gs, &away);
        assert_eq!(pix(&r, 100, 49), BG);
        assert_eq!(pix(&r, 100, 234), CARD);
        assert_eq!(pix(&r, 100, 397), CARD);
        assert_eq!(pix(&r, 30, 449), CARD);
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
    assert_eq!(pix(&r, 100, 49), BG);
    assert_eq!(pix(&r, 100, 234), CARD);
    assert_eq!(pix(&r, 100, 397), CARD);
    assert_eq!(pix(&r, 30, 449), CARD);
    r.draw_main(&gs, &frame());
    let settings = Frame {
        settings_open: true,
        scale: 2.0,
        alpha: 30,
        ..frame()
    };
    r.draw_main(&gs, &settings);
    assert_eq!(pix(&r, 120, 100), CARD_HI);
    assert_eq!(pix(&r, 120, 150), CARD_HI);
    assert_eq!(pix(&r, 120, 250), CARD_HI);
    r.draw_main(&gs, &frame());
    assert_eq!(pix(&r, 120, 100), CARD);
    assert_eq!(pix(&r, 120, 250), CARD);

    let mut pre = GameState::default();
    pre.feed(&format!(
        "{P}ECLIPTICA - now in stage: Stage_Test on phase: 0.5 as class: Blade"
    ));
    pre.feed(&format!("{P}Dealing 100 STRIKE damage"));
    pre.feed(&format!(
        "{P}damage has been taken: 12, from source: machinegunShooter1"
    ));
    r.draw_main(&pre, &frame());
    assert_eq!(pix(&r, 100, 234), CARD);
    assert_eq!(pix(&r, 100, 397), CARD);
    pre.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.5"
    ));
    pre.feed(&format!("{P}Boss Kakarot dead, personal damage dealt: "));
    pre.feed(&format!("{P}STRIKE DMG: 300"));
    pre.feed(&format!("{P}NON-STRIKE DMG: 0"));
    pre.feed(&format!("{P}ECLIPTICA - now in intermission"));
    r.draw_main(&pre, &frame());
    assert_eq!(pix(&r, 100, 234), CARD);

    let flashing = Frame {
        taken_flash_t: 0.0,
        hover: Some(Hit::Log),
        pressed: Some(Hit::RunPrev),
        ..frame()
    };
    r.draw_main(&gs, &flashing);
    assert_ne!(pix(&r, 20, 300), CARD);
    assert_eq!(pix(&r, 304, 11), CARD_HI);

    let group = Frame {
        run_sel: true,
        group_sel: true,
        ..frame()
    };
    r.draw_main(&gs, &group);
    assert_eq!(pix(&r, 300, 397), CARD);
    assert_eq!(pix(&r, 30, 419), DANGER);

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
    assert_eq!(r.hit_test(200, 30), Some(Hit::Info(Info::Status)));
    assert_eq!(r.hit_test(200, 250), Some(Hit::Info(Info::LastKill)));
    assert_eq!(r.hit_test(30, 200), Some(Hit::Info(Info::Dealt(0))));
    assert_eq!(r.hit_test(300, 370), Some(Hit::Info(Info::TakenMore(2))));
    let ctx = TipCtx {
        live: true,
        topmost: false,
        sound_on: false,
        log_open: true,
        discord_on: false,
    };
    assert!(Hit::Pin.tip(ctx).contains("keep it on top"));
    assert!(Hit::Discord.tip(ctx).starts_with("Click to show"));
    assert!(Hit::Discord
        .tip(TipCtx {
            discord_on: true,
            ..ctx
        })
        .ends_with("click to stop"));
    assert!(Hit::Discord.tip(ctx).chars().count() <= 70);
    assert!(Hit::Settings.tip(ctx).chars().count() <= 70);
    assert!(Hit::ScaleUp.tip(ctx).contains("bigger"));
    assert!(Hit::Target.tip(ctx).contains("unmute"));
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
    assert_eq!(pix(&r, tx + 3, LOG_H - 12), CARD_HI);
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
