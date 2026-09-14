#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod discord;
mod log;
mod logwatch;
mod names;
mod parse;
mod render;
mod state;
mod ui;
mod update;
mod vr;
mod vrcx;

use state::{fmt_clock, GameState};

fn main() {
    std::panic::set_hook(Box::new(|info| {
        if let Some(base) = std::env::var_os("APPDATA") {
            let dir = std::path::PathBuf::from(base).join("EclipticaHUD");
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join("crash.txt"), info.to_string());
        }
    }));
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--scan") => scan(args.next()),
        Some("--discord-join") => discord::join_listener(),
        _ => ui::run(),
    }
}

fn scan(path: Option<String>) {
    let path = path.map(std::path::PathBuf::from).or_else(|| {
        logwatch::log_dir()
            .as_deref()
            .and_then(logwatch::newest_log)
    });
    let Some(path) = path else {
        eprintln!("no VRChat log found");
        std::process::exit(1);
    };
    let Ok(file) = logwatch::open_shared(&path) else {
        eprintln!("cannot open {}", path.display());
        std::process::exit(1);
    };

    let mut gs = GameState::default();
    let mut counts = std::collections::BTreeMap::new();
    let mut reader = std::io::BufReader::new(file);
    let mut raw = String::new();
    use std::io::BufRead;
    loop {
        raw.clear();
        match reader.read_line(&mut raw) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let Some(line) = parse::split_line(raw.trim_end_matches(['\r', '\n'])) else {
            continue;
        };
        if let Some(ev) = parse::parse_msg(line.msg) {
            *counts.entry(ev.label()).or_insert(0u64) += 1;
            let before = gs.targets_total;
            let hits_before = gs.taken.back().map_or(0, |h| h.seq);
            let boss_before = gs.boss.is_some();
            gs.apply(line.ts, ev);
            if !boss_before && gs.boss.is_some() {
                if let Some((got, total)) = gs.tokens_shown() {
                    println!(
                        "{}  tokens  {got}/{total}  {}",
                        fmt_clock(line.ts),
                        gs.stage
                    );
                }
            }
            if let Some(hit) = gs.taken.back().filter(|h| h.seq != hits_before) {
                let (who, attack) = state::describe_source(&hit.source, hit.amount);
                println!(
                    "{}  hit     {:>4}  {who}  {attack}",
                    fmt_clock(hit.ts),
                    hit.amount
                );
            }
            if gs.targets_total > before {
                let hit = gs.history.back().unwrap();
                println!(
                    "{}  target  {}  <-  {}",
                    fmt_clock(hit.ts),
                    hit.player,
                    hit.boss
                );
            }
        }
    }
    println!("--- {}", path.display());
    for (label, n) in counts {
        println!("{label}: {n}");
    }
    for (i, r) in gs.runs.iter().enumerate() {
        println!(
            "run {:>2}  {}  {}  {:>2} fights  {}",
            i + 1,
            fmt_clock(r.start_ts),
            if r.lost { "LOST" } else { "run " },
            r.fights.len(),
            r.fights.last().map_or("", |f| f.name.as_str())
        );
    }
    println!("boss targets recorded: {}", gs.targets_total);
}
