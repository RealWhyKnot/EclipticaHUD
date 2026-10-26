use super::anim::*;
use super::logwin::*;
use super::settings::*;
use super::tick::*;
use super::*;
use crate::game::run::{BossFight, Page, Run, RunEnd};
use crate::hud::render::{
    BOARD_DELETE_HIT, BOARD_HIT, BODY_DY, MUTE_HIT, SPLIT_ROWS, UPDATE_LATER_HIT, UPDATE_NOW_HIT,
    UPDATE_SKIP_HIT, USAGE_NO_HIT, USAGE_YES_HIT,
};
use crate::hud::timeline;
use crate::relay::proto::Totals;
use std::io::Write as _;

const P: &str = "2026.09.07 09:12:28 Debug      -  ";
const ENTER: &str = "[Behaviour] Entering Room: Ecliptica - Demo Playtest";

#[derive(Default)]
struct FakeRelay {
    calls: std::sync::Mutex<Vec<(Option<crate::relay::Want>, Option<Totals>)>>,
}

impl crate::relay::Relay for FakeRelay {
    fn sync(&'static self, want: Option<crate::relay::Want>, dmg: Option<Totals>) {
        self.calls.lock().unwrap().push((want, dmg));
    }

    fn poll(&self) -> crate::relay::Poll {
        crate::relay::Poll {
            link: crate::relay::Link::Off,
            session: 0,
            snaps: Vec::new(),
        }
    }
}

impl FakeRelay {
    fn take(&self) -> Vec<(Option<crate::relay::Want>, Option<Totals>)> {
        std::mem::take(&mut *self.calls.lock().unwrap())
    }
}

fn fake_relay(app: &mut App) -> &'static FakeRelay {
    let fake: &'static FakeRelay = Box::leak(Box::default());
    app.party.relay = fake;
    fake
}

fn headless() -> App {
    let mut app = App::new(96);
    fake_relay(&mut app);
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
}

const STAGE_A: &str =
    "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Blade";
const STAGE_B: &str =
    "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Twinmage";

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
            let r = Renderer::new((dpi as f32 * s).round() as u32, LOGICAL_W, LOGICAL_H);
            assert!(s <= MIN_SCALE || r.height <= work.1, "{dpi} {work:?} {s}");
            if s < MAX_SCALE {
                let up = ((s + 0.1) * 10.0).round() / 10.0;
                let r = Renderer::new((dpi as f32 * up).round() as u32, LOGICAL_W, LOGICAL_H);
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
    assert!(app.settings_open);
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
    assert!(app.topmost);
    assert!(app.activate(Hit::Pin));
    assert!(!app.topmost);
    assert!(!app.discord_on);
    assert!(!app.hit_enabled(Hit::Discord));
    app.settings_open = true;
    assert!(app.hit_enabled(Hit::Discord));
    assert!(app.activate(Hit::Discord));
    assert!(app.discord_on && app.presence.is_some());
    assert!(app.activate(Hit::Discord));
    assert!(!app.discord_on);
    app.settings_open = false;
    assert!(app.hit_enabled(Hit::Info(Info::LastHit)));
    assert!(!app.hit_enabled(Hit::Info(Info::Result)));
    assert!(app.run_prev());
    assert!(!app.hit_enabled(Hit::Info(Info::LastHit)));
    assert!(app.hit_enabled(Hit::Info(Info::Result)));
}

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
    app.settings_open = true;
    assert!(!app.hit_enabled(Hit::Stages) && !app.hit_enabled(Hit::Splits));
    app.settings_open = false;
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
    assert!(app
        .viewed_group()
        .is_some_and(|(i, g)| i == 0 && g == Page::Boss(0..2)));
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
        .is_some_and(|(i, g)| i == 0 && g == Page::Boss(0..2)));
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
    app.usage_ask = true;
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
    assert!(app.usage_on);
    assert!(!app.usage_ask);
    assert!(!app.usage_prompt_shown());

    let mut app = headless();
    app.usage_ask = true;
    assert!(app.activate(Hit::UsageNo));
    assert!(!app.usage_on);
    assert!(!app.usage_ask);
    assert!(!crate::usage::ON.load(std::sync::atomic::Ordering::Relaxed));
    assert!(app.hit_enabled(Hit::Settings));
}

mod party_tests {
    use super::*;
    use crate::relay::proto::{member_hash, room_hash, Totals};
    use crate::relay::{self, Link, Poll, Snap, Want};
    use std::time::{Duration, Instant};

    const W1: &str = "[Behaviour] Joining wrld_0fb88df3-2057-4c2f-8e06-e948864378fd:98473~hidden(usr_me)~region(use)";
    const W2: &str = "[Behaviour] Joining wrld_0fb88df3-2057-4c2f-8e06-e948864378fd:11111~hidden(usr_me)~region(use)";

    fn feed(app: &mut App, ts: &str, line: &str) {
        app.gs
            .feed(&format!("2026.09.07 {ts} Debug      -  {line}"));
    }

    fn party_app() -> (App, Want) {
        let mut app = headless();
        feed(&mut app, "09:12:29", "User Authenticated: Me (usr_me)");
        feed(&mut app, "09:12:30", W1);
        feed(
            &mut app,
            "09:12:31",
            "[Behaviour] OnPlayerJoined Me (usr_me)",
        );
        feed(
            &mut app,
            "09:12:31",
            "[Behaviour] OnPlayerJoined Alice (usr_a)",
        );
        app.set_party(true);
        let want = relay::want(
            true,
            true,
            app.gs.location.as_deref(),
            app.gs.me_usr.as_deref(),
        )
        .unwrap();
        (app, want)
    }

    fn poll(link: Link, session: u64, snaps: Vec<Snap>) -> Option<Poll> {
        Some(Poll {
            link,
            session,
            snaps,
        })
    }

    fn snap(at: Instant, want: &Want, entries: &[(&str, u64)]) -> Snap {
        Snap {
            at,
            room: want.room(),
            entries: entries
                .iter()
                .map(|(usr, d)| (member_hash(&want.location, usr), Totals::from((*d, 0))))
                .collect(),
        }
    }

    fn rows(app: &App) -> Vec<(String, bool, bool, u64)> {
        app.party
            .pane()
            .rows
            .iter()
            .map(|r| (r.name.clone(), r.you, r.live, r.dmg))
            .collect()
    }

    fn row(name: &str, you: bool, live: bool, dmg: u64) -> (String, bool, bool, u64) {
        (name.to_string(), you, live, dmg)
    }

    #[test]
    fn party_button_opens_a_window_and_the_window_holds_the_toggle() {
        let mut app = headless();
        assert!(!app.party_on);
        assert!(app.hit_enabled(Hit::Party));
        assert!(app.activate(Hit::Party));
        assert!(app.party_win.visible);
        assert!(!app.party_on);
        assert_eq!(app.party_win.alpha, 0);
        app.render_party();
        assert!(app.party_activate(PartyHit::Toggle));
        assert!(app.party_on);
        assert!(!app.party_activate(PartyHit::Board));
        assert!(app.activate(Hit::Party));
        assert!(!app.party_win.visible);
        assert!(app.party_on);
        app.party_open();
        app.party_set_hover(Some(PartyHit::Close));
        assert!(app.party_activate(PartyHit::Close));
        assert!(!app.party_win.visible);
        assert_eq!(app.party_win.hover, None);
        app.activate(Hit::Settings);
        assert!(app.hit_enabled(Hit::Party));
        assert!(app.hit_enabled(Hit::Sound));
        assert!(!app.hit_enabled(Hit::Mute));
        let (x, y, w, h) = crate::hud::render::PARTY_CLOSE_HIT;
        assert_eq!(app.party_hit(x + w / 2, y + h / 2), Some(PartyHit::Close));
        assert_eq!(app.party_hit(4, 4), None);
    }

    #[test]
    fn finished_shared_fights_are_saved_and_paged() {
        let (mut app, want) = party_app();
        app.set_history(crate::history::History::in_memory());
        let count = |app: &App| app.archive.db.as_ref().unwrap().party_count(0);
        feed(
            &mut app,
            "09:12:50",
            "ECLIPTICA - now in stage: Stage_Forest on phase: 0.1 as class: Blade",
        );
        let t0 = Instant::now();
        let step = |app: &mut App, entries: &[(&str, u64)]| {
            let s = snap(t0, &want, entries);
            app.party_step(t0, Some(&want), poll(Link::Connected, 1, vec![s]));
        };
        feed(
            &mut app,
            "09:13:00",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
        );
        step(&mut app, &[("usr_a", 300), ("usr_me", 100)]);
        assert_eq!(count(&app), 0);
        feed(
            &mut app,
            "09:14:00",
            "Boss Kakarot dead, personal damage dealt: ",
        );
        step(&mut app, &[("usr_a", 320), ("usr_me", 100)]);
        assert_eq!(count(&app), 1);
        step(&mut app, &[("usr_a", 340), ("usr_me", 100)]);
        assert_eq!(count(&app), 1);
        let rec = app.archive.db.as_ref().unwrap().party_page(0, 0).unwrap();
        assert_eq!(rec.boss, "Kakarot");
        assert!(rec.down);
        assert!((rec.progress - 0.1).abs() < 1e-6);
        assert_eq!(rec.rows[0].dealt, 340);
        assert_eq!(app.party.pane().pages, 0);
        feed(
            &mut app,
            "09:20:00",
            "ECLIPTICA - now fighting boss: DarkMouth(Clone) on phase: 0.2",
        );
        step(&mut app, &[("usr_me", 50)]);
        assert_eq!(app.party.pane().pages, 1);
        assert_eq!(app.party.pane().page_label(), "2/2");
        assert!(app.party_activate(PartyHit::Older));
        let pane = app.party.pane();
        assert_eq!(pane.saved, Some(rec.at));
        assert_eq!(pane.board_head(), "SAVED FIGHT");
        assert_eq!(pane.page_label(), "1/2");
        assert_eq!(pane.rows[0].name, "Alice");
        assert_eq!(pane.rows[1].name, "Me");
        assert!(pane.rows[1].you);
        assert!(!app.party_activate(PartyHit::Older));
        step(&mut app, &[("usr_me", 60)]);
        assert_eq!(app.party.pane().saved, Some(rec.at));
        assert!(app.party_activate(PartyHit::Newer));
        assert_eq!(app.party.pane().saved, None);
        assert_eq!(app.party.pane().rows.len(), 1);
        assert!(app.party.pane().fight.as_ref().is_some_and(|f| f.live));
        app.party_activate(PartyHit::Older);
        feed(
            &mut app,
            "09:21:00",
            "Boss DarkMouth dead, personal damage dealt: ",
        );
        step(&mut app, &[("usr_me", 60)]);
        assert_eq!(count(&app), 1);
        feed(
            &mut app,
            "09:25:00",
            "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0.3",
        );
        step(&mut app, &[("usr_me", 5)]);
        assert_eq!(app.party.pane().saved, None);
        assert_eq!(app.party.pane().page, 0);
        let (x, y, w, h) = crate::hud::render::PARTY_OLDER;
        assert_eq!(app.party_hit(x + w / 2, y + h / 2), Some(PartyHit::Older));
    }

    #[test]
    fn a_fight_cut_short_by_leaving_is_still_saved() {
        let (mut app, want) = party_app();
        app.set_history(crate::history::History::in_memory());
        feed(
            &mut app,
            "09:13:00",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
        );
        let t0 = Instant::now();
        let s = snap(t0, &want, &[("usr_a", 300), ("usr_me", 100)]);
        app.party_step(t0, Some(&want), poll(Link::Connected, 1, vec![s]));
        feed(&mut app, "09:13:30", "[Behaviour] OnLeftRoom");
        feed(&mut app, "09:13:40", W2);
        app.party_step(t0, None, poll(Link::Connected, 1, Vec::new()));
        let db = app.archive.db.as_ref().unwrap();
        assert_eq!(db.party_count(0), 1);
        let rec = db.party_page(0, 0).unwrap();
        assert_eq!(rec.rows.len(), 2);
        assert_eq!(rec.rows[0].dealt, 300);
    }

    #[test]
    fn a_wiped_fight_is_saved_as_not_down() {
        let (mut app, want) = party_app();
        app.set_history(crate::history::History::in_memory());
        feed(
            &mut app,
            "09:12:50",
            "ECLIPTICA - now in stage: Stage_Forest on phase: 0.7 as class: Blade",
        );
        feed(
            &mut app,
            "09:13:00",
            "ECLIPTICA - now fighting boss: Pandora(Clone) on phase: 0.76",
        );
        let t0 = Instant::now();
        let s = snap(t0, &want, &[("usr_a", 900), ("usr_me", 400)]);
        app.party_step(t0, Some(&want), poll(Link::Connected, 1, vec![s]));
        assert_eq!(app.archive.db.as_ref().unwrap().party_count(0), 0);
        feed(
            &mut app,
            "09:19:20",
            "Boss Pandora dead, personal damage dealt: ",
        );
        feed(&mut app, "09:19:20", "ECLIPTICA - now in lobby");
        app.party_step(t0, Some(&want), poll(Link::Connected, 1, Vec::new()));
        let db = app.archive.db.as_ref().unwrap();
        assert_eq!(db.party_count(0), 1);
        let rec = db.party_page(0, 0).unwrap();
        assert_eq!(rec.boss, "Pandora");
        assert!(!rec.down);
        assert_eq!(rec.secs, 380);
        assert_eq!(rec.rows[0].dealt, 900);
        app.party_step(t0, Some(&want), poll(Link::Connected, 1, Vec::new()));
        assert_eq!(app.archive.db.as_ref().unwrap().party_count(0), 1);
    }

    #[test]
    fn snaps_fill_the_table_with_roster_names() {
        let (mut app, want) = party_app();
        feed(
            &mut app,
            "09:13:00",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
        );
        feed(&mut app, "09:13:01", "Dealing 100 STRIKE damage");
        let t0 = Instant::now();
        let s = snap(
            t0,
            &want,
            &[("usr_a", 300), ("usr_b", 100), ("usr_me", 100)],
        );
        assert!(app.party_step(t0, Some(&want), poll(Link::Connected, 1, vec![s])));
        assert_eq!(
            rows(&app),
            [row("Alice", false, true, 300), row("Me", true, true, 100)]
        );
        assert_eq!(app.party.pane().status().0, "connected");
        assert!(app.party.pane().ready);
        assert_eq!(app.party.pane().here, 2);
        assert!(app.party.pane().fight.as_ref().is_some_and(|f| f.live));
        assert!(!app.party_step(t0, Some(&want), poll(Link::Connected, 1, Vec::new())));
        let wrong = Snap {
            room: room_hash("wrld_other:1"),
            ..snap(t0, &want, &[("usr_a", 999)])
        };
        app.party_step(t0, Some(&want), poll(Link::Connected, 1, vec![wrong]));
        assert_eq!(rows(&app)[0], row("Alice", false, true, 300));
        app.party_step(
            t0,
            None,
            poll(Link::Connected, 1, vec![snap(t0, &want, &[("usr_a", 999)])]),
        );
        assert_eq!(rows(&app)[0], row("Alice", false, true, 300));
        let t1 = t0 + Duration::from_secs(1);
        let s = snap(t1, &want, &[("usr_a", 500), ("usr_b", 100)]);
        app.party_step(t1, Some(&want), poll(Link::Connected, 1, vec![s]));
        assert_eq!(rows(&app)[0], row("Alice", false, true, 500));
        assert_eq!(app.party.pane().rows[0].dps, 200);
        assert_eq!(rows(&app).len(), 2);
        feed(
            &mut app,
            "09:13:02",
            "[Behaviour] OnPlayerJoined Bob (usr_b)",
        );
        app.party_step(t1, Some(&want), poll(Link::Connected, 1, Vec::new()));
        assert_eq!(rows(&app)[2], row("Bob", false, true, 100));
    }

    #[test]
    fn members_missing_from_a_snap_or_a_dropped_link_freeze() {
        let (mut app, want) = party_app();
        feed(
            &mut app,
            "09:13:00",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
        );
        let t0 = Instant::now();
        let s = snap(t0, &want, &[("usr_a", 300), ("usr_b", 100)]);
        app.party_step(t0, Some(&want), poll(Link::Connected, 1, vec![s]));
        let t1 = t0 + Duration::from_secs(1);
        app.party_step(
            t1,
            Some(&want),
            poll(Link::Connected, 1, vec![snap(t1, &want, &[("usr_a", 400)])]),
        );
        assert_eq!(
            rows(&app),
            [row("Alice", false, true, 400), row("Me", true, true, 0)]
        );
        app.party_step(t1, Some(&want), poll(Link::Retrying, 1, Vec::new()));
        assert_eq!(rows(&app)[0], row("Alice", false, false, 400));
        assert_eq!(app.party.pane().status().0, "can't reach the server");
        app.party_step(
            t1,
            Some(&want),
            poll(Link::Connected, 2, vec![snap(t1, &want, &[("usr_a", 450)])]),
        );
        assert_eq!(rows(&app)[0], row("Alice", false, true, 450));
        app.party_step(t1, Some(&want), poll(Link::Connected, 3, Vec::new()));
        assert_eq!(rows(&app)[0], row("Alice", false, false, 450));
    }

    #[test]
    fn rows_clear_at_the_next_fight_and_reset_on_a_new_room() {
        let (mut app, want) = party_app();
        feed(
            &mut app,
            "09:12:40",
            "[Behaviour] OnPlayerJoined Bob (usr_b)",
        );
        feed(
            &mut app,
            "09:12:41",
            "[Behaviour] OnPlayerJoined Cara (usr_c)",
        );
        feed(
            &mut app,
            "09:13:00",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
        );
        feed(&mut app, "09:13:01", "Dealing 100 STRIKE damage");
        let t0 = Instant::now();
        app.party_step(
            t0,
            Some(&want),
            poll(Link::Connected, 1, vec![snap(t0, &want, &[("usr_b", 300)])]),
        );
        feed(
            &mut app,
            "09:13:30",
            "Boss Kakarot dead, personal damage dealt: ",
        );
        app.party_step(
            t0,
            Some(&want),
            poll(Link::Connected, 1, vec![snap(t0, &want, &[])]),
        );
        assert_eq!(
            rows(&app),
            [row("Bob", false, false, 300), row("Me", true, true, 100)]
        );
        let fight = app.party.pane().fight.clone().unwrap();
        assert!(fight.down && !fight.live);
        assert_eq!(fight.secs, 30);
        feed(
            &mut app,
            "09:14:00",
            "ECLIPTICA - now fighting boss: DarkMouth(Clone) on phase: 0",
        );
        app.party_step(t0, Some(&want), poll(Link::Connected, 1, Vec::new()));
        assert_eq!(rows(&app), [row("Me", true, true, 0)]);
        app.party_step(
            t0,
            Some(&want),
            poll(
                Link::Connected,
                1,
                vec![snap(t0, &want, &[("usr_c", 5), ("usr_b", 9)])],
            ),
        );
        assert_eq!(rows(&app)[0], row("Bob", false, true, 9));
        assert_eq!(rows(&app)[1], row("Cara", false, true, 5));
        feed(&mut app, "09:15:00", "[Behaviour] OnLeftRoom");
        feed(&mut app, "09:15:01", ENTER);
        feed(&mut app, "09:15:01", W2);
        feed(
            &mut app,
            "09:15:02",
            "[Behaviour] OnPlayerJoined Cara (usr_c)",
        );
        let want2 = relay::want(true, true, app.gs.location.as_deref(), Some("usr_me")).unwrap();
        assert_ne!(want2.room(), want.room());
        app.party_step(t0, Some(&want2), poll(Link::Connected, 1, Vec::new()));
        assert!(rows(&app).is_empty());
        app.party_step(
            t0,
            Some(&want2),
            poll(Link::Connected, 2, vec![snap(t0, &want2, &[("usr_c", 7)])]),
        );
        assert_eq!(rows(&app), [row("Cara", false, true, 7)]);
        app.party_step(
            t0,
            Some(&want2),
            poll(
                Link::Connected,
                2,
                vec![snap(t0, &want2, &[("usr_c", 8), ("usr_b", 50)])],
            ),
        );
        assert_eq!(rows(&app), [row("Cara", false, true, 8)]);
    }

    #[test]
    fn turning_sharing_off_clears_the_table() {
        let (mut app, want) = party_app();
        feed(
            &mut app,
            "09:13:00",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
        );
        let t0 = Instant::now();
        app.party_step(
            t0,
            Some(&want),
            poll(Link::Connected, 1, vec![snap(t0, &want, &[("usr_a", 300)])]),
        );
        assert!(!app.party.pane().rows.is_empty());
        app.set_party(false);
        assert!(app.party_step(t0, None, None));
        assert!(rows(&app).is_empty());
        assert!(!app.party.pane().on);
        assert_eq!(app.party.pane().status().0, "off");
        app.set_party(true);
        app.party_step(t0, Some(&want), None);
        assert_eq!(rows(&app), [row("Me", true, true, 0)]);
        assert_eq!(app.party.pane().status().0, "connecting");
    }

    #[test]
    fn sharing_off_never_touches_the_relay() {
        let mut app = headless();
        let fake = fake_relay(&mut app);
        app.vrc_running = true;
        feed(&mut app, "09:12:29", "User Authenticated: Me (usr_me)");
        feed(&mut app, "09:12:30", W1);
        feed(
            &mut app,
            "09:13:00",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
        );
        feed(&mut app, "09:13:01", "Dealing 100 STRIKE damage");
        for _ in 0..5 {
            assert!(!app.poll_party());
        }
        assert!(fake.take().is_empty());
        assert!(!app.party.pane().on);
    }

    #[test]
    fn damage_is_sent_from_the_first_fight_on() {
        let (mut app, want) = party_app();
        let fake = fake_relay(&mut app);
        app.vrc_running = true;
        app.poll_party();
        assert_eq!(fake.take(), [(Some(want.clone()), None)]);
        feed(
            &mut app,
            "09:13:00",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
        );
        feed(&mut app, "09:13:01", "Dealing 100 STRIKE damage");
        feed(
            &mut app,
            "09:13:02",
            "damage has been taken: 25, from source: (Kakarot) attack_RockStomp",
        );
        app.poll_party();
        let sent = Some(Totals::from((100, 25)));
        assert_eq!(fake.take(), [(Some(want.clone()), sent)]);
        feed(
            &mut app,
            "09:13:30",
            "Boss Kakarot dead, personal damage dealt: ",
        );
        app.poll_party();
        assert_eq!(fake.take(), [(Some(want.clone()), sent)]);
        app.vrc_running = false;
        app.poll_party();
        assert_eq!(fake.take(), [(None, None)]);
    }

    #[test]
    fn turning_sharing_off_tells_the_relay_once() {
        let (mut app, want) = party_app();
        let fake = fake_relay(&mut app);
        app.vrc_running = true;
        feed(
            &mut app,
            "09:13:00",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
        );
        feed(&mut app, "09:13:01", "Dealing 100 STRIKE damage");
        app.poll_party();
        assert_eq!(fake.take(), [(Some(want), Some(Totals::from((100, 0))))]);
        app.set_party(false);
        assert!(app.poll_party());
        assert_eq!(fake.take(), [(None, None)]);
        assert!(!app.poll_party());
        assert!(fake.take().is_empty());
    }

    #[test]
    fn leaving_the_world_tells_the_relay() {
        let (mut app, want) = party_app();
        let fake = fake_relay(&mut app);
        app.vrc_running = true;
        app.poll_party();
        assert_eq!(fake.take(), [(Some(want), None)]);
        feed(&mut app, "09:15:00", "[Behaviour] OnLeftRoom");
        app.poll_party();
        assert_eq!(fake.take(), [(None, None)]);
    }

    #[test]
    fn snaps_from_a_dropped_link_are_never_applied() {
        let (mut app, want) = party_app();
        feed(
            &mut app,
            "09:13:00",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.1",
        );
        let hub = relay::tests::hub();
        let id = relay::tests::connect(hub);
        let t0 = Instant::now();
        relay::tests::push(hub, id, snap(t0, &want, &[("usr_a", 300)]));
        app.party_step(t0, Some(&want), Some(hub.poll()));
        assert_eq!(rows(&app)[0], row("Alice", false, true, 300));
        let t1 = t0 + Duration::from_secs(1);
        relay::tests::push(hub, id, snap(t1, &want, &[("usr_a", 900)]));
        relay::tests::drop_link(hub);
        app.party_step(t1, Some(&want), Some(hub.poll()));
        assert_eq!(rows(&app)[0], row("Alice", false, false, 300));
        relay::tests::push(hub, id, snap(t1, &want, &[("usr_a", 950)]));
        let p = hub.poll();
        assert!(p.snaps.is_empty());
        app.party_step(t1, Some(&want), Some(p));
        let stale = snap(t1, &want, &[("usr_a", 999)]);
        let session = hub.poll().session;
        app.party_step(t1, Some(&want), poll(Link::Retrying, session, vec![stale]));
        assert_eq!(rows(&app)[0], row("Alice", false, false, 300));
    }

    #[test]
    fn want_follows_the_toggle_and_the_world() {
        let (mut app, _) = party_app();
        app.vrc_running = true;
        assert!(relay::want(
            app.party_on,
            app.gs.in_ecliptica(),
            app.gs.location.as_deref(),
            app.gs.me_usr.as_deref()
        )
        .is_some());
        feed(&mut app, "09:15:00", "[Behaviour] OnLeftRoom");
        assert_eq!(app.gs.location, None);
        app.poll_party();
        assert!(app.party.pane().rows.is_empty());
    }
}

fn snapshot(gs: &GameState) -> String {
    format!(
        "{:?}
{:?}
{:?}",
        gs.runs, gs.taken, gs.history
    )
}

fn temp_logs(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ehud_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const DEATHS: &str = "2026.01.01 10:00:00 Debug      -  [Behaviour] Entering Room: Ecliptica - Demo Playtest
2026.01.01 10:00:01 Debug      -  ECLIPTICA - now in stage: Stage_Forest on phase: 0.2 as class: Blade
2026.01.01 10:00:05 Debug      -  ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.2
2026.01.01 10:00:06 Debug      -  Dealing 25 STRIKE damage
2026.01.01 10:00:07 Debug      -  damage has been taken: 40, from source: Kakarot
2026.01.01 10:00:08 Debug      -  Local controller dead, switching off.
2026.01.01 10:00:09 Debug      -  some unrelated line
2026.01.01 10:00:10 Debug      -  Local controller dead, switching off.
2026.01.01 10:00:11 Debug      -  Local controller dead, switching off.
2026.01.01 10:00:20 Debug      -  Dealing 38 NON-STRIKE damage
2026.01.01 10:00:30 Debug      -  Boss Kakarot dead, personal damage dealt: 
2026.01.01 10:05:00 Debug      -  last unrelated line
";

#[test]
fn archived_logs_replay_after_vrchat_deletes_them() {
    let dir = temp_logs("archive");
    let log_a = dir.join("output_log_2026-01-01_00-00-00.txt");
    std::fs::write(&log_a, DEATHS).unwrap();
    let log_b = dir.join("output_log_2026-01-02_00-00-00.txt");
    std::fs::write(
        &log_b,
        format!(
            "2026.01.02 10:00:00 Debug      -  {STAGE_B}
"
        ),
    )
    .unwrap();
    let mut db = crate::history::History::in_memory().unwrap();
    let mut from_file = GameState::default();
    archive::backfill(&mut from_file, Some(&dir), Some(&mut db));
    assert!(db.stored("output_log_2026-01-01_00-00-00.txt") < DEATHS.lines().count() as u64);
    std::fs::remove_file(&log_a).unwrap();
    let mut from_db = GameState::default();
    archive::backfill(&mut from_db, Some(&dir), Some(&mut db));
    assert_eq!(snapshot(&from_db), snapshot(&from_file));
    assert_eq!(from_file.runs[0].deaths, 0);
    assert!(from_file.runs[0].end_ts.is_some());
    archive::backfill(&mut GameState::default(), Some(&dir), Some(&mut db));
    assert_eq!(db.logs().len(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

fn kept_of(text: &str, old: bool) -> Vec<(u64, String)> {
    let mut gs = GameState::default();
    let mut kept = Vec::new();
    for line in text.lines() {
        if let Some((ts, Some(at))) = gs.feed_kept(line) {
            let msg = &line[at..];
            let world = matches!(
                crate::game::event::parse_msg(msg),
                Some(crate::game::event::Event::WorldLine(_))
            );
            if !(old && world) {
                kept.push((ts, msg.to_string()));
            }
        }
    }
    kept
}

#[test]
fn archives_from_older_builds_line_up_after_an_upgrade() {
    let dir = temp_logs("upgrade");
    let world = |t: &str, m: &str| format!("2026.01.01 10:00:{t} Debug      -  {m}\n");
    let mut text = String::new();
    for (i, line) in DEATHS.lines().enumerate() {
        text.push_str(line);
        text.push('\n');
        if i == 1 {
            text.push_str(&world("01", "ECLIPTICA - some new message"));
        }
        if i == 4 {
            text.push_str(&world("08", "Removed player Bob"));
        }
    }
    let closed = "output_log_2026-01-01_00-00-00.txt";
    let live = "output_log_2026-01-02_00-00-00.txt";
    std::fs::write(dir.join(closed), &text).unwrap();
    std::fs::write(dir.join(live), &text).unwrap();
    let old = kept_of(&text, true);
    assert_eq!(kept_of(&text, false).len(), old.len() + 2);
    let mut db = crate::history::History::in_memory().unwrap();
    for name in [closed, live] {
        db.archive(name, 0, &old, old.last().unwrap().0);
    }
    db.forget_skew();
    let mut app = App::new(96);
    app.set_history(Some(db));
    archive::backfill(&mut app.gs, Some(&dir), app.archive.db.as_mut());
    assert_eq!(
        app.archive.db.as_ref().unwrap().stored(closed),
        old.len() as u64
    );
    app.watch = LogWatch::for_dir(dir.clone());
    app.tick();
    let tail = format!(
        "{}{}",
        world("40", "Dealing 5 STRIKE damage"),
        world("41", "ECLIPTICA - another new message")
    );
    std::fs::OpenOptions::new()
        .append(true)
        .open(dir.join(live))
        .unwrap()
        .write_all(tail.as_bytes())
        .unwrap();
    app.tick();
    let db = app.archive.db.as_mut().unwrap();
    let mut got = Vec::new();
    db.replay(closed, |ts, m| got.push((ts, m.to_string())));
    assert_eq!(got, old);
    let mut got = Vec::new();
    db.replay(live, |ts, m| got.push((ts, m.to_string())));
    let mut want = old.clone();
    want.extend(kept_of(&tail, false));
    assert_eq!(got.len(), old.len() + 2);
    assert_eq!(got, want);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn lines_drained_from_the_old_log_are_archived_under_its_name() {
    let dir = temp_logs("drain_archive");
    let old = "output_log_2026-01-01_00-00-00.txt";
    let new = "output_log_2026-01-02_00-00-00.txt";
    let (head, tail) = DEATHS.split_at(DEATHS.find("2026.01.01 10:00:10").unwrap());
    std::fs::write(dir.join(old), head).unwrap();
    let mut app = App::new(96);
    app.set_history(crate::history::History::in_memory());
    app.watch = LogWatch::for_dir(dir.clone());
    app.tick();
    let before = app.archive.db.as_ref().unwrap().stored(old);
    std::fs::write(dir.join(new), "").unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(dir.join(old))
        .unwrap()
        .write_all(tail.as_bytes())
        .unwrap();
    app.tick();
    assert_eq!(app.watch.path.as_deref(), Some(dir.join(old).as_path()));
    app.tick();
    assert_eq!(app.watch.path.as_deref(), Some(dir.join(new).as_path()));
    let db = app.archive.db.as_ref().unwrap();
    assert!(before < 10);
    assert_eq!(db.stored(old), 10);
    assert_eq!(db.stored(new), 0);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn live_log_is_archived_as_it_is_read() {
    let dir = temp_logs("live_archive");
    let log_a = dir.join("output_log_2026-01-01_00-00-00.txt");
    let (head, tail) = DEATHS.split_at(DEATHS.find("2026.01.01 10:00:10").unwrap());
    std::fs::write(&log_a, head).unwrap();
    let mut app = App::new(96);
    app.set_history(crate::history::History::in_memory());
    app.watch = LogWatch::for_dir(dir.clone());
    app.tick();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&log_a)
        .unwrap()
        .write_all(tail.as_bytes())
        .unwrap();
    app.tick();
    let mut whole = GameState::default();
    for line in DEATHS.lines() {
        whole.feed(line);
    }
    whole.log_rotated();
    let db = app.archive.db.as_ref().unwrap();
    let mut replayed = GameState::default();
    let end = db.replay("output_log_2026-01-01_00-00-00.txt", |ts, m| {
        replayed.feed_msg(ts, m);
    });
    replayed.seen(end);
    replayed.log_rotated();
    assert_eq!(snapshot(&replayed), snapshot(&whole));
    let mut again = App::new(96);
    again.set_history(app.archive.db.take());
    again.watch = LogWatch::for_dir(dir.clone());
    again.tick();
    let n = again
        .archive
        .db
        .as_ref()
        .unwrap()
        .stored("output_log_2026-01-01_00-00-00.txt");
    let mut fresh = GameState::default();
    let kept = DEATHS
        .lines()
        .filter(|l| fresh.feed_kept(l).is_some_and(|(_, k)| k.is_some()))
        .count() as u64;
    assert_eq!(kept, 10);
    assert_eq!(n, kept);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
#[ignore]
fn real_logs_replay_the_same() {
    let dir = crate::vrchat::log::log_dir().unwrap();
    for path in crate::vrchat::log::all_logs(&dir) {
        let mut whole = GameState::default();
        let mut kept = Vec::new();
        let mut end = 0;
        let t = std::time::Instant::now();
        crate::vrchat::log::read_lines(&path, |line| {
            if let Some((ts, keep)) = whole.feed_kept(line) {
                end = end.max(ts);
                if let Some(at) = keep {
                    kept.push((ts, line[at..].to_string()));
                }
            }
        })
        .unwrap();
        let file_ms = t.elapsed().as_millis();
        whole.log_rotated();
        let file = std::env::temp_dir().join(format!("ehud-size-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&file);
        let mut db = crate::history::History::open(&file).unwrap();
        db.archive("x", 0, &kept, end);
        drop(db);
        let size = std::fs::metadata(&file).unwrap().len();
        let db = crate::history::History::open(&file).unwrap();
        let t = std::time::Instant::now();
        let mut replayed = GameState::default();
        let end = db.replay("x", |ts, m| {
            replayed.feed_msg(ts, m);
        });
        replayed.seen(end);
        replayed.log_rotated();
        let db_ms = t.elapsed().as_millis();
        let bytes: usize = kept.iter().map(|(_, m)| m.len() + 8).sum();
        println!(
            "{} kept={} bytes={} db={} runs={} file_ms={} db_ms={}",
            path.file_name().unwrap().to_string_lossy(),
            kept.len(),
            bytes,
            size,
            whole.runs.len(),
            file_ms,
            db_ms
        );
        assert_eq!(snapshot(&replayed), snapshot(&whole), "{}", path.display());
        drop(db);
        let _ = std::fs::remove_file(&file);
    }
}

#[test]
#[ignore]
fn many_sessions_load_cost() {
    let dir = crate::vrchat::log::log_dir().unwrap();
    let path = dir.join("output_log_2026-10-22_13-44-11.txt");
    let file = std::env::temp_dir().join("ehud-load-cost.db");
    let fresh = !file.exists();
    let mut db = crate::history::History::open(&file).unwrap();
    let mut probe = GameState::default();
    let mut kept = Vec::new();
    let mut end = 0;
    crate::vrchat::log::read_lines(&path, |line| {
        if let Some((ts, keep)) = probe.feed_kept(line) {
            end = end.max(ts);
            if let Some(at) = keep {
                kept.push((ts, line[at..].to_string()));
            }
        }
    })
    .unwrap();
    let n: usize = std::env::var("EHUD_SESSIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(365);
    for i in (0..n).filter(|_| fresh) {
        let shift = i as u64 * 86400;
        let moved: Vec<(u64, String)> = kept.iter().map(|(t, m)| (t + shift, m.clone())).collect();
        db.archive(&format!("output_log_{i:05}.txt"), 0, &moved, end + shift);
    }
    let t = std::time::Instant::now();
    let mut gs = GameState::default();
    archive::backfill(&mut gs, None, Some(&mut db));
    println!(
        "sessions={n} runs={} fights={} load_ms={} pid={}",
        gs.runs.len(),
        gs.runs.iter().map(|r| r.fights.len()).sum::<usize>(),
        t.elapsed().as_millis(),
        std::process::id()
    );
    std::thread::sleep(std::time::Duration::from_secs(4));
    drop(gs);
}

#[test]
fn leaderboard_switch_sits_over_the_taken_card() {
    let mut app = headless();
    let (x, y, w, h) = BOARD_HIT;
    let (bx, by) = (x + w / 2, y + BODY_DY + h / 2);
    assert!(!app.hit_enabled(Hit::Board));
    assert_ne!(app.enabled_hit(bx, by), Some(Hit::Board));
    assert!(app.hit_enabled(Hit::Info(Info::Taken(0))));
    app.activate(Hit::Settings);
    assert_eq!(app.enabled_hit(bx, by), Some(Hit::Board));
    for i in 0..3 {
        assert!(!app.hit_enabled(Hit::Info(Info::Taken(i))));
    }
    assert!(!app.hit_enabled(Hit::Info(Info::TakenGraph)));
    assert!(app.hit_enabled(Hit::Info(Info::Status)));
    assert!(!app.board_header().share_name);
    assert!(app.activate(Hit::Board));
    assert!(app.board_on);
    assert!(crate::board::ON.load(std::sync::atomic::Ordering::Relaxed));
    assert!(app.board_header().share_name);
    assert!(app.activate(Hit::Board));
    assert!(!app.board_on);
    assert!(!crate::board::ON.load(std::sync::atomic::Ordering::Relaxed));
    assert!(!app.board_header().share_name);
}

#[test]
fn deleting_board_kills_does_not_share_the_name() {
    let mut app = headless();
    app.delete_board();
    assert!(!app.board_header().share_name);
    app.set_board(true);
    app.delete_board();
    assert!(app.board_header().share_name);
}

#[test]
fn board_delete_needs_a_second_click() {
    let mut app = headless();
    let (x, y, w, h) = BOARD_DELETE_HIT;
    let (bx, by) = (x + w / 2, y + BODY_DY + h / 2);
    assert!(!app.hit_enabled(Hit::BoardDelete));
    app.activate(Hit::Settings);
    assert_eq!(app.enabled_hit(bx, by), Some(Hit::BoardDelete));
    app.activate(Hit::Board);
    app.activate(Hit::Board);
    assert!(!app.board_deleting);
    assert!(app.activate(Hit::BoardDelete));
    assert!(!app.board_deleting);
    assert!(app.board_armed.is_some());
    assert!(app.activate(Hit::BoardDelete));
    assert!(app.board_deleting);
    assert!(app.board_armed.is_none());
    assert!(crate::board::DELETING.load(std::sync::atomic::Ordering::Relaxed));
    assert!(!app.hit_enabled(Hit::BoardDelete));
}
