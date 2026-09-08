use crate::app::App;
use crate::render::Renderer;
use std::path::PathBuf;
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
const WM_MOUSELEAVE: u32 = 0x02a3;

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
        SystemParametersInfoW(
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

unsafe fn repaint(app: &mut App, hwnd: HWND) {
    app.render();
    InvalidateRect(hwnd, std::ptr::null(), 0);
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
                let tick = app.tick();
                if tick.redraw {
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                }
                if let Some(ms) = tick.timer_ms {
                    SetTimer(hwnd, TIMER_ID, ms, None);
                }
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
                    repaint(app, hwnd);
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
                    repaint(app, hwnd);
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
                    repaint(app, hwnd);
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
                    repaint(app, hwnd);
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
                repaint(app, hwnd);
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
        let mut app = App::new(Renderer::new(dpi));
        let (w, h) = (app.renderer.width, app.renderer.height);

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

        app.tick();
        app.render();
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        SetTimer(hwnd, TIMER_ID, 1000, None);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
