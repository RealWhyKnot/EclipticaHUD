use super::*;

#[test]
fn stage_table_toggles_follows_the_page_and_scrolls() {
    let mut app = headless();
    let stage = |i: u64| BossFight {
        name: "Stage_OuterHeven".into(),
        start_ts: 100 + i * 100,
        end_ts: Some(150 + i * 100),
        dmg: 10 + i,
        ..Default::default()
    };
    app.gs.runs.push(Run {
        start_ts: 100,
        end_ts: Some(5000),
        end: Some(RunEnd::Lost),
        stages: (0..20).map(stage).collect(),
        ..Default::default()
    });
    assert!(!app.hit_enabled(Hit::Stages));
    assert!(app.run_prev());
    assert!(app.hit_enabled(Hit::Stages));
    assert!(!app.hit_enabled(Hit::Splits));
    assert!(app.hit_enabled(Hit::Info(Info::Dealt(0))));
    assert!(app.activate(Hit::Stages));
    assert!(app.stages_shown());
    assert!(app.hit_enabled(Hit::Splits));
    for info in [
        Info::Dealt(0),
        Info::LastKill,
        Info::Taken(1),
        Info::TopAttacks,
    ] {
        assert!(!app.hit_enabled(Hit::Info(info)), "{info:?}");
    }
    assert!(app.hit_enabled(Hit::Info(Info::Pages)));
    assert_eq!(app.split_top, 20 - SPLIT_ROWS);
    assert!(app.split_wheel(120));
    assert_eq!(app.split_top, 3);
    assert!(!app.split_wheel(60));
    assert!(app.split_wheel(60));
    assert_eq!(app.split_top, 0);
    assert!(!app.split_wheel(120));
    assert!(app.split_wheel(-1200));
    assert_eq!(app.split_top, 20 - SPLIT_ROWS);
    while app.group_prev() {}
    assert_eq!(app.split_top, 0);
    app.flags.set(Flag::SettingsOpen, true);
    assert!(!app.hit_enabled(Hit::Stages) && !app.hit_enabled(Hit::Splits));
    app.flags.set(Flag::SettingsOpen, false);
    assert!(app.run_next());
    assert!(!app.stages_shown() && !app.hit_enabled(Hit::Splits));
    assert!(app.hit_enabled(Hit::Info(Info::Dealt(0))));
    assert!(app.run_prev());
    assert!(app.stages_shown());
    assert_eq!(app.split_top, 20 - SPLIT_ROWS);
    assert!(app.activate(Hit::Stages));
    assert!(!app.stages_shown());
    assert!(app.run_next());
    app.gs.feed(&format!("{P}{STAGE_A}"));
    assert!(app.is_live() && app.hit_enabled(Hit::Stages));
    assert!(app.activate(Hit::Stages));
    assert!(app.stages_shown() && app.hit_enabled(Hit::Splits));
    assert!(!app.hit_enabled(Hit::Info(Info::LastHit)));
    app.split_track();
    assert_eq!((app.splits().len(), app.split_top), (1, 0));
}

#[test]
fn killed_boss_moves_to_previous_page() {
    let mut app = headless();
    app.gs.feed(&format!("{P}{STAGE_A}"));
    assert_eq!(app.group_pages(), 1);
    assert!(app.viewed_group().is_none());
    assert!(!app.hit_enabled(Hit::FightPrev));
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Despair(Clone) on phase: 0"
    ));
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: DespairPhase2(Clone) on phase: 0"
    ));
    assert_eq!(app.group_pages(), 1);
    assert!(
        app.viewed_group()
            .is_some_and(|(i, g)| i == 0 && g == Page::Boss(0..2))
    );
    assert!(!app.hit_enabled(Hit::FightPrev));
    app.gs.feed(&format!(
        "{P}Boss DespairPhase2 dead, personal damage dealt: "
    ));
    app.gs.feed(&format!("{P}STRIKE DMG: 100"));
    app.gs.feed(&format!("{P}NON-STRIKE DMG: 0"));
    app.gs.feed(&format!("{P}ECLIPTICA - now in intermission"));
    app.gs.feed(&format!("{P}{STAGE_B}"));
    assert_eq!(app.group_pages(), 2);
    assert!(app.is_live());
    assert!(app.viewed_group().is_none());
    assert!(app.hit_enabled(Hit::FightPrev));
    assert!(app.group_prev());
    assert!(
        app.viewed_group()
            .is_some_and(|(i, g)| i == 0 && g == Page::Boss(0..2))
    );
    assert!(!app.group_prev());
    assert!(app.group_next());
    assert!(app.is_live());
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0"
    ));
    assert_eq!(app.group_pages(), 2);
    assert!(app.viewed_group().is_some_and(|(i, _)| i == 1));
    assert!(app.group_prev());
    assert!(app.viewed_group().is_some_and(|(i, _)| i == 0));
    assert!(app.group_next());
    assert!(app.is_live());
}

#[test]
fn token_warning_when_enemies_clear_with_tokens_missing() {
    let mut app = headless();
    let clock = std::cell::Cell::new(9 * 3600 + 13 * 60);
    let feed = |app: &mut App, msg: &str| {
        let t = clock.get();
        let (h, m, s) = (t / 3600, t / 60 % 60, t % 60);
        app.gs.feed(&format!(
            "2026.09.07 {h:02}:{m:02}:{s:02} Debug      -  {msg}"
        ))
    };
    let wait = |app: &mut App| {
        clock.set(clock.get() + crate::game::state::TOKEN_ALERT_SECS);
        feed(app, "waiting");
    };
    let spawn_stage = |app: &mut App, stage: &str| {
        for _ in 0..3 {
            feed(app, "spawn token, False, 0");
        }
        feed(app, stage);
    };
    feed(&mut app, "Retiring Enemy POOL ID0");
    spawn_stage(&mut app, STAGE_B);
    feed(&mut app, "Initializing Enemy POOL ID0 as ENEMY ID 1");
    feed(&mut app, "Retiring Enemy POOL ID0");
    wait(&mut app);
    app.tick();
    assert!(app.warn_at.is_none());
    spawn_stage(&mut app, STAGE_A);
    feed(&mut app, "ECLIPTICA saving SESSION ID 2505");
    feed(&mut app, "Initializing Enemy POOL ID0 as ENEMY ID 1");
    app.tick();
    assert!(app.warn_at.is_none());
    feed(&mut app, "Retiring Enemy POOL ID0");
    app.tick();
    assert!(app.warn_at.is_none());
    wait(&mut app);
    let tick = app.tick();
    assert!(app.warn_at.is_some());
    assert_eq!(tick.timer_ms, Some(ANIM_MS));
    app.warn_at = None;
    app.tick();
    assert!(app.warn_at.is_none());
    feed(&mut app, "Initializing Enemy POOL ID3 as ENEMY ID 1");
    feed(&mut app, "Retiring Enemy POOL ID3");
    feed(&mut app, "Initializing Enemy POOL ID3 as ENEMY ID 1");
    wait(&mut app);
    app.tick();
    assert!(app.warn_at.is_none());
    feed(&mut app, "Retiring Enemy POOL ID3");
    wait(&mut app);
    app.tick();
    assert!(app.warn_at.is_some());
    app.warn_at = None;
    feed(&mut app, "Initializing Enemy POOL ID3 as ENEMY ID 1");
    for _ in 0..2 {
        feed(&mut app, "ECLIPTICA saving SESSION ID 2505");
    }
    feed(&mut app, "Retiring Enemy POOL ID3");
    wait(&mut app);
    app.tick();
    assert!(app.warn_at.is_none());
}

#[test]
fn timer_settles_after_animation() {
    let mut app = headless();
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now in stage: Stage_Test on phase: 0.5 as class: Blade"
    ));
    let t = app.tick();
    assert!(t.redraw.main);
    assert_eq!(t.timer_ms, Some(ANIM_MS));
    for _ in 0..200 {
        if app.tick().timer_ms == Some(1000) {
            assert_eq!(app.progress_shown, 0.5);
            return;
        }
    }
    panic!("animation never settled");
}

#[test]
fn live_run_redraws_every_tick() {
    fn settled(app: &mut App) -> Tick {
        for _ in 0..200 {
            if app.tick().timer_ms.is_none() && app.timer_ms == 1000 {
                return app.tick();
            }
        }
        panic!("animation never settled");
    }
    let mut app = headless();
    app.gs.feed(&format!("{P}{STAGE_A}"));
    assert!(settled(&mut app).redraw.main);
    app.gs.feed(&format!("{P}ECLIPTICA - now in intermission"));
    assert!(settled(&mut app).redraw.main);
    app.gs.feed(&format!("{P}ECLIPTICA - now in lobby"));
    assert!(!settled(&mut app).redraw.main);
}

#[test]
fn tooltip_after_hover_delay() {
    let mut app = headless();
    app.tick();
    assert!(app.set_hover(Some(Hit::Log)));
    assert!(!app.set_hover(Some(Hit::Log)));
    assert_eq!(app.tip(), None);
    let t = app.tick();
    assert_eq!(t.timer_ms, Some(ANIM_MS));
    std::thread::sleep(std::time::Duration::from_millis(500));
    let t = app.tick();
    assert_eq!(app.tip(), Some(Hit::Log));
    assert!(t.redraw.main);
    assert_eq!(t.timer_ms, Some(1000));
    assert!(!app.tick().redraw.main);
    assert!(app.set_hover(None));
    assert_eq!(app.tip(), None);
    app.log_open();
    assert!(app.log_set_hover(Some(LogHit::Close)));
    assert_eq!(app.log_tip(), None);
    app.tick();
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(app.tick().redraw.log);
    assert_eq!(app.log_tip(), Some(LogHit::Close));
    assert!(!app.tick().redraw.log);
}

#[test]
fn taken_pulse_only_after_first_tick() {
    let mut app = headless();
    app.gs
        .feed(&format!("{P}damage has been taken: 5, from source: "));
    app.tick();
    assert!(app.taken_flash_at.is_none());
    app.gs
        .feed(&format!("{P}damage has been taken: 5, from source: "));
    let t = app.tick();
    assert!(app.taken_flash_at.is_some());
    assert!(t.timer_ms.is_none_or(|ms| ms == ANIM_MS));
    assert_eq!(app.timer_ms, ANIM_MS);
    assert!(app.taken_shown > 0.0);
}

#[test]
fn log_scroll_filter_and_fade() {
    let mut app = headless();
    for i in 0..60 {
        app.gs.feed(&format!(
            "2026.09.07 09:12:{:02} Debug      -  damage has been taken: 1, from source: ",
            i % 60
        ));
    }
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0"
    ));
    app.gs
        .feed(&format!("{P}ownership of Yuki transferred to Alice"));
    let t = app.tick();
    assert!(!t.redraw.log);
    assert!(app.activate(Hit::Log));
    assert!(app.log.visible);
    assert_eq!(app.log.alpha, 0);
    let t = app.tick();
    assert!(t.redraw.log);
    assert_eq!(app.log.rows, 62);
    let max = timeline::max_scroll(62, Renderer::body_h());
    app.log_wheel(-WHEEL_DELTA * 100);
    assert_eq!(app.log.scroll, max);
    app.log_wheel(WHEEL_DELTA / 2);
    assert_eq!(app.log.scroll, max);
    app.log_wheel(WHEEL_DELTA / 2);
    assert_eq!(
        app.log.scroll,
        max - 3.0 * crate::num::to_f32(timeline::ROW_H)
    );
    app.log_press(LogHit::Tab(Filter::Targets), 0);
    assert_eq!(app.log.scroll, 0.0);
    assert_eq!(app.log.rows, 2);
    app.log_press(LogHit::Tab(Filter::All), 0);
    app.log_wheel(-WHEEL_DELTA * 100);
    app.log.scroll_shown = app.log.scroll;
    app.log_press(LogHit::Thumb, 100);
    app.log_drag_to(-5000);
    assert_eq!(app.log.scroll, 0.0);
    app.log_release();
    assert!(app.log.drag.is_none());
    app.gs.log_rotated();
    app.tick();
    assert_eq!(app.log.rows, 0);
    assert_eq!(app.log.scroll, 0.0);
    std::thread::sleep(std::time::Duration::from_millis(220));
    app.tick();
    assert_eq!(app.log.alpha, 255);
    app.log_press(LogHit::Close, 0);
    assert!(!app.log.visible);
}

#[test]
fn update_prompt_is_modal_and_waits_for_boss_fights() {
    let mut app = headless();
    let at = |r: (i32, i32, i32, i32)| (r.0 + r.2 / 2, r.1 + r.3 / 2);
    let (nx, ny) = at(UPDATE_NOW_HIT);
    let (tx, ty) = at(MUTE_HIT);
    assert!(!app.prompt_shown());
    assert_ne!(app.enabled_hit(nx, ny), Some(Hit::UpdateNow));
    app.badge = Badge::Ready("v2026.10.29.0".into());
    assert!(!app.prompt_shown());
    assert!(app.activate(Hit::Update));
    assert!(app.prompt_shown());
    assert_eq!(app.enabled_hit(nx, ny), Some(Hit::UpdateNow));
    let (sx, sy) = at(UPDATE_SKIP_HIT);
    assert_eq!(app.enabled_hit(sx, sy), Some(Hit::UpdateSkip));
    let (lx, ly) = at(UPDATE_LATER_HIT);
    assert_eq!(app.enabled_hit(lx, ly), Some(Hit::UpdateLater));
    assert_eq!(app.enabled_hit(LOGICAL_W - 1, 0), Some(Hit::Close));
    assert_eq!(app.enabled_hit(240, 10), None);
    assert_eq!(app.enabled_hit(210, 10), Some(Hit::Party));
    assert_eq!(app.enabled_hit(300, LOGICAL_H - 20), None);
    for h in [
        Hit::Settings,
        Hit::Mute,
        Hit::RunPrev,
        Hit::Info(Info::Boss),
    ] {
        assert!(!app.hit_enabled(h));
    }
    assert!(app.activate(Hit::UpdateLater));
    assert!(!app.prompt_shown());
    assert_eq!(app.enabled_hit(300, LOGICAL_H - 20), Some(Hit::Update));
    app.activate(Hit::Update);
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1"
    ));
    assert!(!app.prompt_shown());
    assert_ne!(app.enabled_hit(nx, ny), Some(Hit::UpdateNow));
    app.gs
        .feed(&format!("{P}ownership of Kakarot transferred to Alice"));
    assert_eq!(app.enabled_hit(tx, ty), Some(Hit::Mute));
    app.gs
        .feed(&format!("{P}Boss Kakarot dead, personal damage dealt: "));
    assert!(app.prompt_shown());
    app.badge = Badge::Installing("v2026.10.29.0".into());
    assert!(app.prompt_shown());
    assert_eq!(app.enabled_hit(nx, ny), None);
    assert_eq!(app.enabled_hit(sx, sy), None);
    assert_eq!(app.enabled_hit(lx, ly), Some(Hit::UpdateLater));
    assert!(app.activate(Hit::UpdateLater));
    assert!(!app.prompt_shown());
    assert!(app.hit_enabled(Hit::Settings));
    assert_eq!(app.enabled_hit(300, LOGICAL_H - 20), Some(Hit::Update));
    assert!(app.activate(Hit::Update));
    assert!(app.prompt_shown());
    app.badge = Badge::Failed("v2026.10.29.0".into());
    assert_eq!(app.enabled_hit(nx, ny), Some(Hit::UpdateNow));
    assert_eq!(app.enabled_hit(sx, sy), None);
}

#[test]
fn update_restart_waits_for_boss_fights() {
    let mut app = headless();
    assert!(app.can_restart());
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1"
    ));
    assert!(!app.can_restart());
    app.gs
        .feed(&format!("{P}Boss Kakarot dead, personal damage dealt: "));
    assert!(app.can_restart());
}

#[test]
fn usage_question_waits_its_turn_and_saves_the_answer() {
    let at = |r: (i32, i32, i32, i32)| (r.0 + r.2 / 2, r.1 + r.3 / 2);
    let (yx, yy) = at(USAGE_YES_HIT);
    let (nx, ny) = at(USAGE_NO_HIT);
    let mut app = headless();
    assert!(!app.usage_prompt_shown());
    assert_ne!(app.enabled_hit(yx, yy), Some(Hit::UsageYes));
    app.flags.set(Flag::UsageAsk, true);
    assert!(app.usage_prompt_shown());
    assert_eq!(app.enabled_hit(yx, yy), Some(Hit::UsageYes));
    assert_eq!(app.enabled_hit(nx, ny), Some(Hit::UsageNo));
    assert_eq!(app.enabled_hit(LOGICAL_W - 1, 0), Some(Hit::Close));
    assert_eq!(app.enabled_hit(210, 10), Some(Hit::Party));
    assert_eq!(app.enabled_hit(240, 10), None);
    for h in [
        Hit::Settings,
        Hit::Usage,
        Hit::RunPrev,
        Hit::Info(Info::Boss),
    ] {
        assert!(!app.hit_enabled(h));
    }
    app.badge = Badge::Ready("v2026.10.29.0".into());
    app.activate(Hit::Update);
    assert!(app.prompt_shown());
    assert!(!app.usage_prompt_shown());
    assert!(!app.hit_enabled(Hit::UsageYes));
    app.activate(Hit::UpdateLater);
    assert!(app.usage_prompt_shown());
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1"
    ));
    assert!(!app.usage_prompt_shown());
    assert_ne!(app.enabled_hit(yx, yy), Some(Hit::UsageYes));
    app.gs
        .feed(&format!("{P}Boss Kakarot dead, personal damage dealt: "));
    assert!(app.usage_prompt_shown());
    assert!(app.activate(Hit::UsageYes));
    assert!(app.flags.has(Flag::Usage));
    assert!(!app.flags.has(Flag::UsageAsk));
    assert!(!app.usage_prompt_shown());

    let mut app = headless();
    app.flags.set(Flag::UsageAsk, true);
    assert!(app.activate(Hit::UsageNo));
    assert!(!app.flags.has(Flag::Usage));
    assert!(!app.flags.has(Flag::UsageAsk));
    assert!(!crate::usage::ON.load(std::sync::atomic::Ordering::Relaxed));
    assert!(app.hit_enabled(Hit::Settings));
}
