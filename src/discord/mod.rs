mod activity;
mod ipc;
mod log;
mod redact;

pub(crate) use activity::activity;
use activity::{clip, esc, valid_location};
use ipc::{Conn, DIAG_CLIP, set_activity_cmd};
pub(crate) use log::init as log_init;
use redact::redact;

use crate::game::names::{boss_name, phase_name, stage_name};
use crate::game::run::base_name;
use crate::game::state::{GameState, Life, Mode};
use crate::update::{REPO, json_str};
use std::fs::File;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};
use tracing::info;

pub(crate) const APP_ID: &str = "1549108707449905243";

const RETRY: Duration = Duration::from_secs(15);

const SEND_GAP: Duration = Duration::from_secs(15);

const POLL: Duration = Duration::from_millis(250);

const JOIN_WAIT: Duration = Duration::from_secs(45);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Link {
    Off,
    Waiting,
    Connected,
    Rejected,
}

static LINK: AtomicU8 = AtomicU8::new(0);

pub(crate) fn link() -> Link {
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
        info!("link {l:?}");
    }
}

fn launch_vrchat(location: &str) {
    if !valid_location(location) {
        info!("ignored join with an unexpected location");
        return;
    }
    info!("joining {}", location.split(':').next().unwrap_or_default());
    #[cfg(not(test))]
    {
        let uri = format!("vrchat://launch?ref=vrchat.com&id={location}");
        if let Err(e) = std::process::Command::new("explorer.exe").arg(uri).spawn() {
            info!("launching VRChat failed: {e}");
        }
    }
}

fn connect() -> Option<Conn> {
    if cfg!(test) { None } else { Conn::open() }
}

#[derive(Clone, PartialEq)]
enum Msg {
    Show(Option<String>),
    Off,
}

pub(crate) struct Presence {
    tx: Sender<Msg>,
    last: Msg,
}

impl Presence {
    pub(crate) fn new() -> Presence {
        let (tx, rx) = channel();
        std::thread::spawn(move || worker(&rx));
        Presence { tx, last: Msg::Off }
    }

    pub(crate) fn show(&mut self, activity: Option<String>) {
        let next = Msg::Show(activity);
        if self.last != next {
            let _ = self.tx.send(next.clone());
            self.last = next;
        }
    }

    pub(crate) fn off(&mut self) {
        let _ = self.tx.send(Msg::Off);
        self.last = Msg::Off;
    }
}

fn worker(rx: &Receiver<Msg>) {
    let mut want = Msg::Off;
    let mut sent = Msg::Off;
    let mut conn: Option<Conn> = None;
    let mut next_try = Instant::now();
    let mut next_send = Instant::now();
    loop {
        match rx.recv_timeout(POLL) {
            Ok(msg) => {
                want = msg;
                while let Ok(newer) = rx.try_recv() {
                    want = newer;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let Msg::Show(activity) = &want else {
            if let Some(mut c) = conn.take() {
                if matches!(sent, Msg::Show(Some(_))) {
                    let pid = std::process::id();
                    let _ = c.command(|n| set_activity_cmd(pid, n, None));
                }
                sent = Msg::Off;
            }
            set_link(Link::Off);
            continue;
        };
        if conn.is_none() {
            if Instant::now() < next_try {
                continue;
            }
            if let Some(c) = connect() {
                conn = Some(c);
                sent = Msg::Off;
                set_link(Link::Connected);
            } else {
                next_try = Instant::now() + RETRY;
                set_link(Link::Waiting);
                continue;
            }
        }
        let Some(c) = conn.as_mut() else {
            continue;
        };
        let result = c.pump().and_then(|joins| {
            for j in &joins {
                launch_vrchat(j);
            }
            let shown = matches!(&sent, Msg::Show(shown) if shown == activity);
            if !shown && Instant::now() >= next_send {
                let pid = std::process::id();
                c.command(|n| set_activity_cmd(pid, n, activity.as_deref()))?;
                info!(
                    "set_activity {}",
                    activity.as_deref().map_or_else(
                        || "clear".to_string(),
                        |a| redact(a).chars().take(DIAG_CLIP).collect()
                    )
                );
                sent = Msg::Show(activity.clone());
                next_send = Instant::now() + SEND_GAP;
            }
            Ok(())
        });
        if let Err(e) = result {
            info!("connection lost: {e}");
            conn = None;
            next_try = Instant::now() + RETRY;
            set_link(Link::Waiting);
        }
    }
}

pub(crate) fn join_listener() {
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
                info!("join listener lost Discord: {e}");
                conn = None;
            }
            None => {}
        }
        std::thread::sleep(POLL);
    }
    info!("join listener gave up waiting");
}

pub(crate) fn register_scheme() {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegSetKeyValueW};
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
        // SAFETY: every pointer is a NUL-terminated UTF-16 buffer that outlives the call, and the size is its byte length.
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                sub.as_ptr(),
                name.as_ref()
                    .map_or(std::ptr::null(), std::vec::Vec::as_ptr),
                REG_SZ,
                data.as_ptr().cast(),
                crate::num::to_u32(data.len() * 2),
            )
        };
        if status != 0 {
            info!("registering the join handler failed: {status}");
            return;
        }
    }
}
