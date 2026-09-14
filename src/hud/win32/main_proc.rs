use super::*;

pub(super) unsafe extern "system" fn wndproc(
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
