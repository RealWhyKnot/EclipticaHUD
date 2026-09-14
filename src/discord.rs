use crate::names::{boss_name, phase_name, stage_name};
use crate::state::{base_name, phase_num, GameState, Mode};
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
const REPLY_WAIT: Duration = Duration::from_secs(5);
const JOIN_WAIT: Duration = Duration::from_secs(45);
const CYCLE_SECS: u64 = 15;
const TEXT_MAX: usize = 128;

const BOSS_ART: &[&str] = &[
    "amaziah",
    "antking",
    "blacklily",
    "bravera",
    "buffnoob",
    "conehead",
    "corus",
    "darkmouth",
    "despair",
    "flylord",
    "goldengrouch",
    "gravetender",
    "jackedpumpkin",
    "jimbringer",
    "kakarot",
    "kodama",
    "m41d",
    "manalyteancient",
    "maxipuss",
    "melon",
    "mephiel",
    "middleman",
    "nan",
    "neopilot",
    "nx_obsidian",
    "obisidus",
    "oone",
    "pandora",
    "pride",
    "queenbug",
    "steven",
    "yuki",
];

const CLASS_ART: &[&str] = &[
    "spellsword",
    "twinmage",
    "gunmancer",
    "fistmage",
    "spellhammer",
    "shieldmage",
    "thaumaturge",
    "nekomancer",
];

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

fn set_link(l: Link) {
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

fn diag(msg: &str) {
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

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn clip(s: &str) -> String {
    s.chars().take(TEXT_MAX).collect()
}

fn art_key(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn boss_art(base: &str) -> Option<String> {
    let key = art_key(base);
    BOSS_ART
        .contains(&key.as_str())
        .then(|| format!("boss_{key}"))
}

fn class_art(class: &str) -> Option<String> {
    let key: String = class
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    CLASS_ART
        .contains(&key.as_str())
        .then(|| format!("class_{key}"))
}

fn short(n: u64) -> String {
    match n {
        0..=9_999 => n.to_string(),
        10_000..=999_999 => format!("{:.1}k", n as f64 / 1000.0),
        _ => format!("{:.2}M", n as f64 / 1_000_000.0),
    }
}

pub fn valid_location(s: &str) -> bool {
    let Some((world, inst)) = s.split_once(':') else {
        return false;
    };
    let Some(id) = world.strip_prefix("wrld_") else {
        return false;
    };
    id.len() == 36
        && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
        && !inst.is_empty()
        && inst.len() <= 200
        && inst
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"~()_-.".contains(&b))
}

pub fn web_join_url(location: &str) -> Option<String> {
    let (world, inst) = location.split_once(':')?;
    valid_location(location)
        .then(|| format!("https://vrchat.com/home/launch?worldId={world}&instanceId={inst}"))
}

fn party_id(location: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in location.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

pub fn activity(gs: &GameState, now: u64, unix_now: u64) -> Option<String> {
    let at = |ts: u64| unix_now.saturating_sub(now.saturating_sub(ts));
    let run = gs.runs.last().filter(|r| r.end_ts.is_none());
    let mut state: Vec<String> = Vec::new();
    let mut large = "logo".to_string();
    let mut large_text = "Ecliptica".to_string();
    let mut since = run.map(|r| r.start_ts);
    let details = match gs.mode {
        Mode::Idle if !gs.in_ecliptica() => return None,
        Mode::Idle | Mode::Lobby => {
            since = None;
            "In the lobby".to_string()
        }
        Mode::Intermission => {
            if let Some(r) = run {
                let mut facts = vec![
                    format!("{} bosses down", r.kills()),
                    format!("{} damage dealt", short(r.dmg())),
                    format!("{} damage taken", short(r.taken())),
                ];
                if r.deaths > 0 {
                    facts.push(format!("{} deaths", r.deaths));
                }
                facts.push(format!(
                    "{}m in the run",
                    now.saturating_sub(r.start_ts) / 60
                ));
                state.push(pick(&facts, unix_now));
            }
            match gs.stage_no {
                Some(n) => format!("Intermission after stage {n}"),
                None => "Intermission".to_string(),
            }
        }
        Mode::Stage => match gs.boss.as_deref() {
            Some(boss) => {
                let base = base_name(boss);
                let name = boss_name(base);
                if let Some(key) = boss_art(base) {
                    large = key;
                }
                large_text = name.to_string();
                since = Some(gs.fight_start);
                state.push(format!("{} DPS", short(gs.fight_dps(now))));
                state.push(format!("{} damage", short(gs.fight_dmg)));
                match phase_num(boss) {
                    1 => format!("Fighting {name}"),
                    p => format!("Fighting {name} (P{p})"),
                }
            }
            None => {
                large_text = stage_name(&gs.stage).to_string();
                let s = &gs.stage_stats;
                let mut facts = Vec::new();
                if let Some((got, total)) = gs.tokens_shown() {
                    facts.push(format!("Tokens {got}/{total}"));
                }
                if let Some(n) = gs.stage_no {
                    facts.push(format!("Stage {n}"));
                }
                facts.push(format!("{} DPS clearing", short(gs.stage_dps(now))));
                facts.push(format!("{} damage dealt", short(s.dmg)));
                facts.push(format!("{} damage taken", short(s.taken)));
                facts.push(format!("{} hits taken", s.hits));
                if let Some(d) = run.map(|r| r.deaths).filter(|d| *d > 0) {
                    facts.push(format!("{d} deaths"));
                }
                if s.start_ts > 0 {
                    facts.push(format!(
                        "Clearing for {}m",
                        now.saturating_sub(s.start_ts) / 60
                    ));
                }
                state.push(pick(&facts, unix_now));
                format!("{} | {}", stage_name(&gs.stage), phase_name(gs.progress))
            }
        },
    };
    if gs.boss.is_some() {
        if let Some(d) = run.map(|r| r.deaths).filter(|d| *d > 0) {
            state.push(if d == 1 {
                "1 death".to_string()
            } else {
                format!("{d} deaths")
            });
        }
    }
    if gs.is_dead(now) {
        state.insert(0, "Dead".to_string());
    }

    let location = gs.location.as_deref().filter(|l| valid_location(l));
    let join_url = location.and_then(web_join_url);
    let secret = location.filter(|l| l.len() <= TEXT_MAX);
    let github = format!("https://github.com/{REPO}/releases/latest");
    let mut fields = vec![
        "\"type\":5".to_string(),
        format!("\"details\":{}", esc(&clip(&details))),
    ];
    if let (Some(url), Some(_)) = (&join_url, secret) {
        fields.push(format!("\"details_url\":{}", esc(url)));
    }
    if !state.is_empty() {
        fields.push(format!("\"state\":{}", esc(&clip(&state.join(" | ")))));
    }
    if let Some(ts) = since {
        fields.push(format!("\"timestamps\":{{\"start\":{}}}", at(ts)));
    }
    let mut assets = vec![
        format!("\"large_image\":{}", esc(&large)),
        format!("\"large_text\":{}", esc(&clip(&large_text))),
        format!("\"large_url\":{}", esc(&github)),
    ];
    if let Some(key) = class_art(&gs.class) {
        assets.push(format!("\"small_image\":{}", esc(&key)));
        assets.push(format!("\"small_text\":{}", esc(&clip(&gs.class))));
    }
    fields.push(format!("\"assets\":{{{}}}", assets.join(",")));
    match secret {
        Some(loc) => fields.push(format!(
            "\"party\":{{\"id\":{}}},\"secrets\":{{\"join\":{}}}",
            esc(&party_id(loc)),
            esc(loc)
        )),
        None => {
            let mut buttons = Vec::new();
            if let Some(url) = &join_url {
                buttons.push(format!(
                    "{{\"label\":\"Join in VRChat\",\"url\":{}}}",
                    esc(url)
                ));
            }
            buttons.push(format!(
                "{{\"label\":\"Get EclipticaHUD\",\"url\":{}}}",
                esc(&github)
            ));
            fields.push(format!("\"buttons\":[{}]", buttons.join(",")));
        }
    }
    Some(format!("{{{}}}", fields.join(",")))
}

fn pick(facts: &[String], unix_now: u64) -> String {
    facts[(unix_now / CYCLE_SECS) as usize % facts.len()].clone()
}

fn frame(op: u32, body: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body.as_bytes());
    out
}

fn set_activity_cmd(pid: u32, nonce: u64, activity: Option<&str>) -> String {
    match activity {
        Some(a) => format!(
            "{{\"cmd\":\"SET_ACTIVITY\",\"args\":{{\"pid\":{pid},\"activity\":{a}}},\"nonce\":\"{nonce}\"}}"
        ),
        None => format!(
            "{{\"cmd\":\"SET_ACTIVITY\",\"args\":{{\"pid\":{pid}}},\"nonce\":\"{nonce}\"}}"
        ),
    }
}

fn join_secret(body: &str) -> Option<String> {
    if json_str(body, "evt").as_deref() != Some("ACTIVITY_JOIN") {
        return None;
    }
    json_str(body, "secret")
}

const DIAG_CLIP: usize = 600;

struct Conn {
    pipe: File,
    nonce: u64,
    last_reply: String,
}

impl Conn {
    fn open() -> Option<Conn> {
        for i in 0..10 {
            let Ok(pipe) = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(format!(r"\\?\pipe\discord-ipc-{i}"))
            else {
                continue;
            };
            let mut conn = Conn {
                pipe,
                nonce: 0,
                last_reply: String::new(),
            };
            match conn.handshake() {
                Ok(()) => {
                    diag(&format!("connected on discord-ipc-{i}"));
                    return Some(conn);
                }
                Err(e) => diag(&format!("handshake on discord-ipc-{i} failed: {e}")),
            }
        }
        None
    }

    fn send(&mut self, op: u32, body: &str) -> io::Result<()> {
        self.pipe.write_all(&frame(op, body))
    }

    fn command(&mut self, body: impl FnOnce(u64) -> String) -> io::Result<()> {
        self.nonce += 1;
        let text = body(self.nonce);
        self.send(1, &text)
    }

    fn available(&self) -> io::Result<u32> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::Pipes::PeekNamedPipe;
        let mut avail = 0u32;
        let ok = unsafe {
            PeekNamedPipe(
                self.pipe.as_raw_handle() as _,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut avail,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(avail)
        }
    }

    fn next_frame(&mut self) -> io::Result<Option<(u32, String)>> {
        if self.available()? < 8 {
            return Ok(None);
        }
        let mut head = [0u8; 8];
        self.pipe.read_exact(&mut head)?;
        let op = u32::from_le_bytes(head[..4].try_into().unwrap());
        let len = u32::from_le_bytes(head[4..].try_into().unwrap()) as usize;
        if len > 64 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "frame too large",
            ));
        }
        let mut body = vec![0u8; len];
        self.pipe.read_exact(&mut body)?;
        Ok(Some((op, String::from_utf8_lossy(&body).into_owned())))
    }

    fn handshake(&mut self) -> io::Result<()> {
        self.send(0, &format!("{{\"v\":1,\"client_id\":{}}}", esc(APP_ID)))?;
        let deadline = Instant::now() + REPLY_WAIT;
        while Instant::now() < deadline {
            match self.next_frame()? {
                Some((1, body)) if json_str(&body, "evt").as_deref() == Some("READY") => {
                    return self.command(|n| {
                        format!(
                            "{{\"cmd\":\"SUBSCRIBE\",\"evt\":\"ACTIVITY_JOIN\",\"nonce\":\"{n}\"}}"
                        )
                    });
                }
                Some((2, body)) => {
                    return Err(io::Error::new(io::ErrorKind::ConnectionRefused, body));
                }
                Some(_) => {}
                None => std::thread::sleep(Duration::from_millis(50)),
            }
        }
        Err(io::Error::new(io::ErrorKind::TimedOut, "no READY"))
    }

    fn pump(&mut self) -> io::Result<Vec<String>> {
        let mut joins = Vec::new();
        while let Some((op, body)) = self.next_frame()? {
            match op {
                3 => self.send(4, &body)?,
                2 => return Err(io::Error::new(io::ErrorKind::ConnectionAborted, body)),
                _ => {
                    let evt = json_str(&body, "evt");
                    if let Some(secret) = join_secret(&body) {
                        joins.push(secret);
                    } else if evt.as_deref() == Some("ERROR") {
                        if link() != Link::Rejected {
                            diag(&format!("discord error: {}", clip(&body)));
                        }
                        set_link(Link::Rejected);
                    } else if json_str(&body, "cmd").as_deref() == Some("SET_ACTIVITY") {
                        let shown: String = body.chars().take(DIAG_CLIP).collect();
                        if shown != self.last_reply {
                            diag(&format!("reply {shown}"));
                            self.last_reply = shown;
                        }
                        set_link(Link::Connected);
                    }
                }
            }
        }
        Ok(joins)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const LOC: &str = "wrld_0fb88df3-2057-4c2f-8e06-e948864378fd:87887~hidden(usr_4e64b21b-fbd0-4c12-8b55-c8c500b517b1)~region(use)";

    fn feed(gs: &mut GameState, t: &str, msg: &str) -> u64 {
        gs.feed(&format!("2026.09.14 {t} Debug      -  {msg}"))
            .unwrap()
    }

    #[test]
    fn escapes_json_strings() {
        assert_eq!(esc("a\"b\\c\nd"), "\"a\\\"b\\\\c\\u000ad\"");
        assert_eq!(esc("Be\u{430}rHands"), "\"Be\u{430}rHands\"");
    }

    #[test]
    fn frames_are_little_endian() {
        let f = frame(1, "{}");
        assert_eq!(&f[..8], &[1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(&f[8..], b"{}");
    }

    #[test]
    fn short_numbers() {
        assert_eq!(short(950), "950");
        assert_eq!(short(21059), "21.1k");
        assert_eq!(short(1_250_000), "1.25M");
    }

    #[test]
    fn art_keys_match_uploaded_assets() {
        assert_eq!(boss_art("NX-Obsidian").as_deref(), Some("boss_nx_obsidian"));
        assert_eq!(boss_art("FlyLord").as_deref(), Some("boss_flylord"));
        assert_eq!(boss_art("SomeNewBoss"), None);
        assert_eq!(
            class_art("Spellhammer").as_deref(),
            Some("class_spellhammer")
        );
        assert_eq!(
            class_art("Shield Mage").as_deref(),
            Some("class_shieldmage")
        );
        assert_eq!(class_art(""), None);
        assert_eq!(BOSS_ART.len() + CLASS_ART.len() + 1, 41);
    }

    #[test]
    fn locations_are_validated() {
        assert!(valid_location(LOC));
        assert!(valid_location(
            "wrld_cf971c96-e449-4e31-aeee-2c6d89e94915:56097~group(grp_c8503b1e-e7a7-46d7-8ebd-47eeb425251d)~groupAccessType(plus)~region(use)"
        ));
        assert!(!valid_location("wrld_0fb88df3:1"));
        assert!(!valid_location(
            "wrld_0fb88df3-2057-4c2f-8e06-e948864378fd:1&calc.exe"
        ));
        assert!(!valid_location(
            "wrld_0fb88df3-2057-4c2f-8e06-e948864378fd:1 \"x\""
        ));
        assert!(!valid_location(
            "wrld_0fb88df3-2057-4c2f-8e06-e948864378fd:"
        ));
        assert_eq!(
            web_join_url(LOC).as_deref(),
            Some("https://vrchat.com/home/launch?worldId=wrld_0fb88df3-2057-4c2f-8e06-e948864378fd&instanceId=87887~hidden(usr_4e64b21b-fbd0-4c12-8b55-c8c500b517b1)~region(use)")
        );
    }

    #[test]
    fn join_secret_only_from_join_dispatch() {
        let body = format!(
            "{{\"cmd\":\"DISPATCH\",\"data\":{{\"secret\":\"{LOC}\"}},\"evt\":\"ACTIVITY_JOIN\"}}"
        );
        assert_eq!(join_secret(&body).as_deref(), Some(LOC));
        assert_eq!(
            join_secret(
                "{\"cmd\":\"DISPATCH\",\"data\":{\"secret\":\"x\"},\"evt\":\"ACTIVITY_SPECTATE\"}"
            ),
            None
        );
    }

    #[test]
    fn set_activity_command_shape() {
        assert_eq!(
            set_activity_cmd(42, 7, Some("{\"details\":\"x\"}")),
            "{\"cmd\":\"SET_ACTIVITY\",\"args\":{\"pid\":42,\"activity\":{\"details\":\"x\"}},\"nonce\":\"7\"}"
        );
        assert_eq!(
            set_activity_cmd(42, 8, None),
            "{\"cmd\":\"SET_ACTIVITY\",\"args\":{\"pid\":42},\"nonce\":\"8\"}"
        );
    }

    #[test]
    #[ignore]
    fn live_handshake() {
        let mut conn = Conn::open().expect("Discord running with IPC");
        std::thread::sleep(Duration::from_millis(500));
        let (op, body) = conn.next_frame().unwrap().expect("subscribe reply");
        println!("{op} {body}");
        assert_eq!(op, 1);
        assert_eq!(json_str(&body, "cmd").as_deref(), Some("SUBSCRIBE"));
        assert_ne!(json_str(&body, "evt").as_deref(), Some("ERROR"));
    }

    fn live_reply(conn: &mut Conn) -> String {
        let deadline = Instant::now() + REPLY_WAIT;
        while Instant::now() < deadline {
            match conn.next_frame().unwrap() {
                Some((1, body)) if json_str(&body, "cmd").as_deref() == Some("SET_ACTIVITY") => {
                    return body;
                }
                Some(_) => {}
                None => std::thread::sleep(Duration::from_millis(50)),
            }
        }
        panic!("no SET_ACTIVITY reply");
    }

    #[test]
    #[ignore]
    fn live_set_activity() {
        let mut gs = GameState::default();
        feed(&mut gs, "03:59:35", &format!("[Behaviour] Joining {LOC}"));
        for _ in 0..3 {
            feed(&mut gs, "04:02:02", "spawn token, False, 0");
        }
        let now = feed(
            &mut gs,
            "04:02:02",
            "ECLIPTICA - now in stage: Stage_BalboaRuins on phase: 0.19 as class: Nekomancer",
        );
        let unix_now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let a = activity(&gs, now, unix_now).unwrap();
        let mut conn = Conn::open().expect("Discord running with IPC");
        let pid = std::process::id();
        conn.command(|n| set_activity_cmd(pid, n, Some(&a)))
            .unwrap();
        let reply = live_reply(&mut conn);
        println!("{reply}");
        std::thread::sleep(Duration::from_secs(3));
        conn.command(|n| set_activity_cmd(pid, n, None)).unwrap();
        println!("{}", live_reply(&mut conn));
        assert_ne!(json_str(&reply, "evt").as_deref(), Some("ERROR"), "{reply}");
    }

    #[test]
    fn idle_clears_presence() {
        let gs = GameState::default();
        assert_eq!(activity(&gs, 0, 1_000), None);
    }

    #[test]
    fn stage_and_intermission_cycle_facts() {
        let mut gs = GameState::default();
        for _ in 0..3 {
            feed(&mut gs, "04:02:02", "spawn token, False, 0");
        }
        feed(
            &mut gs,
            "04:02:02",
            "ECLIPTICA - now in stage: Stage_BalboaRuins on phase: 0.19 as class: Nekomancer",
        );
        feed(&mut gs, "04:02:10", "Dealing 300 STRIKE damage");
        let now = feed(
            &mut gs,
            "04:02:12",
            "damage has been taken: 8, from source: machinegunShooter1",
        );
        let states: Vec<String> = (0..8)
            .map(|i| json_str(&activity(&gs, now, i * CYCLE_SECS).unwrap(), "state").unwrap())
            .collect();
        assert!(states.contains(&"Tokens 0/3".to_string()), "{states:?}");
        assert!(
            states.contains(&"300 damage dealt".to_string()),
            "{states:?}"
        );
        assert!(states.contains(&"8 damage taken".to_string()), "{states:?}");
        assert!(states.contains(&"1 hits taken".to_string()), "{states:?}");
        assert!(
            states.iter().any(|s| s.ends_with(" DPS clearing")),
            "{states:?}"
        );
        assert_ne!(states[0], states[1]);
        feed(
            &mut gs,
            "04:03:00",
            "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.19",
        );
        feed(
            &mut gs,
            "04:03:01",
            "Boss Kakarot dead, personal damage dealt: ",
        );
        feed(&mut gs, "04:03:01", "STRIKE DMG: 900");
        feed(&mut gs, "04:03:01", "NON-STRIKE DMG: 0");
        let now = feed(&mut gs, "04:03:05", "ECLIPTICA - now in intermission");
        let states: Vec<String> = (0..5)
            .map(|i| json_str(&activity(&gs, now, i * CYCLE_SECS).unwrap(), "state").unwrap())
            .collect();
        assert!(states.contains(&"1 bosses down".to_string()), "{states:?}");
        assert!(
            states.contains(&"300 damage dealt".to_string()),
            "{states:?}"
        );
        assert!(states.contains(&"1m in the run".to_string()), "{states:?}");
    }

    #[test]
    fn joining_the_world_shows_the_lobby() {
        let mut gs = GameState::default();
        feed(&mut gs, "03:59:35", "[Behaviour] Entering Room: Sky Dream");
        assert_eq!(activity(&gs, 0, 1_000), None);
        let now = feed(
            &mut gs,
            "04:00:00",
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        );
        let a = activity(&gs, now, 1_000).unwrap();
        assert!(a.contains("\"details\":\"In the lobby\""), "{a}");
        assert!(!a.contains("timestamps"), "{a}");
    }

    #[test]
    fn stage_presence_shows_tokens_and_join() {
        let mut gs = GameState::default();
        feed(&mut gs, "03:59:35", &format!("[Behaviour] Joining {LOC}"));
        for _ in 0..3 {
            feed(&mut gs, "04:02:02", "spawn token, False, 0");
        }
        let start = feed(
            &mut gs,
            "04:02:02",
            "ECLIPTICA - now in stage: Stage_BalboaRuins on phase: 0.19 as class: Nekomancer",
        );
        feed(&mut gs, "04:02:03", "Advancing Stage Progress to: 2");
        let now = feed(&mut gs, "04:02:26", "ECLIPTICA saving SESSION ID 2505");
        let a = activity(&gs, now, 1_800_000_000).unwrap();
        assert!(a.starts_with("{\"type\":5,"), "{a}");
        assert!(a.contains("\"details\":\"Balboa Ruins | Primal\""), "{a}");
        assert!(a.contains("\"state\":\"Stage 2\""), "{a}");
        let b = activity(&gs, now, 1_799_999_985).unwrap();
        assert!(b.contains("\"state\":\"Tokens 1/3\""), "{b}");
        assert!(a.contains(&format!(
            "\"timestamps\":{{\"start\":{}}}",
            1_800_000_000 - (now - start)
        )));
        assert!(a.contains("\"large_image\":\"logo\""));
        assert!(a.contains("\"small_image\":\"class_nekomancer\""));
        assert!(!a.contains("buttons"), "{a}");
        assert!(
            a.contains("\"details_url\":\"https://vrchat.com/home/launch?worldId=wrld_0fb88df3")
        );
        assert!(a.contains(
            "\"large_url\":\"https://github.com/RealWhyKnot/EclipticaHUD/releases/latest\""
        ));
        assert!(a.contains(&format!("\"secrets\":{{\"join\":\"{LOC}\"}}")));
        assert!(a.contains("\"party\":{\"id\":\""));
    }

    #[test]
    fn boss_presence_uses_boss_art_and_no_player_names() {
        let mut gs = GameState::default();
        feed(
            &mut gs,
            "04:10:00",
            "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer",
        );
        feed(
            &mut gs,
            "04:10:51",
            "ECLIPTICA - now fighting boss: JimBringerPhase2(Clone) on phase: 0",
        );
        feed(
            &mut gs,
            "04:10:52",
            "ownership of JimBringerPhase2 transferred to Alice",
        );
        feed(&mut gs, "04:10:53", "Dealing 21059 STRIKE damage");
        let now = feed(&mut gs, "04:10:53", "Local controller dead, switching off.");
        let a = activity(&gs, now, 1_800_000_000).unwrap();
        assert!(
            a.contains("\"details\":\"Fighting Jim C. Bringer (P2)\""),
            "{a}"
        );
        assert!(
            a.contains("\"state\":\"Dead | 10.5k DPS | 21.1k damage | 1 death\""),
            "{a}"
        );
        assert!(a.contains("\"large_image\":\"boss_jimbringer\""));
        assert!(!a.contains("Alice"));
        assert!(!a.contains("Join in VRChat"));
        assert!(!a.contains("secrets"));
        assert!(a.contains(
            "\"buttons\":[{\"label\":\"Get EclipticaHUD\",\"url\":\"https://github.com/RealWhyKnot/EclipticaHUD/releases/latest\"}]"
        ));
    }

    #[test]
    fn lobby_presence_has_no_timer() {
        let mut gs = GameState::default();
        let now = feed(&mut gs, "04:12:08", "ECLIPTICA - now in lobby");
        let a = activity(&gs, now, 1_800_000_000).unwrap();
        assert_eq!(
            a,
            "{\"type\":5,\"details\":\"In the lobby\",\"assets\":{\"large_image\":\"logo\",\"large_text\":\"Ecliptica\",\"large_url\":\"https://github.com/RealWhyKnot/EclipticaHUD/releases/latest\"},\"buttons\":[{\"label\":\"Get EclipticaHUD\",\"url\":\"https://github.com/RealWhyKnot/EclipticaHUD/releases/latest\"}]}"
        );
    }
}
