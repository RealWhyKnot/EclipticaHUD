use super::*;

#[test]
fn flash_on_target_change() {
    let mut app = headless();
    assert!(app.flags.has(Flag::Sound));
    let t = app.tick();
    assert!(!t.redraw.main);
    assert_eq!(t.timer_ms, None);
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1"
    ));
    app.gs
        .feed(&format!("{P}ownership of Kakarot transferred to Alice"));
    let t = app.tick();
    assert!(t.redraw.main);
    assert_eq!(t.timer_ms, Some(ANIM_MS));
    assert!(app.flash_at.is_some());
    app.gs
        .feed(&format!("{P}Boss Kakarot dead, personal damage dealt: "));
    app.tick();
}

#[test]
fn scale_fits_the_screen() {
    assert_eq!(fit_scale(96, (LOGICAL_W, LOGICAL_H), (1920, 1040)), 1.6);
    assert_eq!(
        fit_scale(96, (LOGICAL_W, LOGICAL_H), (3440, 1400)),
        MAX_SCALE
    );
    assert_eq!(fit_scale(144, (LOGICAL_W, LOGICAL_H), (1920, 1040)), 1.0);
    assert_eq!(fit_scale(96, (LOGICAL_W, LOGICAL_H), (200, 200)), MIN_SCALE);
    for dpi in [96, 120, 144, 168, 192] {
        for work in [(1280, 680), (1920, 1040), (2560, 1400), (3440, 1400)] {
            let s = fit_scale(dpi, (LOGICAL_W, LOGICAL_H), work);
            let r = Renderer::new(
                crate::num::to_u32((crate::num::to_f32(dpi) * s).round()),
                LOGICAL_W,
                LOGICAL_H,
            );
            assert!(s <= MIN_SCALE || r.height <= work.1, "{dpi} {work:?} {s}");
            if s < MAX_SCALE {
                let up = ((s + 0.1) * 10.0).round() / 10.0;
                let r = Renderer::new(
                    crate::num::to_u32((crate::num::to_f32(dpi) * up).round()),
                    LOGICAL_W,
                    LOGICAL_H,
                );
                assert!(r.height > work.1 || r.width > work.0, "{dpi} {work:?} {up}");
            }
        }
    }
}

#[test]
fn scale_stops_where_the_screen_ends() {
    let mut app = headless();
    let base = app.renderer.height;
    app.activate(Hit::Settings);
    app.set_scale(MAX_SCALE);
    assert!(app.set_scale_cap(1.6));
    assert_eq!(app.scale, MAX_SCALE);
    assert!((app.shown_scale() - 1.6).abs() < 0.001);
    assert_eq!(
        app.renderer.height,
        Renderer::new(154, LOGICAL_W, LOGICAL_H).height
    );
    assert!(!app.hit_enabled(Hit::ScaleUp));
    assert!(!app.activate(Hit::ScaleUp));
    assert!(app.activate(Hit::ScaleDown));
    assert!((app.scale - 1.5).abs() < 0.001);
    assert!(app.activate(Hit::ScaleUp));
    assert!((app.scale - 1.6).abs() < 0.001);
    assert!(!app.set_scale_cap(1.8));
    app.set_scale(MAX_SCALE);
    assert!(app.set_scale_cap(MAX_SCALE));
    assert_eq!(app.renderer.height, base * 2);
    assert!(!app.set_scale_cap(MAX_SCALE));
}

#[test]
fn each_window_keeps_its_own_dpi() {
    let mut app = headless();
    app.log.dpi = 144;
    app.rescale();
    assert_eq!(app.renderer.width, LOGICAL_W);
    assert_eq!(app.log.renderer.width, LOG_W * 3 / 2);
    assert_eq!(app.party_win.renderer.width, PARTY_W);
    app.set_scale(2.0);
    assert_eq!(app.log.renderer.width, LOG_W * 3);
    assert_eq!(app.party_win.renderer.width, PARTY_W * 2);
}

#[test]
fn settings_steps_and_clamps() {
    let mut app = headless();
    let base = app.renderer.width;
    assert!(!app.hit_enabled(Hit::ScaleUp));
    assert!(app.hit_enabled(Hit::Mute) || app.gs.boss.is_none());
    assert!(app.activate(Hit::Settings));
    assert!(app.flags.has(Flag::SettingsOpen));
    assert!(!app.hit_enabled(Hit::Mute));
    assert!(!app.hit_enabled(Hit::Info(Info::Boss)));
    assert!(!app.hit_enabled(Hit::AlphaUp));
    assert!(app.hit_enabled(Hit::AlphaDown));
    assert!(app.activate(Hit::ScaleUp));
    assert!((app.scale - 1.1).abs() < 0.001);
    assert!(app.renderer.width > base);
    assert!(app.log.renderer.width > LOG_W);
    for _ in 0..20 {
        app.activate(Hit::ScaleUp);
    }
    assert_eq!(app.scale, MAX_SCALE);
    assert!(!app.hit_enabled(Hit::ScaleUp));
    assert!(!app.activate(Hit::ScaleUp));
    assert_eq!(app.renderer.width, base * 2);
    for _ in 0..20 {
        app.activate(Hit::ScaleDown);
    }
    assert_eq!(app.scale, MIN_SCALE);
    assert_eq!(app.renderer.width, base / 2);
    assert!(!app.hit_enabled(Hit::ScaleDown));
    for _ in 0..10 {
        app.activate(Hit::AlphaDown);
    }
    assert_eq!(app.alpha, MIN_ALPHA);
    assert!(!app.activate(Hit::AlphaDown));
    assert!(app.activate(Hit::AlphaUp));
    assert_eq!(app.alpha, MIN_ALPHA + ALPHA_STEP);
    assert_eq!(app.alpha_byte(), 102);
    assert_eq!(app.gs.win(), 10);
    assert!(app.hit_enabled(Hit::WindowUp) && app.hit_enabled(Hit::WindowDown));
    assert!(app.activate(Hit::WindowUp));
    assert_eq!(app.gs.win(), 15);
    for _ in 0..10 {
        app.activate(Hit::WindowUp);
    }
    assert_eq!(app.gs.win(), 30);
    assert!(!app.hit_enabled(Hit::WindowUp));
    assert!(!app.activate(Hit::WindowUp));
    for _ in 0..20 {
        app.activate(Hit::WindowDown);
    }
    assert_eq!(app.gs.win(), 3);
    assert!(!app.activate(Hit::WindowDown));
    app.set_window(12);
    assert_eq!(app.gs.win(), 12);
    assert!(app.activate(Hit::WindowDown));
    assert_eq!(app.gs.win(), 10);
    app.set_window(99);
    assert_eq!(app.gs.win(), 30);
    assert!(!app.hit_enabled(Hit::Info(Info::LastKill)));
    app.set_scale(9.0);
    assert_eq!(app.scale, MAX_SCALE);
    app.set_alpha(0);
    assert_eq!(app.alpha, MIN_ALPHA);
    assert!(app.activate(Hit::Settings));
    assert!(!app.flags.has(Flag::SettingsOpen));
    assert!(!app.hit_enabled(Hit::ScaleDown));
}

#[test]
fn empty_state_has_one_page() {
    let mut app = headless();
    assert_eq!(app.pages(), 1);
    assert_eq!(app.viewed_page(), 0);
    assert!(app.viewed_run().is_none());
    assert!(!app.hit_enabled(Hit::RunPrev));
    assert!(!app.run_prev());
    assert_eq!(app.log_rows(), 0);
    app.gs.feed(&format!("{P}{STAGE_A}"));
    app.gs.feed(&format!("{P}ECLIPTICA - now in lobby"));
    app.gs.feed(&format!("{P}{STAGE_B}"));
    assert_eq!(app.pages(), 2);
    assert!(app.run_prev());
    app.gs.feed(&format!("{P}ECLIPTICA - now in lobby"));
    assert_eq!(app.sel_run, Some(0));
    assert_eq!(app.pages(), 3);
    assert!(app.run_next());
    assert_eq!(app.sel_run, Some(1));
    assert!(app.run_next());
    assert_eq!(app.sel_run, None);
}

#[test]
fn env_and_clock() {
    let mut app = headless();
    assert_eq!(app.env(), Env::InWorld);
    app.gs.feed(&format!("{P}{STAGE_A}"));
    app.tick();
    let ts = app.last_ts;
    assert_eq!(app.now(), ts);
    app.last_ts_at = Instant::now()
        .checked_sub(std::time::Duration::from_secs(600))
        .unwrap();
    assert_eq!(app.now(), ts + 600);
    app.last_ts_at = Instant::now()
        .checked_sub(std::time::Duration::from_secs(10))
        .unwrap();
    assert_eq!(app.now(), ts + 10);
    app.gs
        .feed(&format!("{P}[Behaviour] Entering Room: Sky Dream"));
    assert_eq!(app.env(), Env::NotInWorld);
    assert_eq!(app.now(), ts);
    assert!(app.gs.runs[0].end_ts.is_some());
    assert_eq!(app.pages(), 2);
    assert_eq!(app.log_rows(), 0);
    app.flags.set(Flag::VrcRunning, false);
    assert_eq!(app.env(), Env::NoVrchat);
    let t = app.tick();
    assert!(t.redraw.main);
    assert!(!app.tick().redraw.main);
}

#[test]
fn backfill_reads_all_but_newest() {
    let dir = std::env::temp_dir().join(format!("ehud_backfill_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log_a = dir.join("output_log_2026-01-01_00-00-00.txt");
    std::fs::write(
        &log_a,
        format!("2026.01.01 10:00:00 Debug      -  {STAGE_A}\r\n"),
    )
    .unwrap();
    let log_b = dir.join("output_log_2026-01-02_00-00-00.txt");
    std::fs::write(
        &log_b,
        format!("2026.01.02 10:00:00 Debug      -  {STAGE_B}\n"),
    )
    .unwrap();
    let future = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
    std::fs::File::options()
        .write(true)
        .open(&log_b)
        .unwrap()
        .set_modified(future)
        .unwrap();
    let mut gs = GameState::default();
    archive::backfill(&mut gs, Some(&dir), None);
    assert_eq!(gs.runs.len(), 1);
    assert!(gs.runs[0].end_ts.is_some());
    assert_eq!(gs.runs[0].class, "Blade");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn rotation_in_tick_closes_run() {
    let dir = std::env::temp_dir().join(format!("ehud_rotate_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log_a = dir.join("output_log_2026-01-01_00-00-00.txt");
    std::fs::write(
        &log_a,
        format!("2026.01.01 10:00:00 Debug      -  {STAGE_A}\n"),
    )
    .unwrap();
    let mut app = App::new(96);
    app.watch = LogWatch::for_dir(dir.clone());
    app.tick();
    assert_eq!(app.gs.runs.len(), 1);
    assert!(app.gs.runs[0].end_ts.is_none());
    let log_b = dir.join("output_log_2026-01-02_00-00-00.txt");
    std::fs::write(
        &log_b,
        format!("2026.01.02 10:00:00 Debug      -  {STAGE_B}\n"),
    )
    .unwrap();
    let future = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
    std::fs::File::options()
        .write(true)
        .open(&log_b)
        .unwrap()
        .set_modified(future)
        .unwrap();
    app.tick();
    assert_eq!(app.gs.runs.len(), 1);
    assert!(app.gs.runs[0].end_ts.is_some());
    app.tick();
    assert_eq!(app.gs.runs.len(), 2);
    assert_eq!(app.gs.runs[1].class, "Twinmage");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn closing_a_run_truncates_the_wal() {
    let dir = std::env::temp_dir().join(format!("ehud_wal_close_{}", std::process::id()));
    let logs = dir.join("logs");
    std::fs::create_dir_all(&logs).unwrap();
    let wal = dir.join("history.db-wal");
    let wal_len = || std::fs::metadata(&wal).map_or(0, |m| m.len());
    let mut app = App::new(96);
    app.set_history(crate::history::History::open(&dir.join("history.db")));
    app.watch = LogWatch::for_dir(logs.clone());
    let log_a = logs.join("output_log_2026-01-01_00-00-00.txt");
    std::fs::write(
        &log_a,
        format!("2026.01.01 10:00:00 Debug      -  {STAGE_A}\n"),
    )
    .unwrap();
    app.tick();
    assert!(app.gs.live_run().is_some());
    assert!(wal_len() > 0);
    std::fs::OpenOptions::new()
        .append(true)
        .open(&log_a)
        .unwrap()
        .write_all(format!("2026.01.01 10:00:05 Debug      -  {STAGE_A}\n").as_bytes())
        .unwrap();
    app.tick();
    assert!(app.gs.live_run().is_some());
    assert!(wal_len() > 0);
    let log_b = logs.join("output_log_2026-01-02_00-00-00.txt");
    std::fs::write(&log_b, "").unwrap();
    app.tick();
    assert!(app.gs.runs[0].end_ts.is_some());
    assert_eq!(wal_len(), 0);
    drop(app);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn navigation() {
    let mut app = headless();
    app.gs.feed(&format!("{P}{STAGE_A}"));
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0"
    ));
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: YukiPhase2(Clone) on phase: 0"
    ));
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0"
    ));
    app.gs.feed(&format!("{P}ECLIPTICA - now in lobby"));
    assert_eq!(app.pages(), 2);
    assert_eq!(app.viewed_page(), 1);
    assert!(app.viewed_run().is_none());
    assert!(app.hit_enabled(Hit::RunPrev));
    assert!(!app.hit_enabled(Hit::RunNext));
    assert!(app.run_prev());
    assert_eq!(app.sel_run, Some(0));
    assert!(
        app.viewed_run()
            .is_some_and(|(i, r)| i == 0 && r.fights.len() == 3)
    );
    assert!(app.run_next());
    assert!(app.is_live() && app.sel_run.is_none());
    assert!(app.viewed_run().is_none());
    app.gs.feed(&format!("{P}{STAGE_B}"));
    assert_eq!(app.gs.runs.len(), 2);
    assert_eq!(app.pages(), 2);
    assert!(app.viewed_run().is_some_and(|(i, _)| i == 1));
    assert!(app.is_live());
    assert!(!app.run_next());
    assert!(!app.group_next());
    assert!(!app.hit_enabled(Hit::RunNext));
    assert!(app.hit_enabled(Hit::RunPrev));
    assert!(app.run_prev());
    assert_eq!(app.sel_run, Some(0));
    let (gi, g) = app.viewed_group().unwrap();
    assert_eq!((gi, g), (1, Page::Boss(2..3)));
    assert!(app.group_prev());
    let (gi, g) = app.viewed_group().unwrap();
    assert_eq!((gi, g), (0, Page::Boss(0..2)));
    assert!(!app.group_prev());
    assert!(app.group_next());
    assert_eq!(app.sel_group, None);
    assert!(app.run_next());
    assert!(app.is_live());
    assert!(!app.hit_enabled(Hit::Mute));
    assert!(app.hit_enabled(Hit::Log));
    assert!(app.flags.has(Flag::Topmost));
    assert!(app.activate(Hit::Pin));
    assert!(!app.flags.has(Flag::Topmost));
    assert!(!app.flags.has(Flag::Discord));
    assert!(!app.hit_enabled(Hit::Discord));
    app.flags.set(Flag::SettingsOpen, true);
    assert!(app.hit_enabled(Hit::Discord));
    assert!(app.activate(Hit::Discord));
    assert!(app.flags.has(Flag::Discord) && app.presence.is_some());
    assert!(app.activate(Hit::Discord));
    assert!(!app.flags.has(Flag::Discord));
    app.flags.set(Flag::SettingsOpen, false);
    assert!(app.hit_enabled(Hit::Info(Info::LastHit)));
    assert!(!app.hit_enabled(Hit::Info(Info::Result)));
    assert!(app.run_prev());
    assert!(!app.hit_enabled(Hit::Info(Info::LastHit)));
    assert!(app.hit_enabled(Hit::Info(Info::Result)));
}
