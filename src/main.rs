#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod discord;
mod log;
mod logwatch;

mod game;
mod render;
mod scan;

mod ui;
mod update;
mod vr;

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
        Some("--scan") => scan::scan(args.next()),
        Some("--discord-join") => discord::join_listener(),
        _ => ui::run(),
    }
}
