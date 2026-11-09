use super::{
    BeginPaint, BitBlt, CREATESTRUCTW, DefWindowProcW, EndPaint, GW_OWNER, GWLP_USERDATA,
    GetCursorPos, GetWindow, HTCAPTION, HTCLIENT, HWND, IDC_ARROW, IDC_HAND, IDC_SIZENS, LPARAM,
    LRESULT, LoadCursorW, LogHit, MOVING, Ordering, PAINTSTRUCT, POINT, PostMessageW,
    ReleaseCapture, SRCCOPY, ScreenToClient, SetCapture, SetCursor, SetTimer, SetWindowLongPtrW,
    TIMER_ID, VK_ESCAPE, WM_CAPTURECHANGED, WM_CLOSE, WM_DESTROY, WM_DPICHANGED, WM_ENTERSIZEMOVE,
    WM_EXITSIZEMOVE, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSELEAVE, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_NCCREATE, WM_NCHITTEST, WM_NCMOUSEMOVE, WM_PAINT, WM_SETCURSOR, WPARAM,
    app_mut, dpi_changed, drag_window, fit_windows, point, repaint_log, save_log_pos, sync_log,
    track_leave,
};

pub(super) unsafe extern "system" fn log_wndproc(
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
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &raw mut ps);
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
                EndPaint(hwnd, &raw const ps);
                0
            }
            WM_NCHITTEST => {
                let (x, y) = point(lp);
                let mut pt = POINT { x, y };
                ScreenToClient(hwnd, &raw mut pt);
                if let Some(app) = app_mut(hwnd) {
                    let thumb = app.log_thumb();
                    if app.log.renderer.log_hit_test(pt.x, pt.y, thumb).is_some() {
                        return crate::num::to_isize(HTCLIENT);
                    }
                }
                crate::num::to_isize(HTCAPTION)
            }
            _ => log_mouse(hwnd, msg, wp, lp),
        }
    }
}

unsafe fn log_mouse(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: `log_wndproc` passes on the live window handle and message Windows gave it, and the user data is null or the App that `run` keeps alive.
    unsafe {
        match msg {
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
                    match app.log.renderer.log_hit_test(x, y, thumb) {
                        Some(LogHit::Title | LogHit::Count) | None => drag_window(hwnd),
                        Some(hit) => {
                            app.log_press(hit, y);
                            if hit == LogHit::Thumb {
                                SetCapture(hwnd);
                            }
                            repaint_log(app);
                        }
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
                if dragging && let Some(app) = app_mut(hwnd) {
                    repaint_log(app);
                }
                0
            }
            WM_MOUSEWHEEL => {
                let delta = i32::from(crate::num::to_u16((wp >> 16) & 0xffff).cast_signed());
                if let Some(app) = app_mut(hwnd) {
                    app.log_wheel(delta);
                    repaint_log(app);
                    SetTimer(GetWindow(hwnd, GW_OWNER), TIMER_ID, 16, None);
                }
                0
            }
            _ => log_frame(hwnd, msg, wp, lp),
        }
    }
}

unsafe fn log_frame(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: `log_wndproc` passes on the live window handle and message Windows gave it, and the user data is null or the App that `run` keeps alive.
    unsafe {
        match msg {
            WM_SETCURSOR if crate::num::to_u32(lp & 0xffff) == HTCLIENT => {
                let mut pt = POINT { x: 0, y: 0 };
                GetCursorPos(&raw mut pt);
                ScreenToClient(hwnd, &raw mut pt);
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
            WM_KEYDOWN if wp == crate::num::to_usize(VK_ESCAPE) => {
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
            WM_ENTERSIZEMOVE => {
                MOVING.store(true, Ordering::Relaxed);
                0
            }
            WM_EXITSIZEMOVE => {
                MOVING.store(false, Ordering::Relaxed);
                let owner = GetWindow(hwnd, GW_OWNER);
                if let Some(app) = app_mut(hwnd) {
                    fit_windows(app, owner);
                }
                save_log_pos(hwnd, true);
                0
            }
            WM_DPICHANGED => {
                let owner = GetWindow(hwnd, GW_OWNER);
                if let Some(app) = app_mut(hwnd) {
                    dpi_changed(app, owner, hwnd, wp, lp);
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
}
