use super::*;

const REPLY_WAIT: Duration = Duration::from_secs(5);

pub(super) const DIAG_CLIP: usize = 600;

fn frame(op: u32, body: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body.as_bytes());
    out
}

pub(super) fn set_activity_cmd(pid: u32, nonce: u64, activity: Option<&str>) -> String {
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

pub(super) struct Conn {
    pipe: File,
    nonce: u64,
    last_reply: String,
}

impl Conn {
    pub(super) fn open() -> Option<Conn> {
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

    pub(super) fn command(&mut self, body: impl FnOnce(u64) -> String) -> io::Result<()> {
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

    pub(super) fn pump(&mut self) -> io::Result<Vec<String>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    const LOC: &str = "wrld_0fb88df3-2057-4c2f-8e06-e948864378fd:87887~hidden(usr_4e64b21b-fbd0-4c12-8b55-c8c500b517b1)~region(use)";

    fn feed(gs: &mut GameState, t: &str, msg: &str) -> u64 {
        gs.feed(&format!("2026.09.14 {t} Debug      -  {msg}"))
            .unwrap()
    }

    #[test]
    fn frames_are_little_endian() {
        let f = frame(1, "{}");
        assert_eq!(&f[..8], &[1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(&f[8..], b"{}");
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
}
