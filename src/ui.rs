use crate::logwatch::LogWatch;
use crate::parse::GameState;
use crate::render::{Frame, Renderer};
use crate::vr::VrOverlay;
use std::path::PathBuf;
use std::time::Instant;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Dwm::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT, VK_ESCAPE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const TIMER_ID: usize = 1;
const FLASH_MS: f32 = 3000.0;
const WM_MOUSELEAVE: u32 = 0x02a3;

struct App {
    gs: GameState,
    watch: LogWatch,
    renderer: Renderer,
    vr: VrOverlay,
    rgba: Vec<u8>,
    last_ts: u64,
    last_ts_at: Instant,
    hover_close: bool,
    pressed_close: bool,
    tracking: bool,
    flash_at: Option<Instant>,
    last_target_since: u64,
    progress_shown: f32,
    dps_peak: u64,
    dps_boss: Option<String>,
    dps_frac_shown: f32,
    timer_ms: u32,
}

fn approach(cur: &mut f32, target: f32) -> bool {
    let d = target - *cur;
    if d.abs() < 0.002 {
        *cur = target;
        false
    } else {
        *cur += d * 0.18;
        true
    }
}

impl App {
    fn now(&self) -> u64 {
        self.last_ts + self.last_ts_at.elapsed().as_secs()
    }

    fn flash_t(&self) -> f32 {
        match self.flash_at {
            Some(t) => (t.elapsed().as_millis() as f32 / FLASH_MS).min(1.0),
            None => 1.0,
        }
    }

    fn repaint(&mut self, hwnd: HWND) {
        let frame = Frame {
            now: self.now(),
            flash_t: self.flash_t(),
            hover_close: self.hover_close,
            pressed_close: self.pressed_close,
            vr: self.vr.status(),
            log_ok: self.watch.path.is_some(),
            progress_shown: self.progress_shown,
            dps_frac_shown: self.dps_frac_shown,
        };
        self.renderer.draw(&mut self.gs, &frame);
        unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
        self.renderer.rgba(&mut self.rgba);
        self.vr.submit(&self.rgba, self.renderer.width as u32, self.renderer.height as u32, true);
    }

    fn tick(&mut self, hwnd: HWND) {
        let gs = &mut self.gs;
        let mut newest = 0u64;
        self.watch.poll(|line| {
            if let Some(ts) = gs.feed(line) {
                newest = newest.max(ts);
            }
        });
        if newest > self.last_ts {
            self.last_ts = newest;
            self.last_ts_at = Instant::now();
        }
        let now = self.now();
        if self.gs.target_since != self.last_target_since {
            self.last_target_since = self.gs.target_since;
            if self.gs.target.is_some() {
                self.flash_at = Some(Instant::now());
            }
        }
        if self.gs.boss != self.dps_boss {
            self.dps_boss = self.gs.boss.clone();
            self.dps_peak = 0;
        }
        let rolling = self.gs.rolling_dps(now);
        self.dps_peak = self.dps_peak.max(rolling);
        let dps_target = if self.gs.boss.is_some() && self.dps_peak > 0 {
            rolling as f32 / self.dps_peak as f32
        } else {
            0.0
        };
        let mut anim = approach(&mut self.progress_shown, self.gs.progress);
        anim |= approach(&mut self.dps_frac_shown, dps_target);
        anim |= self.flash_at.is_some() && self.flash_t() < 1.0;
        let redraw = self.gs.changed || anim || self.gs.boss.is_some();
        self.gs.changed = false;
        if redraw {
            self.repaint(hwnd);
        } else {
            self.vr.submit(&self.rgba, self.renderer.width as u32, self.renderer.height as u32, false);
        }
        let want: u32 = if anim { 33 } else { 1000 };
        if want != self.timer_ms {
            self.timer_ms = want;
            unsafe { SetTimer(hwnd, TIMER_ID, want, None) };
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn pos_file() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(base).join("EclipticaHUD").join("pos.txt"))
}

fn load_pos() -> Option<(i32, i32)> {
    let text = std::fs::read_to_string(pos_file()?).ok()?;
    let mut it = text.split_whitespace().map(|v| v.parse().ok());
    Some((it.next()??, it.next()??))
}

fn save_pos(hwnd: HWND) {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    if unsafe { GetWindowRect(hwnd, &mut r) } == 0 {
        return;
    }
    if let Some(path) = pos_file() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, format!("{} {}", r.left, r.top));
    }
}

fn default_pos(w: i32, h: i32) -> (i32, i32) {
    let mut work = RECT { left: 0, top: 0, right: 1920, bottom: 1080 };
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            &mut work as *mut RECT as *mut core::ffi::c_void,
            0,
        );
    }
    (work.right - w - 24, work.bottom - h - 24)
}

unsafe fn app_mut(hwnd: HWND) -> Option<&'static mut App> {
    let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
    p.as_mut()
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let cs = lp as *const CREATESTRUCTW;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_TIMER if wp == TIMER_ID => {
            if let Some(app) = app_mut(hwnd) {
                app.tick(hwnd);
            }
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            if let Some(app) = app_mut(hwnd) {
                BitBlt(hdc, 0, 0, app.renderer.width, app.renderer.height, app.renderer.dc, 0, 0, SRCCOPY);
            }
            EndPaint(hwnd, &ps);
            0
        }
        WM_NCHITTEST => {
            let x = (lp & 0xffff) as i16 as i32;
            let y = ((lp >> 16) & 0xffff) as i16 as i32;
            let mut pt = POINT { x, y };
            ScreenToClient(hwnd, &mut pt);
            if let Some(app) = app_mut(hwnd) {
                if app.renderer.close_hit(pt.x, pt.y) {
                    return HTCLIENT as LRESULT;
                }
            }
            HTCAPTION as LRESULT
        }
        WM_MOUSEMOVE => {
            let x = (lp & 0xffff) as i16 as i32;
            let y = ((lp >> 16) & 0xffff) as i16 as i32;
            if let Some(app) = app_mut(hwnd) {
                let over = app.renderer.close_hit(x, y);
                if over != app.hover_close {
                    app.hover_close = over;
                    app.repaint(hwnd);
                }
                if over && !app.tracking {
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    if TrackMouseEvent(&mut tme) != 0 {
                        app.tracking = true;
                    }
                }
            }
            0
        }
        WM_MOUSELEAVE | WM_NCMOUSEMOVE => {
            if let Some(app) = app_mut(hwnd) {
                if msg == WM_MOUSELEAVE {
                    app.tracking = false;
                }
                if app.hover_close {
                    app.hover_close = false;
                    app.repaint(hwnd);
                }
            }
            if msg == WM_NCMOUSEMOVE {
                DefWindowProcW(hwnd, msg, wp, lp)
            } else {
                0
            }
        }
        WM_LBUTTONDOWN => {
            let x = (lp & 0xffff) as i16 as i32;
            let y = ((lp >> 16) & 0xffff) as i16 as i32;
            if let Some(app) = app_mut(hwnd) {
                if app.renderer.close_hit(x, y) {
                    app.pressed_close = true;
                    SetCapture(hwnd);
                    app.repaint(hwnd);
                }
            }
            0
        }
        WM_LBUTTONUP => {
            let x = (lp & 0xffff) as i16 as i32;
            let y = ((lp >> 16) & 0xffff) as i16 as i32;
            ReleaseCapture();
            if let Some(app) = app_mut(hwnd) {
                if app.pressed_close {
                    app.pressed_close = false;
                    let inside = app.renderer.close_hit(x, y);
                    app.repaint(hwnd);
                    if inside {
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                    }
                }
            }
            0
        }
        WM_SETCURSOR if (lp & 0xffff) as u32 == HTCLIENT => {
            SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_HAND));
            1
        }
        WM_KEYDOWN if wp == VK_ESCAPE as usize => {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
            0
        }
        WM_EXITSIZEMOVE => {
            save_pos(hwnd);
            0
        }
        WM_DPICHANGED => {
            if let Some(app) = app_mut(hwnd) {
                app.renderer = Renderer::new((wp & 0xffff) as u32);
                let r = &*(lp as *const RECT);
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    r.left,
                    r.top,
                    app.renderer.width,
                    app.renderer.height,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            0
        }
        WM_DESTROY => {
            save_pos(hwnd);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

pub fn run() {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let dpi = GetDpiForSystem();
        let renderer = Renderer::new(dpi);
        let (w, h) = (renderer.width, renderer.height);
        let mut app = App {
            gs: GameState::default(),
            watch: LogWatch::new(),
            renderer,
            vr: VrOverlay::new(),
            rgba: Vec::new(),
            last_ts: 0,
            last_ts_at: Instant::now(),
            hover_close: false,
            pressed_close: false,
            tracking: false,
            flash_at: None,
            last_target_since: 0,
            progress_shown: 0.0,
            dps_peak: 0,
            dps_boss: None,
            dps_frac_shown: 0.0,
            timer_ms: 1000,
        };

        let module = GetModuleHandleW(std::ptr::null());
        let class_name = wide("EclipticaHUD");
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = module;
        wc.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
        wc.lpszClassName = class_name.as_ptr();
        RegisterClassW(&wc);

        let (x, y) = load_pos().unwrap_or_else(|| default_pos(w, h));
        let title = wide("Ecliptica HUD");
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_APPWINDOW,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            x,
            y,
            w,
            h,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            module,
            &mut app as *mut App as *mut core::ffi::c_void,
        );

        let dark: i32 = 1;
        DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE as u32, &dark as *const i32 as *const core::ffi::c_void, 4);
        let round: i32 = DWMWCP_ROUND as i32;
        DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE as u32, &round as *const i32 as *const core::ffi::c_void, 4);

        app.gs.changed = true;
        app.tick(hwnd);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        SetTimer(hwnd, TIMER_ID, 1000, None);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
