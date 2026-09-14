mod activity;
mod ipc;

pub use activity::activity;
use activity::{clip, esc, valid_location};
use ipc::*;

use crate::game::names::{boss_name, phase_name, stage_name};
use crate::game::run::base_name;
use crate::game::state::{GameState, Mode};
use crate::update::{json_str, REPO};
use std::fs::File;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

pub const APP_ID: &str = "1549108707449905243";

const RETRY: Duration = Duration::from_secs(15);

const SEND_GAP: Duration = Duration::from_secs(15);

const POLL: Duration = Duration::from_millis(250);

const JOIN_WAIT: Duration = Duration::from_secs(45);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Link {
    Off,
    Waiting,
    Connected,
    Rejected,
}

static LINK: AtomicU8 = AtomicU8::new(0);

pub fn link() -> Link {
    match LINK.load(Ordering::Relaxed) {
        1 => Link::Waiting,
        2 => Link::Connected,
        3 => Link::Rejected,
        _ => Link::Off,
    }
}

pub(super) fn set_link(l: Link) {
    let v = match l {
        Link::Off => 0,
        Link::Waiting => 1,
        Link::Connected => 2,
        Link::Rejected => 3,
    };
    if LINK.swap(v, Ordering::Relaxed) != v {
        diag(&format!("link {l:?}"));
    }
}

pub(super) fn diag(msg: &str) {
    #[cfg(not(test))]
    if let Some(base) = std::env::var_os("APPDATA") {
        let path = std::path::PathBuf::from(base)
            .join("EclipticaHUD")
            .join("discord.log");
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > 64 * 1024) {
            let _ = std::fs::remove_file(&path);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let _ = writeln!(f, "{secs} {msg}");
        }
    }
    #[cfg(test)]
    let _ = msg;
}

fn launch_vrchat(location: &str) {
    if !valid_location(location) {
        diag("ignored join with an unexpected location");
        return;
    }
    diag(&format!(
        "joining {}",
        location.split(':').next().unwrap_or_default()
    ));
    #[cfg(not(test))]
    {
        let uri = format!("vrchat://launch?ref=vrchat.com&id={location}");
        if let Err(e) = std::process::Command::new("explorer.exe").arg(uri).spawn() {
            diag(&format!("launching VRChat failed: {e}"));
        }
    }
}

fn connect() -> Option<Conn> {
    if cfg!(test) {
        None
    } else {
        Conn::open()
    }
}

enum Msg {
    Show(Option<String>),
    Off,
}

pub struct Presence {
    tx: Sender<Msg>,
    last: Option<Option<String>>,
}

impl Presence {
    pub fn new() -> Presence {
        let (tx, rx) = channel();
        std::thread::spawn(move || worker(rx));
        Presence { tx, last: None }
    }

    pub fn show(&mut self, activity: Option<String>) {
        if self.last.as_ref() != Some(&activity) {
            let _ = self.tx.send(Msg::Show(activity.clone()));
            self.last = Some(activity);
        }
    }

    pub fn off(&mut self) {
        let _ = self.tx.send(Msg::Off);
        self.last = None;
    }
}

fn worker(rx: Receiver<Msg>) {
    let mut want: Option<Option<String>> = None;
    let mut sent: Option<Option<String>> = None;
    let mut conn: Option<Conn> = None;
    let mut next_try = Instant::now();
    let mut next_send = Instant::now();
    loop {
        match rx.recv_timeout(POLL) {
            Ok(msg) => {
                let mut apply = |m| match m {
                    Msg::Show(a) => want = Some(a),
                    Msg::Off => want = None,
                };
                apply(msg);
                while let Ok(m) = rx.try_recv() {
                    apply(m);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let Some(activity) = want.as_ref() else {
            if let Some(mut c) = conn.take() {
                if matches!(sent, Some(Some(_))) {
                    let pid = std::process::id();
                    let _ = c.command(|n| set_activity_cmd(pid, n, None));
                }
                sent = None;
            }
            set_link(Link::Off);
            continue;
        };
        if conn.is_none() {
            if Instant::now() < next_try {
                continue;
            }
            match connect() {
                Some(c) => {
                    conn = Some(c);
                    sent = None;
                    set_link(Link::Connected);
                }
                None => {
                    next_try = Instant::now() + RETRY;
                    set_link(Link::Waiting);
                    continue;
                }
            }
        }
        let c = conn.as_mut().unwrap();
        let result = c.pump().and_then(|joins| {
            joins.iter().for_each(|j| launch_vrchat(j));
            if sent.as_ref() != Some(activity) && Instant::now() >= next_send {
                let pid = std::process::id();
                c.command(|n| set_activity_cmd(pid, n, activity.as_deref()))?;
                diag(&format!(
                    "set_activity {}",
                    activity.as_deref().map_or_else(
                        || "clear".to_string(),
                        |a| a.chars().take(DIAG_CLIP).collect()
                    )
                ));
                sent = Some(activity.clone());
                next_send = Instant::now() + SEND_GAP;
            }
            Ok(())
        });
        if let Err(e) = result {
            diag(&format!("connection lost: {e}"));
            conn = None;
            next_try = Instant::now() + RETRY;
            set_link(Link::Waiting);
        }
    }
}

pub fn join_listener() {
    let deadline = Instant::now() + JOIN_WAIT;
    let mut conn = None;
    while Instant::now() < deadline {
        if conn.is_none() {
            conn = connect();
        }
        match conn.as_mut().map(Conn::pump) {
            Some(Ok(joins)) => {
                if let Some(j) = joins.first() {
                    launch_vrchat(j);
                    return;
                }
            }
            Some(Err(e)) => {
                diag(&format!("join listener lost Discord: {e}"));
                conn = None;
            }
            None => {}
        }
        std::thread::sleep(POLL);
    }
    diag("join listener gave up waiting");
}

pub fn register_scheme() {
    use windows_sys::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};
    if cfg!(test) {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let exe = exe.display().to_string();
    let key = format!(r"Software\Classes\discord-{APP_ID}");
    let values = [
        (key.clone(), None, format!("URL:Run game {APP_ID} protocol")),
        (key.clone(), Some("URL Protocol"), String::new()),
        (format!(r"{key}\DefaultIcon"), None, exe.clone()),
        (
            format!(r"{key}\shell\open\command"),
            None,
            format!("\"{exe}\" --discord-join"),
        ),
    ];
    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    for (sub, name, data) in values {
        let sub = wide(&sub);
        let name = name.map(wide);
        let data = wide(&data);
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                sub.as_ptr(),
                name.as_ref().map_or(std::ptr::null(), |n| n.as_ptr()),
                REG_SZ,
                data.as_ptr() as _,
                (data.len() * 2) as u32,
            )
        };
        if status != 0 {
            diag(&format!("registering the join handler failed: {status}"));
            return;
        }
    }
}
