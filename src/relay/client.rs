use super::proto::{Msg, Room, MAX_SNAP_LEN};
use super::{rand, Backoff, Hub, Link, Snap, Throttle, Url, Want};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU16, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows_sys::Win32::Networking::WinHttp::*;

const TIMEOUT_MS: i32 = 10_000;
const CLOSE_WAIT: Duration = Duration::from_secs(2);
const CLOSE_TIMEOUT_S: u32 = 1;
const NORMAL: u16 = 1000;
const GOING_AWAY: u16 = 1001;
const POLICY: u16 = 1008;
pub(super) const ROOM_FULL: u16 = 4000;
pub(super) const MEMBER_TAKEN: u16 = 4001;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

struct Handle(*mut c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

pub(super) struct Ws(AtomicPtr<c_void>);

impl Ws {
    fn send(&self, b: &[u8]) -> bool {
        let h = self.0.load(Ordering::Acquire);
        !h.is_null()
            && unsafe {
                WinHttpWebSocketSend(
                    h,
                    WINHTTP_WEB_SOCKET_BINARY_MESSAGE_BUFFER_TYPE,
                    b.as_ptr() as *const c_void,
                    b.len() as u32,
                )
            } == 0
    }

    fn receive(&self, buf: &mut [u8]) -> Option<(usize, WINHTTP_WEB_SOCKET_BUFFER_TYPE)> {
        let h = self.0.load(Ordering::Acquire);
        if h.is_null() {
            return None;
        }
        let mut read = 0u32;
        let mut kind: WINHTTP_WEB_SOCKET_BUFFER_TYPE = 0;
        let err = unsafe {
            WinHttpWebSocketReceive(
                h,
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as u32,
                &mut read,
                &mut kind,
            )
        };
        (err == 0).then_some((read as usize, kind))
    }

    fn shutdown(&self, code: u16) {
        let h = self.0.load(Ordering::Acquire);
        if !h.is_null() {
            unsafe { WinHttpWebSocketShutdown(h, code, std::ptr::null(), 0) };
        }
    }

    fn abort(&self, code: u16) {
        let h = self.0.load(Ordering::Acquire);
        if !h.is_null() {
            unsafe { WinHttpWebSocketClose(h, code, std::ptr::null(), 0) };
        }
    }

    fn close_status(&self) -> u16 {
        let h = self.0.load(Ordering::Acquire);
        let mut status = 0u16;
        let mut reason = [0u8; 123];
        let mut len = 0u32;
        let ok = !h.is_null()
            && unsafe {
                WinHttpWebSocketQueryCloseStatus(
                    h,
                    &mut status,
                    reason.as_mut_ptr() as *mut c_void,
                    reason.len() as u32,
                    &mut len,
                )
            } == 0;
        if ok {
            status
        } else {
            0
        }
    }
}

impl Drop for Ws {
    fn drop(&mut self) {
        let h = self.0.swap(std::ptr::null_mut(), Ordering::AcqRel);
        if !h.is_null() {
            unsafe { WinHttpCloseHandle(h) };
        }
    }
}

struct Conn {
    ws: Arc<Ws>,
    _connect: Handle,
    _session: Handle,
}

impl Conn {
    fn open(url: &Url) -> Option<Conn> {
        let agent = wide(&format!("EclipticaHUD/{}", crate::update::VERSION));
        unsafe {
            let mut raw = WinHttpOpen(
                agent.as_ptr(),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                std::ptr::null(),
                std::ptr::null(),
                0,
            );
            if raw.is_null() {
                raw = WinHttpOpen(
                    agent.as_ptr(),
                    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                );
            }
            let session = Handle(raw);
            if session.0.is_null()
                || WinHttpSetTimeouts(session.0, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS)
                    == 0
            {
                return None;
            }
            let host = wide(&url.host);
            let connect = Handle(WinHttpConnect(session.0, host.as_ptr(), url.port, 0));
            if connect.0.is_null() {
                return None;
            }
            let verb = wide("GET");
            let path = wide(&url.path);
            let flags = if url.secure { WINHTTP_FLAG_SECURE } else { 0 };
            let req = Handle(WinHttpOpenRequest(
                connect.0,
                verb.as_ptr(),
                path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                flags,
            ));
            if req.0.is_null()
                || WinHttpSetOption(
                    req.0,
                    WINHTTP_OPTION_UPGRADE_TO_WEB_SOCKET,
                    std::ptr::null(),
                    0,
                ) == 0
                || WinHttpSendRequest(req.0, std::ptr::null(), 0, std::ptr::null(), 0, 0, 0) == 0
                || WinHttpReceiveResponse(req.0, std::ptr::null_mut()) == 0
            {
                return None;
            }
            let mut code = 0u32;
            let mut len = 4u32;
            if WinHttpQueryHeaders(
                req.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                std::ptr::null(),
                &mut code as *mut u32 as *mut c_void,
                &mut len,
                std::ptr::null_mut(),
            ) == 0
                || code != 101
            {
                return None;
            }
            let ws = WinHttpWebSocketCompleteUpgrade(req.0, 0);
            drop(req);
            if ws.is_null() {
                return None;
            }
            let close_secs = CLOSE_TIMEOUT_S;
            WinHttpSetOption(
                ws,
                WINHTTP_OPTION_WEB_SOCKET_CLOSE_TIMEOUT,
                &close_secs as *const u32 as *const c_void,
                4,
            );
            Some(Conn {
                ws: Arc::new(Ws(AtomicPtr::new(ws))),
                _connect: connect,
                _session: session,
            })
        }
    }
}

struct Rx {
    end: AtomicBool,
    done: AtomicBool,
    code: AtomicU16,
    peer: AtomicU16,
}

impl Rx {
    fn raise(&self, hub: &Hub, flag: &AtomicBool) {
        let _guard = hub.lock();
        flag.store(true, Ordering::Release);
        hub.cv.notify_all();
    }
}

enum Outcome {
    Failed,
    Lost,
    Refused,
    Changed,
}

fn current(hub: &Hub) -> Option<Want> {
    let mut s = hub.lock();
    if s.want.is_none() {
        if s.link == Link::Connected {
            s.end_session();
        }
        s.running = false;
        s.link = Link::Off;
        hub.cv.notify_all();
    }
    s.want.clone()
}

pub(super) fn run(hub: &'static Hub) {
    let mut backoff = Backoff::default();
    while let Some(want) = current(hub) {
        hub.set_link(Link::Connecting);
        let started = Instant::now();
        let outcome = match Url::parse(&want.url).and_then(|u| Conn::open(&u)) {
            Some(conn) => session(hub, conn, &want),
            None => Outcome::Failed,
        };
        let delay = match outcome {
            Outcome::Changed => {
                backoff.reset();
                continue;
            }
            Outcome::Refused => backoff.max_ms(rand()),
            Outcome::Lost => {
                backoff.after_session(started.elapsed());
                backoff.next_ms(rand())
            }
            Outcome::Failed => backoff.next_ms(rand()),
        };
        hub.set_link(Link::Retrying);
        let deadline = Instant::now() + Duration::from_millis(delay);
        let mut s = hub.lock();
        while s.want.as_ref() == Some(&want) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            s = hub
                .cv
                .wait_timeout(s, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        if s.want.as_ref() != Some(&want) {
            backoff.reset();
        }
    }
}

fn session(hub: &'static Hub, conn: Conn, want: &Want) -> Outcome {
    let room = want.room();
    let join = Msg::Join {
        room,
        member: want.member(),
        claim: want.claim(),
    };
    if hub.lock().want.as_ref() != Some(want) {
        return Outcome::Changed;
    }
    if !conn.ws.send(&join.encode()) {
        return Outcome::Failed;
    }
    let id = hub.set_link(Link::Connected);
    let rx = Arc::new(Rx {
        end: AtomicBool::new(false),
        done: AtomicBool::new(false),
        code: AtomicU16::new(NORMAL),
        peer: AtomicU16::new(0),
    });
    let spawned = {
        let (ws, rx) = (conn.ws.clone(), rx.clone());
        std::thread::Builder::new()
            .name("ehud-relay-rx".into())
            .spawn(move || receive(hub, &ws, &rx, id, room))
    };
    let Ok(reader) = spawned else {
        return Outcome::Failed;
    };
    let epoch = Instant::now();
    let mut throttle = Throttle::default();
    let outcome = loop {
        let s = hub.lock();
        if s.want.as_ref() != Some(want) {
            if s.want.is_none() {
                rx.code.store(GOING_AWAY, Ordering::Relaxed);
            }
            break Outcome::Changed;
        }
        if rx.end.load(Ordering::Acquire) {
            break Outcome::Lost;
        }
        let now = epoch.elapsed().as_millis() as u64;
        let dmg = s.dmg;
        if let Some(msg) = throttle.due(now, dmg) {
            drop(s);
            if !conn.ws.send(&msg.encode()) {
                break Outcome::Lost;
            }
            continue;
        }
        let wait = Duration::from_millis(throttle.wait_ms(now, dmg));
        drop(hub.cv.wait_timeout(s, wait));
    };
    let code = rx.code.load(Ordering::Relaxed);
    conn.ws.shutdown(code);
    if !wait_done(hub, &rx) {
        conn.ws.abort(code);
    }
    let _ = reader.join();
    match (outcome, rx.peer.load(Ordering::Acquire)) {
        (Outcome::Lost, ROOM_FULL | MEMBER_TAKEN) => Outcome::Refused,
        (other, _) => other,
    }
}

fn wait_done(hub: &Hub, rx: &Rx) -> bool {
    let deadline = Instant::now() + CLOSE_WAIT;
    let mut s = hub.lock();
    while !rx.done.load(Ordering::Acquire) {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return false;
        }
        s = hub
            .cv
            .wait_timeout(s, left.min(Duration::from_millis(50)))
            .unwrap_or_else(|e| e.into_inner())
            .0;
    }
    true
}

fn receive(hub: &Hub, ws: &Ws, rx: &Rx, id: u64, room: Room) {
    let mut buf = vec![0u8; MAX_SNAP_LEN + 1];
    let mut msg: Vec<u8> = Vec::with_capacity(MAX_SNAP_LEN);
    let mut bad = false;
    while let Some((n, kind)) = ws.receive(&mut buf) {
        if kind == WINHTTP_WEB_SOCKET_CLOSE_BUFFER_TYPE {
            rx.peer.store(ws.close_status(), Ordering::Release);
            break;
        }
        if bad {
            continue;
        }
        let ok = match kind {
            WINHTTP_WEB_SOCKET_BINARY_FRAGMENT_BUFFER_TYPE
            | WINHTTP_WEB_SOCKET_BINARY_MESSAGE_BUFFER_TYPE => {
                msg.extend_from_slice(&buf[..n]);
                if msg.len() > MAX_SNAP_LEN {
                    false
                } else if kind == WINHTTP_WEB_SOCKET_BINARY_FRAGMENT_BUFFER_TYPE {
                    true
                } else if let Some(Msg::Snap(entries)) = Msg::decode(&msg) {
                    hub.push_snap(
                        id,
                        Snap {
                            at: Instant::now(),
                            room,
                            entries,
                        },
                    );
                    msg.clear();
                    true
                } else {
                    false
                }
            }
            _ => false,
        };
        if !ok {
            bad = true;
            msg = Vec::new();
            rx.code.store(POLICY, Ordering::Relaxed);
            rx.raise(hub, &rx.end);
        }
    }
    rx.raise(hub, &rx.end);
    rx.raise(hub, &rx.done);
}
