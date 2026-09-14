use crate::app::App;
use crate::app::WINDOW_STEPS;
use crate::hud::render::{Hit, LogHit, MAX_ALPHA, MAX_SCALE, MIN_ALPHA, MIN_SCALE};
use crate::update;
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

fn data_file(name: &str) -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(base).join("EclipticaHUD").join(name))
}

fn write_data(name: &str, text: String) {
    if let Some(path) = data_file(name) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, text);
    }
}

fn sound_on_from(text: &str) -> bool {
    text.trim() != "0"
}

fn load_sound() -> bool {
    data_file("sound.txt")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .is_none_or(|t| sound_on_from(&t))
}

fn save_sound(on: bool) {
    write_data("sound.txt", if on { "1" } else { "0" }.to_string());
}

fn load_topmost() -> bool {
    data_file("top.txt")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .is_none_or(|t| sound_on_from(&t))
}

fn save_topmost(on: bool) {
    write_data("top.txt", if on { "1" } else { "0" }.to_string());
}

fn discord_on_from(text: &str) -> bool {
    text.trim() == "1"
}

fn scale_from(text: &str) -> f32 {
    text.trim()
        .parse::<u32>()
        .map_or(1.0, |p| (p as f32 / 100.0).clamp(MIN_SCALE, MAX_SCALE))
}

fn alpha_from(text: &str) -> u8 {
    text.trim().parse::<u32>().map_or(MAX_ALPHA, |p| {
        p.clamp(MIN_ALPHA as u32, MAX_ALPHA as u32) as u8
    })
}

fn load_text(name: &str) -> Option<String> {
    data_file(name).and_then(|p| std::fs::read_to_string(p).ok())
}

fn save_scale(scale: f32) {
    write_data("scale.txt", ((scale * 100.0).round() as i32).to_string());
}

fn window_from(text: &str) -> u64 {
    text.trim()
        .parse::<u64>()
        .map_or(crate::game::state::DEFAULT_WINDOW, |w| {
            w.clamp(WINDOW_STEPS[0], WINDOW_STEPS[WINDOW_STEPS.len() - 1])
        })
}

fn save_window(window: u64) {
    write_data("window.txt", window.to_string());
}

fn save_alpha(alpha: u8) {
    write_data("alpha.txt", alpha.to_string());
}

unsafe fn apply_alpha(app: &App, main: HWND) {
    SetLayeredWindowAttributes(main, 0, app.alpha_byte(), LWA_ALPHA);
}

unsafe fn apply_size(app: &mut App, main: HWND) {
    let flags = SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE;
    SetWindowPos(
        main,
        std::ptr::null_mut(),
        0,
        0,
        app.renderer.width,
        app.renderer.height,
        flags,
    );
    if !app.log.hwnd.is_null() {
        SetWindowPos(
            app.log.hwnd,
            std::ptr::null_mut(),
            0,
            0,
            app.log.renderer.width,
            app.log.renderer.height,
            flags,
        );
    }
    repaint(app, main);
    repaint_log(app);
}

fn load_discord() -> bool {
    data_file("discord.txt")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .is_some_and(|t| discord_on_from(&t))
}

fn save_discord(on: bool) {
    write_data("discord.txt", if on { "1" } else { "0" }.to_string());
}

unsafe fn sync_topmost(main: HWND) {
    let Some(app) = app_mut(main) else {
        return;
    };
    let after = if app.topmost {
        HWND_TOPMOST
    } else {
        HWND_NOTOPMOST
    };
    let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
    SetWindowPos(main, after, 0, 0, 0, 0, flags);
    if !app.log.hwnd.is_null() {
        SetWindowPos(app.log.hwnd, after, 0, 0, 0, 0, flags);
    }
}

fn parse_pos(text: &str) -> Option<(i32, i32, bool)> {
    let mut it = text.split_whitespace();
    let x = it.next()?.parse().ok()?;
    let y = it.next()?.parse().ok()?;
    let open = it.next().is_some_and(|v| v == "1");
    Some((x, y, open))
}

fn load_pos(name: &str) -> Option<(i32, i32, bool)> {
    parse_pos(&std::fs::read_to_string(data_file(name)?).ok()?)
}

fn window_pos(hwnd: HWND) -> Option<(i32, i32)> {
    let mut r = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    (unsafe { GetWindowRect(hwnd, &mut r) } != 0).then_some((r.left, r.top))
}

fn save_pos(hwnd: HWND) {
    if let Some((x, y)) = window_pos(hwnd) {
        write_data("pos.txt", format!("{x} {y}"));
    }
}

fn save_log_pos(hwnd: HWND, open: bool) {
    if let Some((x, y)) = window_pos(hwnd) {
        write_data("logpos.txt", format!("{x} {y} {}", u8::from(open)));
    }
}

fn work_area() -> RECT {
    let mut work = RECT {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            &mut work as *mut RECT as *mut core::ffi::c_void,
            0,
        );
    }
    work
}

fn default_pos(w: i32, h: i32) -> (i32, i32) {
    let work = work_area();
    (work.right - w - 24, work.bottom - h - 24)
}

fn default_log_pos(main: HWND, w: i32) -> (i32, i32) {
    let work = work_area();
    let (mx, my) = window_pos(main).unwrap_or((work.right, work.bottom));
    let mut mr = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe { GetWindowRect(main, &mut mr) };
    let x = if mx - w - 12 >= work.left {
        mx - w - 12
    } else {
        mr.right + 12
    };
    (x, my)
}

unsafe fn app_mut(hwnd: HWND) -> Option<&'static mut App> {
    let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
    p.as_mut()
}

unsafe fn repaint(app: &mut App, hwnd: HWND) {
    app.render();
    InvalidateRect(hwnd, std::ptr::null(), 0);
}

unsafe fn repaint_log(app: &mut App) {
    if !app.log.hwnd.is_null() {
        app.render_log();
        InvalidateRect(app.log.hwnd, std::ptr::null(), 0);
    }
}

fn point(lp: LPARAM) -> (i32, i32) {
    (
        (lp & 0xffff) as i16 as i32,
        ((lp >> 16) & 0xffff) as i16 as i32,
    )
}

unsafe fn track_leave(hwnd: HWND, tracking: &mut bool) {
    if *tracking {
        return;
    }
    let mut tme = TRACKMOUSEEVENT {
        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_LEAVE,
        hwndTrack: hwnd,
        dwHoverTime: 0,
    };
    if TrackMouseEvent(&mut tme) != 0 {
        *tracking = true;
    }
}

unsafe fn apply_dwm(hwnd: HWND) {
    let dark: i32 = 1;
    DwmSetWindowAttribute(
        hwnd,
        DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
        &dark as *const i32 as *const core::ffi::c_void,
        4,
    );
    let round: i32 = DWMWCP_ROUND;
    DwmSetWindowAttribute(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE as u32,
        &round as *const i32 as *const core::ffi::c_void,
        4,
    );
}

unsafe fn create_log_window(main: HWND) {
    let app_ptr = GetWindowLongPtrW(main, GWLP_USERDATA) as *mut App;
    let Some(app) = app_ptr.as_mut() else {
        return;
    };
    let (w, h) = (app.log.renderer.width, app.log.renderer.height);
    let (x, y) = load_pos("logpos.txt")
        .map(|(x, y, _)| (x, y))
        .unwrap_or_else(|| default_log_pos(main, w));
    let module = GetModuleHandleW(std::ptr::null());
    let class_name = wide("EclipticaHUDLog");
    let title = wide("Ecliptica HUD log");
    let hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_POPUP,
        x,
        y,
        w,
        h,
        main,
        std::ptr::null_mut(),
        module,
        app_ptr as *mut core::ffi::c_void,
    );
    apply_dwm(hwnd);
    SetLayeredWindowAttributes(hwnd, 0, 0, LWA_ALPHA);
    if let Some(app) = app_ptr.as_mut() {
        app.log.hwnd = hwnd;
    }
}

unsafe fn sync_log(main: HWND) {
    let app_ptr = GetWindowLongPtrW(main, GWLP_USERDATA) as *mut App;
    let Some(app) = app_ptr.as_mut() else {
        return;
    };
    let want = app.log.visible;
    if want && app.log.hwnd.is_null() {
        create_log_window(main);
        sync_topmost(main);
    }
    let Some(app) = app_ptr.as_mut() else {
        return;
    };
    let hwnd = app.log.hwnd;
    if hwnd.is_null() {
        return;
    }
    let shown = IsWindowVisible(hwnd) != 0;
    if want && !shown {
        app.render_log();
        SetLayeredWindowAttributes(hwnd, 0, app.log.alpha, LWA_ALPHA);
        app.log.alpha_shown = app.log.alpha;
        ShowWindow(hwnd, SW_SHOW);
        save_log_pos(hwnd, true);
    } else if !want && shown {
        ShowWindow(hwnd, SW_HIDE);
        save_log_pos(hwnd, false);
    }
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
                if app.log.visible && !app.log.hwnd.is_null() {
                    if app.log.alpha != app.log.alpha_shown {
                        SetLayeredWindowAttributes(app.log.hwnd, 0, app.log.alpha, LWA_ALPHA);
                        app.log.alpha_shown = app.log.alpha;
                    }
                    if tick.redraw_log {
                        InvalidateRect(app.log.hwnd, std::ptr::null(), 0);
                    }
                }
                if let Some(ms) = tick.timer_ms {
                    SetTimer(hwnd, TIMER_ID, ms, None);
                }
                if tick.quit {
                    PostMessageW(hwnd, WM_CLOSE, 0, 0);
                }
            }
            sync_log(hwnd);
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            if let Some(app) = app_mut(hwnd) {
                BitBlt(
                    hdc,
                    0,
                    0,
                    app.renderer.width,
                    app.renderer.height,
                    app.renderer.dc,
                    0,
                    0,
                    SRCCOPY,
                );
            }
            EndPaint(hwnd, &ps);
            0
        }
        WM_NCHITTEST => {
            let (x, y) = point(lp);
            let mut pt = POINT { x, y };
            ScreenToClient(hwnd, &mut pt);
            if let Some(app) = app_mut(hwnd) {
                if app.enabled_hit(pt.x, pt.y).is_some() {
                    return HTCLIENT as LRESULT;
                }
            }
            HTCAPTION as LRESULT
        }
        WM_MOUSEMOVE => {
            let (x, y) = point(lp);
            if let Some(app) = app_mut(hwnd) {
                let over = app.enabled_hit(x, y);
                if app.set_hover(over) {
                    repaint(app, hwnd);
                    if over.is_some() {
                        SetTimer(hwnd, TIMER_ID, 16, None);
                        app.nudge_timer();
                    }
                }
                if over.is_some() {
                    track_leave(hwnd, &mut app.tracking);
                }
            }
            0
        }
        WM_MOUSELEAVE | WM_NCMOUSEMOVE => {
            if let Some(app) = app_mut(hwnd) {
                if msg == WM_MOUSELEAVE {
                    app.tracking = false;
                }
                if app.set_hover(None) {
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
            let (x, y) = point(lp);
            if let Some(app) = app_mut(hwnd) {
                if let Some(hit) = app.enabled_hit(x, y).filter(|h| h.clickable()) {
                    app.pressed = Some(hit);
                    SetCapture(hwnd);
                    repaint(app, hwnd);
                }
            }
            0
        }
        WM_LBUTTONUP => {
            let (x, y) = point(lp);
            let pressed = app_mut(hwnd).and_then(|app| app.pressed.take());
            ReleaseCapture();
            if let Some(app) = app_mut(hwnd) {
                if let Some(hit) = pressed {
                    let inside = app.enabled_hit(x, y) == Some(hit);
                    if inside {
                        match hit {
                            Hit::Close => {
                                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                            }
                            Hit::Update => {
                                update::spawn_install();
                                app.tick();
                            }
                            Hit::Target => {
                                app.activate(hit);
                                save_sound(app.sound_on);
                            }
                            Hit::Pin => {
                                app.activate(hit);
                                save_topmost(app.topmost);
                                sync_topmost(hwnd);
                            }
                            Hit::Discord => {
                                app.activate(hit);
                                save_discord(app.discord_on);
                            }
                            Hit::ScaleDown | Hit::ScaleUp => {
                                if app.activate(hit) {
                                    save_scale(app.scale);
                                    apply_size(app, hwnd);
                                }
                            }
                            Hit::WindowDown | Hit::WindowUp => {
                                if app.activate(hit) {
                                    save_window(app.gs.win());
                                }
                            }
                            Hit::AlphaDown | Hit::AlphaUp => {
                                if app.activate(hit) {
                                    save_alpha(app.alpha);
                                    apply_alpha(app, hwnd);
                                }
                            }
                            _ => {
                                app.activate(hit);
                            }
                        }
                    }
                    app.hover = app.enabled_hit(x, y);
                    repaint(app, hwnd);
                }
            }
            sync_log(hwnd);
            0
        }
        WM_CAPTURECHANGED => {
            if let Some(app) = app_mut(hwnd) {
                if app.pressed.take().is_some() {
                    repaint(app, hwnd);
                }
            }
            0
        }
        WM_SETCURSOR if (lp & 0xffff) as u32 == HTCLIENT => {
            let mut pt = POINT { x: 0, y: 0 };
            GetCursorPos(&mut pt);
            ScreenToClient(hwnd, &mut pt);
            let hand = app_mut(hwnd)
                .and_then(|app| app.enabled_hit(pt.x, pt.y))
                .is_some_and(|h| h.clickable());
            let cursor = if hand { IDC_HAND } else { IDC_ARROW };
            SetCursor(LoadCursorW(std::ptr::null_mut(), cursor));
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
                app.dpi = (wp & 0xffff) as u32;
                app.rescale();
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
        WM_CLOSE => {
            if let Some(app) = app_mut(hwnd) {
                if !app.log.hwnd.is_null() {
                    save_log_pos(app.log.hwnd, app.log.visible);
                }
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_DESTROY => {
            save_pos(hwnd);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe extern "system" fn log_wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let cs = lp as *const CREATESTRUCTW;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            if let Some(app) = app_mut(hwnd) {
                BitBlt(
                    hdc,
                    0,
                    0,
                    app.log.renderer.width,
                    app.log.renderer.height,
                    app.log.renderer.dc,
                    0,
                    0,
                    SRCCOPY,
                );
            }
            EndPaint(hwnd, &ps);
            0
        }
        WM_NCHITTEST => {
            let (x, y) = point(lp);
            let mut pt = POINT { x, y };
            ScreenToClient(hwnd, &mut pt);
            if let Some(app) = app_mut(hwnd) {
                let thumb = app.log_thumb();
                if app.log.renderer.log_hit_test(pt.x, pt.y, thumb).is_some() {
                    return HTCLIENT as LRESULT;
                }
            }
            HTCAPTION as LRESULT
        }
        WM_MOUSEMOVE => {
            let (x, y) = point(lp);
            if let Some(app) = app_mut(hwnd) {
                if app.log.drag.is_some() {
                    app.log_drag_to(y);
                    repaint_log(app);
                } else {
                    let thumb = app.log_thumb();
                    let over = app.log.renderer.log_hit_test(x, y, thumb);
                    if app.log_set_hover(over) {
                        repaint_log(app);
                        if over.is_some() {
                            SetTimer(GetWindow(hwnd, GW_OWNER), TIMER_ID, 16, None);
                            app.nudge_timer();
                        }
                    }
                    if over.is_some() {
                        track_leave(hwnd, &mut app.log.tracking);
                    }
                }
            }
            0
        }
        WM_MOUSELEAVE | WM_NCMOUSEMOVE => {
            if let Some(app) = app_mut(hwnd) {
                if msg == WM_MOUSELEAVE {
                    app.log.tracking = false;
                }
                if app.log.drag.is_none() && app.log_set_hover(None) {
                    repaint_log(app);
                }
            }
            if msg == WM_NCMOUSEMOVE {
                DefWindowProcW(hwnd, msg, wp, lp)
            } else {
                0
            }
        }
        WM_LBUTTONDOWN => {
            let (x, y) = point(lp);
            if let Some(app) = app_mut(hwnd) {
                let thumb = app.log_thumb();
                if let Some(hit) = app.log.renderer.log_hit_test(x, y, thumb) {
                    app.log_press(hit, y);
                    if hit == LogHit::Thumb {
                        SetCapture(hwnd);
                    }
                    repaint_log(app);
                }
            }
            if let Some(owner) = GetWindow(hwnd, GW_OWNER).as_mut() {
                sync_log(owner);
            }
            0
        }
        WM_LBUTTONUP | WM_CAPTURECHANGED => {
            let dragging = app_mut(hwnd).is_some_and(|app| {
                let was = app.log.drag.is_some();
                app.log_release();
                was
            });
            if msg == WM_LBUTTONUP {
                ReleaseCapture();
            }
            if dragging {
                if let Some(app) = app_mut(hwnd) {
                    repaint_log(app);
                }
            }
            0
        }
        WM_MOUSEWHEEL => {
            let delta = ((wp >> 16) & 0xffff) as u16 as i16 as i32;
            if let Some(app) = app_mut(hwnd) {
                app.log_wheel(delta);
                repaint_log(app);
                SetTimer(GetWindow(hwnd, GW_OWNER), TIMER_ID, 16, None);
            }
            0
        }
        WM_SETCURSOR if (lp & 0xffff) as u32 == HTCLIENT => {
            let mut pt = POINT { x: 0, y: 0 };
            GetCursorPos(&mut pt);
            ScreenToClient(hwnd, &mut pt);
            let cursor = match app_mut(hwnd) {
                Some(app) => match app.log.renderer.log_hit_test(pt.x, pt.y, app.log_thumb()) {
                    Some(LogHit::Thumb | LogHit::Track) => IDC_SIZENS,
                    Some(h) if h.clickable() => IDC_HAND,
                    _ => IDC_ARROW,
                },
                None => IDC_ARROW,
            };
            SetCursor(LoadCursorW(std::ptr::null_mut(), cursor));
            1
        }
        WM_KEYDOWN if wp == VK_ESCAPE as usize => {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
            0
        }
        WM_CLOSE => {
            if let Some(app) = app_mut(hwnd) {
                app.log_close();
            }
            if let Some(owner) = GetWindow(hwnd, GW_OWNER).as_mut() {
                sync_log(owner);
            }
            0
        }
        WM_EXITSIZEMOVE => {
            save_log_pos(hwnd, true);
            0
        }
        WM_DPICHANGED => {
            if let Some(app) = app_mut(hwnd) {
                let r = &*(lp as *const RECT);
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    r.left,
                    r.top,
                    app.log.renderer.width,
                    app.log.renderer.height,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                repaint_log(app);
            }
            0
        }
        WM_DESTROY => {
            if let Some(app) = app_mut(hwnd) {
                app.log.hwnd = std::ptr::null_mut();
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

pub fn run() {
    update::cleanup_old();
    update::spawn_check();
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let dpi = GetDpiForSystem();
        let mut app = App::new(dpi);
        app.sound_on = load_sound();
        app.topmost = load_topmost();
        app.set_scale(load_text("scale.txt").map_or(1.0, |t| scale_from(&t)));
        app.set_alpha(load_text("alpha.txt").map_or(MAX_ALPHA, |t| alpha_from(&t)));
        app.set_window(
            load_text("window.txt").map_or(crate::game::state::DEFAULT_WINDOW, |t| window_from(&t)),
        );
        app.backfill_history();
        if load_discord() {
            app.set_discord(true);
        }
        let (w, h) = (app.renderer.width, app.renderer.height);

        let module = GetModuleHandleW(std::ptr::null());
        let class_name = wide("EclipticaHUD");
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = module;
        wc.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
        wc.lpszClassName = class_name.as_ptr();
        RegisterClassW(&wc);
        let log_class = wide("EclipticaHUDLog");
        let mut lc: WNDCLASSW = std::mem::zeroed();
        lc.lpfnWndProc = Some(log_wndproc);
        lc.hInstance = module;
        lc.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
        lc.lpszClassName = log_class.as_ptr();
        RegisterClassW(&lc);

        let (x, y) = load_pos("pos.txt")
            .map(|(x, y, _)| (x, y))
            .unwrap_or_else(|| default_pos(w, h));
        let title = wide("Ecliptica HUD");
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_APPWINDOW | WS_EX_LAYERED,
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
        apply_dwm(hwnd);
        apply_alpha(&app, hwnd);

        app.tick();
        app.render();
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        sync_topmost(hwnd);
        SetTimer(hwnd, TIMER_ID, 1000, None);
        if load_pos("logpos.txt").is_some_and(|(_, _, open)| open) {
            app.log_open();
            sync_log(hwnd);
        }

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    if update::restart_pending() {
        update::relaunch();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_default_and_parse() {
        assert!(!sound_on_from("0"));
        assert!(!sound_on_from("0\n"));
        assert!(sound_on_from("1"));
        assert!(sound_on_from(""));
        assert!(sound_on_from("garbage"));
    }

    #[test]
    fn scale_and_alpha_parse() {
        assert_eq!(scale_from("150"), 1.5);
        assert_eq!(scale_from("garbage"), 1.0);
        assert_eq!(scale_from("10"), MIN_SCALE);
        assert_eq!(scale_from("900"), MAX_SCALE);
        assert_eq!(alpha_from("70\n"), 70);
        assert_eq!(alpha_from(""), MAX_ALPHA);
        assert_eq!(alpha_from("5"), MIN_ALPHA);
        assert_eq!(alpha_from("300"), MAX_ALPHA);
        assert_eq!(window_from("15"), 15);
        assert_eq!(window_from("x"), 10);
        assert_eq!(window_from("1"), 3);
        assert_eq!(window_from("99"), 30);
    }

    #[test]
    fn discord_defaults_off() {
        assert!(!discord_on_from(""));
        assert!(!discord_on_from("0"));
        assert!(!discord_on_from("garbage"));
        assert!(discord_on_from(
            "1
"
        ));
    }

    #[test]
    fn pos_parse() {
        assert_eq!(parse_pos("10 20"), Some((10, 20, false)));
        assert_eq!(parse_pos("10 20 1"), Some((10, 20, true)));
        assert_eq!(parse_pos("10 20 0\n"), Some((10, 20, false)));
        assert_eq!(parse_pos("-5 7 1"), Some((-5, 7, true)));
        assert_eq!(parse_pos("x"), None);
        assert_eq!(parse_pos(""), None);
    }
}
