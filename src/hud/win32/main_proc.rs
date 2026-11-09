use super::{
    BeginPaint, BitBlt, CREATESTRUCTW, DefWindowProcW, EndPaint, GWLP_USERDATA, GetCursorPos,
    HTCAPTION, HTCLIENT, HWND, Hit, IDC_ARROW, IDC_HAND, InvalidateRect, LPARAM, LRESULT,
    LWA_ALPHA, LoadCursorW, MOVING, Ordering, PAINTSTRUCT, POINT, PostMessageW, PostQuitMessage,
    ReleaseCapture, SRCCOPY, ScreenToClient, SetCapture, SetCursor, SetLayeredWindowAttributes,
    SetTimer, SetWindowLongPtrW, TIMER_ID, VK_ESCAPE, WM_CAPTURECHANGED, WM_CLOSE, WM_DESTROY,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSELEAVE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCCREATE, WM_NCHITTEST,
    WM_NCMOUSEMOVE, WM_PAINT, WM_SETCURSOR, WM_TIMER, WPARAM, app_mut, apply_alpha, apply_size,
    dpi_changed, drag_window, fit_windows, keep_all_on_screen, point, repaint, save_alpha,
    save_board, save_discord, save_log_pos, save_party_pos, save_pos, save_scale, save_skipped,
    save_sound, save_topmost, save_usage, save_window, sync_log, sync_party, sync_topmost,
    track_leave, update,
};
use crate::hud::flags::Flag;

pub(super) unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    // SAFETY: Windows calls this with a live window handle, and the user data is null or the App that `run` keeps alive.
    unsafe {
        match msg {
            WM_NCCREATE => {
                let cs = std::ptr::with_exposed_provenance::<CREATESTRUCTW>(lp.cast_unsigned());
                SetWindowLongPtrW(
                    hwnd,
                    GWLP_USERDATA,
                    (*cs).lpCreateParams.expose_provenance().cast_signed(),
                );
                DefWindowProcW(hwnd, msg, wp, lp)
            }
            WM_TIMER if wp == TIMER_ID => {
                if let Some(app) = app_mut(hwnd) {
                    let tick = app.tick();
                    if tick.redraw.main {
                        InvalidateRect(hwnd, std::ptr::null(), 0);
                    }
                    if app.log.visible && !app.log.hwnd.is_null() {
                        if app.log.alpha != app.log.alpha_shown {
                            SetLayeredWindowAttributes(app.log.hwnd, 0, app.log.alpha, LWA_ALPHA);
                            app.log.alpha_shown = app.log.alpha;
                        }
                        if tick.redraw.log {
                            InvalidateRect(app.log.hwnd, std::ptr::null(), 0);
                        }
                    }
                    let pw = &mut app.party_win;
                    if pw.visible && !pw.hwnd.is_null() {
                        if pw.alpha != pw.alpha_shown {
                            SetLayeredWindowAttributes(pw.hwnd, 0, pw.alpha, LWA_ALPHA);
                            pw.alpha_shown = pw.alpha;
                        }
                        if tick.redraw.party {
                            InvalidateRect(pw.hwnd, std::ptr::null(), 0);
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
                sync_party(hwnd);
                0
            }
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &raw mut ps);
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
                EndPaint(hwnd, &raw const ps);
                0
            }
            WM_NCHITTEST => {
                let (x, y) = point(lp);
                let mut pt = POINT { x, y };
                ScreenToClient(hwnd, &raw mut pt);
                if let Some(app) = app_mut(hwnd)
                    && app.enabled_hit(pt.x, pt.y).is_some()
                {
                    return crate::num::to_isize(HTCLIENT);
                }
                crate::num::to_isize(HTCAPTION)
            }
            _ => main_mouse(hwnd, msg, wp, lp),
        }
    }
}

unsafe fn main_mouse(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: `wndproc` passes on the live window handle and message Windows gave it, and the user data is null or the App that `run` keeps alive.
    unsafe {
        match msg {
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
                        let mut tracking = app.flags.has(Flag::Tracking);
                        track_leave(hwnd, &mut tracking);
                        app.flags.set(Flag::Tracking, tracking);
                    }
                }
                0
            }
            WM_MOUSELEAVE | WM_NCMOUSEMOVE => {
                if let Some(app) = app_mut(hwnd) {
                    if msg == WM_MOUSELEAVE {
                        app.flags.set(Flag::Tracking, false);
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
                    match app.enabled_hit(x, y).filter(|h| h.clickable()) {
                        Some(hit) => {
                            app.pressed = Some(hit);
                            SetCapture(hwnd);
                            repaint(app, hwnd);
                        }
                        None => drag_window(hwnd),
                    }
                }
                0
            }
            _ => main_click(hwnd, msg, wp, lp),
        }
    }
}

unsafe fn main_click(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: `wndproc` passes on the live window handle and message Windows gave it, and the user data is null or the App that `run` keeps alive.
    unsafe {
        match msg {
            WM_LBUTTONUP => {
                let (x, y) = point(lp);
                let pressed = app_mut(hwnd).and_then(|app| app.pressed.take());
                ReleaseCapture();
                if let Some(app) = app_mut(hwnd)
                    && let Some(hit) = pressed
                {
                    let inside = app.enabled_hit(x, y) == Some(hit);
                    if inside {
                        match hit {
                            Hit::Close => {
                                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                            }
                            Hit::UpdateNow => {
                                update::spawn_install();
                                app.poll_badge();
                            }
                            Hit::UpdateSkip => {
                                if let Some(tag) = app.skip_update() {
                                    save_skipped(&tag);
                                }
                            }
                            Hit::Mute | Hit::Sound => {
                                app.activate(hit);
                                save_sound(app.flags.has(Flag::Sound));
                            }
                            Hit::Pin => {
                                app.activate(hit);
                                save_topmost(app.flags.has(Flag::Topmost));
                                sync_topmost(hwnd);
                            }
                            Hit::Discord => {
                                app.activate(hit);
                                save_discord(app.flags.has(Flag::Discord));
                            }
                            Hit::Usage | Hit::UsageYes | Hit::UsageNo => {
                                app.activate(hit);
                                save_usage(app.flags.has(Flag::Usage));
                            }
                            Hit::Board | Hit::BoardDelete => {
                                app.activate(hit);
                                save_board(app.flags.has(Flag::Board));
                            }
                            Hit::Party => {
                                app.activate(hit);
                                sync_party(hwnd);
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
                sync_log(hwnd);
                0
            }
            _ => main_frame(hwnd, msg, wp, lp),
        }
    }
}

unsafe fn main_frame(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: `wndproc` passes on the live window handle and message Windows gave it, and the user data is null or the App that `run` keeps alive.
    unsafe {
        match msg {
            WM_MOUSEWHEEL => {
                let (x, y) = point(lp);
                let mut pt = POINT { x, y };
                ScreenToClient(hwnd, &raw mut pt);
                let delta = i32::from(crate::num::to_u16((wp >> 16) & 0xffff).cast_signed());
                if let Some(app) = app_mut(hwnd)
                    && app.enabled_hit(pt.x, pt.y) == Some(Hit::Splits)
                    && app.split_wheel(delta)
                {
                    repaint(app, hwnd);
                }
                0
            }
            WM_CAPTURECHANGED => {
                if let Some(app) = app_mut(hwnd)
                    && app.pressed.take().is_some()
                {
                    repaint(app, hwnd);
                }
                0
            }
            WM_SETCURSOR if crate::num::to_u32(lp & 0xffff) == HTCLIENT => {
                let mut pt = POINT { x: 0, y: 0 };
                GetCursorPos(&raw mut pt);
                ScreenToClient(hwnd, &raw mut pt);
                let pointing = app_mut(hwnd)
                    .and_then(|app| app.enabled_hit(pt.x, pt.y))
                    .is_some_and(super::super::render::Hit::clickable);
                let cursor = if pointing { IDC_HAND } else { IDC_ARROW };
                SetCursor(LoadCursorW(std::ptr::null_mut(), cursor));
                1
            }
            WM_KEYDOWN if wp == crate::num::to_usize(VK_ESCAPE) => {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                0
            }
            WM_ENTERSIZEMOVE => {
                MOVING.store(true, Ordering::Relaxed);
                0
            }
            WM_EXITSIZEMOVE => {
                MOVING.store(false, Ordering::Relaxed);
                if let Some(app) = app_mut(hwnd) {
                    fit_windows(app, hwnd);
                }
                save_pos(hwnd);
                0
            }
            WM_DPICHANGED => {
                if let Some(app) = app_mut(hwnd) {
                    dpi_changed(app, hwnd, hwnd, wp, lp);
                }
                0
            }
            WM_DISPLAYCHANGE => {
                if let Some(app) = app_mut(hwnd) {
                    fit_windows(app, hwnd);
                    keep_all_on_screen(app, hwnd);
                }
                0
            }
            WM_CLOSE => {
                if let Some(app) = app_mut(hwnd) {
                    if !app.log.hwnd.is_null() {
                        save_log_pos(app.log.hwnd, app.log.visible);
                    }
                    if !app.party_win.hwnd.is_null() {
                        save_party_pos(app.party_win.hwnd, app.party_win.visible);
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
}
