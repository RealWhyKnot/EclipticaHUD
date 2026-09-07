use crate::logwatch::LogWatch;
use crate::parse::GameState;
use crate::render::Renderer;
use crate::vr::VrOverlay;
use std::path::PathBuf;
use std::time::Instant;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Dwm::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const TIMER_ID: usize = 1;
const FLASH_SECS: u64 = 3;

struct App {
    gs: GameState,
    watch: LogWatch,
    renderer: Renderer,
    vr: VrOverlay,
    rgba: Vec<u8>,
    last_ts: u64,
    last_ts_at: Instant,
}

impl App {
    fn now(&self) -> u64 {
        self.last_ts + self.last_ts_at.elapsed().as_secs()
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
        let flash = self.gs.target.is_some() && now.saturating_sub(self.gs.target_since) < FLASH_SECS;
        let redraw = self.gs.changed || self.gs.boss.is_some() || flash;
        self.gs.changed = false;
        if redraw {
            self.renderer.draw(&mut self.gs, now, flash);
            unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
            self.renderer.rgba(&mut self.rgba);
        }
        self.vr.submit(&self.rgba, self.renderer.width as u32, self.renderer.height as u32, redraw);
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
        WM_LBUTTONDOWN => {
            let x = (lp & 0xffff) as i16 as i32;
            let y = ((lp >> 16) & 0xffff) as i16 as i32;
            if let Some(app) = app_mut(hwnd) {
                if app.renderer.close_hit(x, y) {
                    PostMessageW(hwnd, WM_CLOSE, 0, 0);
                }
            }
            0
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
