use super::anim::*;
use super::logwin::*;
use super::settings::*;
use super::tick::*;
use super::*;
use crate::hud::timeline;

const P: &str = "2026.09.07 09:12:28 Debug      -  ";
const ENTER: &str = "[Behaviour] Entering Room: Ecliptica - Demo Playtest";

fn headless() -> App {
    let mut app = App::new(96);
    app.watch = LogWatch::default();
    app.gs.feed(&format!("{P}{ENTER}"));
    app.gs.changed = false;
    app
}

#[test]
fn flash_on_target_change() {
    let mut app = headless();
    assert!(app.sound_on);
    let t = app.tick();
    assert!(!t.redraw);
    assert_eq!(t.timer_ms, None);
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1"
    ));
    app.gs
        .feed(&format!("{P}ownership of Kakarot transferred to Alice"));
    let t = app.tick();
    assert!(t.redraw);
    assert_eq!(t.timer_ms, Some(ANIM_MS));
    assert!(app.flash_at.is_some());
    app.gs
        .feed(&format!("{P}Boss Kakarot dead, personal damage dealt: "));
    app.tick();
    assert_eq!(app.dps_boss, None);
}

const STAGE_A: &str =
    "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Blade";
const STAGE_B: &str =
    "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Twinmage";

#[test]
fn settings_steps_and_clamps() {
    let mut app = headless();
    let base = app.renderer.width;
    assert!(!app.hit_enabled(Hit::ScaleUp));
    assert!(app.hit_enabled(Hit::Target) || app.gs.boss.is_none());
    assert!(app.activate(Hit::Settings));
    assert!(app.settings_open);
    assert!(!app.hit_enabled(Hit::Target));
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
    assert!(!app.settings_open);
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
    assert!(app.log_rows() == 0);
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
    app.last_ts_at = Instant::now() - std::time::Duration::from_secs(600);
    assert_eq!(app.now(), ts + 600);
    app.last_ts_at = Instant::now() - std::time::Duration::from_secs(10);
    assert_eq!(app.now(), ts + 10);
    app.gs
        .feed(&format!("{P}[Behaviour] Entering Room: Sky Dream"));
    assert_eq!(app.env(), Env::NotInWorld);
    assert_eq!(app.now(), ts);
    assert!(app.gs.runs[0].end_ts.is_some());
    assert_eq!(app.pages(), 2);
    assert!(app.log_rows() == 0);
    app.vrc_running = false;
    assert_eq!(app.env(), Env::NoVrchat);
    let t = app.tick();
    assert!(t.redraw);
    assert!(!app.tick().redraw);
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
    backfill(&mut gs, &dir);
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
    assert!(app
        .viewed_run()
        .is_some_and(|(i, r)| i == 0 && r.fights.len() == 3));
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
    assert_eq!((gi, g.len()), (1, 1));
    assert!(app.group_prev());
    let (gi, g) = app.viewed_group().unwrap();
    assert_eq!((gi, g.len()), (0, 2));
    assert!(!app.group_prev());
    assert!(app.group_next());
    assert_eq!(app.sel_group, None);
    assert!(app.run_next());
    assert!(app.is_live());
    assert!(!app.hit_enabled(Hit::Target));
    assert!(app.hit_enabled(Hit::Log));
    assert!(app.topmost);
    assert!(app.activate(Hit::Pin));
    assert!(!app.topmost);
    assert!(!app.discord_on);
    assert!(app.hit_enabled(Hit::Discord));
    assert!(app.activate(Hit::Discord));
    assert!(app.discord_on && app.presence.is_some());
    assert!(app.activate(Hit::Discord));
    assert!(!app.discord_on);
    assert!(app.hit_enabled(Hit::Info(Info::LastHit)));
    assert!(!app.hit_enabled(Hit::Info(Info::Result)));
    assert!(app.run_prev());
    assert!(!app.hit_enabled(Hit::Info(Info::LastHit)));
    assert!(app.hit_enabled(Hit::Info(Info::Result)));
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
    assert!(app
        .viewed_group()
        .is_some_and(|(i, g)| i == 0 && g.len() == 2));
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
    assert!(app
        .viewed_group()
        .is_some_and(|(i, g)| i == 0 && g.len() == 2));
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
    let feed = |app: &mut App, msg: &str| app.gs.feed(&format!("{P}{msg}"));
    let spawn_stage = |app: &mut App, stage: &str| {
        for _ in 0..3 {
            feed(app, "spawn token, False, 0");
        }
        feed(app, stage);
    };
    spawn_stage(&mut app, STAGE_B);
    feed(&mut app, "Initializing Enemy POOL ID0 as ENEMY ID 1");
    feed(&mut app, "Retiring Enemy POOL ID0");
    app.tick();
    assert!(app.warn_at.is_none());
    spawn_stage(&mut app, STAGE_A);
    feed(&mut app, "ECLIPTICA saving SESSION ID 2505");
    feed(&mut app, "Initializing Enemy POOL ID0 as ENEMY ID 1");
    app.tick();
    assert!(app.warn_at.is_none());
    feed(&mut app, "Retiring Enemy POOL ID0");
    let tick = app.tick();
    assert!(app.warn_at.is_some());
    assert_eq!(tick.timer_ms, Some(ANIM_MS));
    app.warn_at = None;
    app.tick();
    assert!(app.warn_at.is_none());
    feed(&mut app, "Initializing Enemy POOL ID3 as ENEMY ID 1");
    feed(&mut app, "Retiring Enemy POOL ID3");
    app.tick();
    assert!(app.warn_at.is_some());
    app.warn_at = None;
    feed(&mut app, "Initializing Enemy POOL ID3 as ENEMY ID 1");
    for _ in 0..2 {
        feed(&mut app, "ECLIPTICA saving SESSION ID 2505");
    }
    feed(&mut app, "Retiring Enemy POOL ID3");
    app.tick();
    assert!(app.warn_at.is_none());
}

#[test]
fn dps_peak_survives_phase_change() {
    let mut app = headless();
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0"
    ));
    app.gs.feed(&format!("{P}Dealing 100 STRIKE damage"));
    app.gs
        .feed(&format!("{P}damage has been taken: 40, from source: "));
    app.tick();
    let peak = app.dps_peak;
    assert!(peak > 0);
    assert_eq!(app.taken_peak, 40);
    assert_eq!(app.dps_boss.as_deref(), Some("Yuki"));
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: YukiPhase2(Clone) on phase: 0"
    ));
    app.tick();
    assert_eq!(app.dps_peak, peak);
    assert_eq!(app.taken_peak, 40);
    assert_eq!(app.dps_boss.as_deref(), Some("Yuki"));
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0"
    ));
    app.tick();
    assert_eq!(app.dps_boss.as_deref(), Some("Kakarot"));
}

#[test]
fn timer_settles_after_animation() {
    let mut app = headless();
    app.gs.feed(&format!(
        "{P}ECLIPTICA - now in stage: Stage_Test on phase: 0.5 as class: Blade"
    ));
    let t = app.tick();
    assert!(t.redraw);
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
    assert!(settled(&mut app).redraw);
    app.gs.feed(&format!("{P}ECLIPTICA - now in intermission"));
    assert!(settled(&mut app).redraw);
    app.gs.feed(&format!("{P}ECLIPTICA - now in lobby"));
    assert!(!settled(&mut app).redraw);
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
    assert!(t.redraw);
    assert_eq!(t.timer_ms, Some(1000));
    assert!(!app.tick().redraw);
    assert!(app.set_hover(None));
    assert_eq!(app.tip(), None);
    app.log_open();
    assert!(app.log_set_hover(Some(LogHit::Close)));
    assert_eq!(app.log_tip(), None);
    app.tick();
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(app.tick().redraw_log);
    assert_eq!(app.log_tip(), Some(LogHit::Close));
    assert!(!app.tick().redraw_log);
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
    assert!(!t.redraw_log);
    assert!(app.activate(Hit::Log));
    assert!(app.log.visible);
    assert_eq!(app.log.alpha, 0);
    let t = app.tick();
    assert!(t.redraw_log);
    assert_eq!(app.log.rows, 62);
    let max = timeline::max_scroll(62, app.log.renderer.body_h());
    app.log_wheel(-WHEEL_DELTA * 100);
    assert_eq!(app.log.scroll, max);
    app.log_wheel(WHEEL_DELTA / 2);
    assert_eq!(app.log.scroll, max);
    app.log_wheel(WHEEL_DELTA / 2);
    assert_eq!(app.log.scroll, max - 3.0 * timeline::ROW_H as f32);
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
