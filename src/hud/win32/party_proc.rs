use super::{
    BeginPaint, BitBlt, CREATESTRUCTW, DefWindowProcW, EndPaint, GW_OWNER, GWLP_USERDATA,
    GetCursorPos, GetWindow, HTCAPTION, HTCLIENT, HWND, IDC_ARROW, IDC_HAND, LPARAM, LRESULT,
    LoadCursorW, MOVING, Ordering, PAINTSTRUCT, POINT, PartyHit, PostMessageW, ReleaseCapture,
    SRCCOPY, ScreenToClient, SetCapture, SetCursor, SetTimer, SetWindowLongPtrW, TIMER_ID,
    VK_ESCAPE, WM_CAPTURECHANGED, WM_CLOSE, WM_DESTROY, WM_DPICHANGED, WM_ENTERSIZEMOVE,
    WM_EXITSIZEMOVE, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSELEAVE, WM_MOUSEMOVE,
    WM_NCCREATE, WM_NCHITTEST, WM_NCMOUSEMOVE, WM_PAINT, WM_SETCURSOR, WPARAM, app_mut,
    dpi_changed, drag_window, fit_windows, point, repaint, repaint_party, save_party,
    save_party_pos, sync_party, track_leave,
};
use crate::hud::flags::Flag;

pub(super) unsafe extern "system" fn party_wndproc(
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
                    let r = &app.party_win.renderer;
                    BitBlt(hdc, 0, 0, r.width, r.height, r.dc, 0, 0, SRCCOPY);
                }
                EndPaint(hwnd, &raw const ps);
                0
            }
            WM_NCHITTEST => {
                let (x, y) = point(lp);
                let mut pt = POINT { x, y };
                ScreenToClient(hwnd, &raw mut pt);
                if app_mut(hwnd).is_some_and(|app| app.party_hit(pt.x, pt.y).is_some()) {
                    return crate::num::to_isize(HTCLIENT);
                }
                crate::num::to_isize(HTCAPTION)
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
            _ => party_click(hwnd, msg, wp, lp),
        }
    }
}

unsafe fn party_click(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: `party_wndproc` passes on the live window handle and message Windows gave it, and the user data is null or the App that `run` keeps alive.
    unsafe {
        match msg {
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
                            save_party(app.flags.has(Flag::Party));
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
                if let Some(app) = app_mut(hwnd)
                    && app.party_win.pressed.take().is_some()
                {
                    repaint_party(app);
                }
                0
            }
            WM_SETCURSOR if crate::num::to_u32(lp & 0xffff) == HTCLIENT => {
                let mut pt = POINT { x: 0, y: 0 };
                GetCursorPos(&raw mut pt);
                ScreenToClient(hwnd, &raw mut pt);
                let pointing = app_mut(hwnd)
                    .and_then(|app| app.party_hit(pt.x, pt.y))
                    .is_some_and(super::super::render::PartyHit::clickable);
                let cursor = if pointing { IDC_HAND } else { IDC_ARROW };
                SetCursor(LoadCursorW(std::ptr::null_mut(), cursor));
                1
            }
            WM_KEYDOWN if wp == crate::num::to_usize(VK_ESCAPE) => {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                0
            }
            _ => party_frame(hwnd, msg, wp, lp),
        }
    }
}

unsafe fn party_frame(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: `party_wndproc` passes on the live window handle and message Windows gave it, and the user data is null or the App that `run` keeps alive.
    unsafe {
        match msg {
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
                save_party_pos(hwnd, true);
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
                    app.party_win.hwnd = std::ptr::null_mut();
                }
                0
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}
