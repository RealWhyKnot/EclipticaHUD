use super::*;

pub(super) unsafe extern "system" fn log_wndproc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
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
