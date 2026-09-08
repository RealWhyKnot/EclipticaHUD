#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod logwatch;
mod parse;
mod render;
mod state;
mod ui;
mod vr;

use state::{fmt_clock, GameState};

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--scan") => scan(args.next()),
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
            gs.apply(line.ts, ev);
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
    println!("boss targets recorded: {}", gs.targets_total);
}
