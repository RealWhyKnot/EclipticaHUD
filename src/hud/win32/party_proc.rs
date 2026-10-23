use super::*;

pub(super) unsafe extern "system" fn party_wndproc(
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
                let r = &app.party_win.renderer;
                BitBlt(hdc, 0, 0, r.width, r.height, r.dc, 0, 0, SRCCOPY);
            }
            EndPaint(hwnd, &ps);
            0
        }
        WM_NCHITTEST => {
            let (x, y) = point(lp);
            let mut pt = POINT { x, y };
            ScreenToClient(hwnd, &mut pt);
            if app_mut(hwnd).is_some_and(|app| app.party_hit(pt.x, pt.y).is_some()) {
                return HTCLIENT as LRESULT;
            }
            HTCAPTION as LRESULT
        }
        WM_MOUSEMOVE => {
            let (x, y) = point(lp);
            if let Some(app) = app_mut(hwnd) {
                let over = app.party_hit(x, y);
                if app.party_set_hover(over) {
                    repaint_party(app);
                    if over.is_some() {
                        SetTimer(GetWindow(hwnd, GW_OWNER), TIMER_ID, 16, None);
                        app.nudge_timer();
                    }
                }
                if over.is_some() {
                    track_leave(hwnd, &mut app.party_win.tracking);
                }
            }
            0
        }
        WM_MOUSELEAVE | WM_NCMOUSEMOVE => {
            if let Some(app) = app_mut(hwnd) {
                if msg == WM_MOUSELEAVE {
                    app.party_win.tracking = false;
                }
                if app.party_set_hover(None) {
                    repaint_party(app);
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
                match app.party_hit(x, y) {
                    Some(hit) if hit.clickable() => {
                        app.party_win.pressed = Some(hit);
                        SetCapture(hwnd);
                        repaint_party(app);
                    }
                    _ => drag_window(hwnd),
                }
            }
            0
        }
        WM_LBUTTONUP => {
            let (x, y) = point(lp);
            let pressed = app_mut(hwnd).and_then(|app| app.party_win.pressed.take());
            ReleaseCapture();
            if let Some(app) = app_mut(hwnd) {
                if let Some(hit) = pressed.filter(|h| app.party_hit(x, y) == Some(*h)) {
                    app.party_activate(hit);
                    if hit == PartyHit::Toggle {
                        save_party(app.party_on);
                    }
                }
                app.party_win.hover = app.party_hit(x, y);
                repaint_party(app);
            }
            let owner = GetWindow(hwnd, GW_OWNER);
            sync_party(owner);
            if let Some(app) = app_mut(owner) {
                repaint(app, owner);
            }
            0
        }
        WM_CAPTURECHANGED => {
            if let Some(app) = app_mut(hwnd) {
                if app.party_win.pressed.take().is_some() {
                    repaint_party(app);
                }
            }
            0
        }
        WM_SETCURSOR if (lp & 0xffff) as u32 == HTCLIENT => {
            let mut pt = POINT { x: 0, y: 0 };
            GetCursorPos(&mut pt);
            ScreenToClient(hwnd, &mut pt);
            let hand = app_mut(hwnd)
                .and_then(|app| app.party_hit(pt.x, pt.y))
                .is_some_and(|h| h.clickable());
            let cursor = if hand { IDC_HAND } else { IDC_ARROW };
            SetCursor(LoadCursorW(std::ptr::null_mut(), cursor));
            1
        }
        WM_KEYDOWN if wp == VK_ESCAPE as usize => {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
            0
        }
        WM_CLOSE => {
            if let Some(app) = app_mut(hwnd) {
                app.party_close();
            }
            let owner = GetWindow(hwnd, GW_OWNER);
            sync_party(owner);
            if let Some(app) = app_mut(owner) {
                repaint(app, owner);
            }
            0
        }
        WM_EXITSIZEMOVE => {
            save_party_pos(hwnd, true);
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
                    app.party_win.renderer.width,
                    app.party_win.renderer.height,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                repaint_party(app);
            }
            0
        }
        WM_DESTROY => {
            if let Some(app) = app_mut(hwnd) {
                app.party_win.hwnd = std::ptr::null_mut();
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
