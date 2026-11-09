use super::{
    App, AtomicBool, CreateWindowExW, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute, GWLP_USERDATA,
    GetDpiForWindow, GetModuleHandleW, GetWindowLongPtrW, HTCAPTION, HWND, HWND_NOTOPMOST,
    HWND_TOPMOST, InvalidateRect, IsWindowVisible, LOG_H, LOG_W, LOGICAL_H, LOGICAL_W, LPARAM,
    LWA_ALPHA, MAX_SCALE, Ordering, PARTY_H, PARTY_W, RECT, ReleaseCapture, Renderer, SW_HIDE,
    SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageW,
    SetLayeredWindowAttributes, SetWindowPos, ShowWindow, TME_LEAVE, TRACKMOUSEEVENT,
    TrackMouseEvent, WM_NCLBUTTONDOWN, WPARAM, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP, default_log_pos, default_party_pos, fit_scale, keep_on_screen, save_log_pos,
    save_party_pos, saved_pos, wide, work_of,
};
use crate::hud::flags::Flag;

pub(super) unsafe fn app_mut(hwnd: HWND) -> Option<&'static mut App> {
    // SAFETY: the caller passes a window whose user data is null or the App that `run` keeps alive.
    unsafe {
        let p = std::ptr::with_exposed_provenance_mut::<App>(
            GetWindowLongPtrW(hwnd, GWLP_USERDATA).cast_unsigned(),
        );
        p.as_mut()
    }
}

pub(super) unsafe fn repaint(app: &mut App, hwnd: HWND) {
    // SAFETY: the caller passes a live window of this thread.
    unsafe {
        app.render();
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }
}

pub(super) unsafe fn repaint_log(app: &mut App) {
    // SAFETY: the stored handle is null or a live window of this thread.
    unsafe {
        if !app.log.hwnd.is_null() {
            app.render_log();
            InvalidateRect(app.log.hwnd, std::ptr::null(), 0);
        }
    }
}

pub(super) unsafe fn repaint_party(app: &mut App) {
    // SAFETY: the stored handle is null or a live window of this thread.
    unsafe {
        if !app.party_win.hwnd.is_null() {
            app.render_party();
            InvalidateRect(app.party_win.hwnd, std::ptr::null(), 0);
        }
    }
}

pub(super) unsafe fn drag_window(hwnd: HWND) {
    // SAFETY: the caller passes a live window of this thread.
    unsafe {
        ReleaseCapture();
        SendMessageW(hwnd, WM_NCLBUTTONDOWN, crate::num::to_usize(HTCAPTION), 0);
    }
}

pub(super) fn point(lp: LPARAM) -> (i32, i32) {
    (
        i32::from(crate::num::to_u16(lp & 0xffff).cast_signed()),
        i32::from(crate::num::to_u16((lp >> 16) & 0xffff).cast_signed()),
    )
}

pub(super) unsafe fn track_leave(hwnd: HWND, tracking: &mut bool) {
    // SAFETY: the caller passes a live window of this thread, and `track` has its size set.
    unsafe {
        if *tracking {
            return;
        }
        let mut track = TRACKMOUSEEVENT {
            cbSize: crate::num::to_u32(std::mem::size_of::<TRACKMOUSEEVENT>()),
            dwFlags: TME_LEAVE,
            hwndTrack: hwnd,
            dwHoverTime: 0,
        };
        if TrackMouseEvent(&raw mut track) != 0 {
            *tracking = true;
        }
    }
}

pub(super) unsafe fn apply_alpha(app: &App, main: HWND) {
    // SAFETY: the caller passes a live layered window of this thread.
    unsafe {
        SetLayeredWindowAttributes(main, 0, app.alpha_byte(), LWA_ALPHA);
    }
}

pub(super) static MOVING: AtomicBool = AtomicBool::new(false);

fn windows(app: &App, main: HWND) -> [(HWND, &Renderer); 3] {
    [
        (main, &app.renderer),
        (app.log.hwnd, &app.log.renderer),
        (app.party_win.hwnd, &app.party_win.renderer),
    ]
}

pub(super) unsafe fn apply_size(app: &mut App, main: HWND) {
    // SAFETY: the caller passes the main window, and the other handles are null or live windows of this thread.
    unsafe {
        for (hwnd, r) in windows(app, main) {
            if hwnd.is_null() {
                continue;
            }
            let flags = SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE;
            SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, r.width, r.height, flags);
            keep_on_screen(hwnd);
        }
        repaint(app, main);
        repaint_log(app);
        repaint_party(app);
    }
}

pub(super) unsafe fn keep_all_on_screen(app: &App, main: HWND) {
    for (hwnd, _) in windows(app, main) {
        if !hwnd.is_null() {
            keep_on_screen(hwnd);
        }
    }
}

fn scale_cap(app: &App, main: HWND) -> f32 {
    [
        (main, true, app.dpi, (LOGICAL_W, LOGICAL_H)),
        (app.log.hwnd, app.log.visible, app.log.dpi, (LOG_W, LOG_H)),
        (
            app.party_win.hwnd,
            app.party_win.visible,
            app.party_win.dpi,
            (PARTY_W, PARTY_H),
        ),
    ]
    .into_iter()
    .filter(|&(hwnd, shown, _, _)| shown && !hwnd.is_null())
    .map(|(hwnd, _, dpi, size)| {
        let w = work_of(hwnd);
        fit_scale(dpi, size, (w.right - w.left, w.bottom - w.top))
    })
    .fold(MAX_SCALE, f32::min)
}

pub(super) unsafe fn fit_windows(app: &mut App, main: HWND) {
    // SAFETY: the caller passes the main window, and the other handles are null or live windows of this thread.
    unsafe {
        let (log, party) = (app.log.hwnd, app.party_win.hwnd);
        let mut changed = false;
        for (hwnd, dpi) in [
            (main, &mut app.dpi),
            (log, &mut app.log.dpi),
            (party, &mut app.party_win.dpi),
        ] {
            let now = if hwnd.is_null() {
                0
            } else {
                GetDpiForWindow(hwnd)
            };
            if now != 0 && now != *dpi {
                *dpi = now;
                changed = true;
            }
        }
        let resized = app.set_scale_cap(scale_cap(app, main));
        if changed && !resized {
            app.rescale();
        }
        if changed || resized {
            apply_size(app, main);
        }
    }
}

pub(super) unsafe fn dpi_changed(app: &mut App, main: HWND, hwnd: HWND, wp: WPARAM, lp: LPARAM) {
    // SAFETY: for WM_DPICHANGED the system passes `lp` as a pointer to a RECT that lives for the message.
    unsafe {
        let dpi = crate::num::to_u32(wp & 0xffff);
        if hwnd == main {
            app.dpi = dpi;
        } else if hwnd == app.log.hwnd {
            app.log.dpi = dpi;
        } else if hwnd == app.party_win.hwnd {
            app.party_win.dpi = dpi;
        }
        app.rescale();
        let r = &*std::ptr::with_exposed_provenance::<RECT>(lp.cast_unsigned());
        let size = windows(app, main)
            .into_iter()
            .find(|&(h, _)| h == hwnd)
            .map(|(_, r)| (r.width, r.height));
        if let Some((w, h)) = size {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                r.left,
                r.top,
                w,
                h,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        if !MOVING.load(Ordering::Relaxed) {
            fit_windows(app, main);
        }
        repaint(app, main);
        repaint_log(app);
        repaint_party(app);
    }
}

pub(super) unsafe fn sync_topmost(main: HWND) {
    // SAFETY: the caller passes the main window, whose user data is null or the live App.
    unsafe {
        let Some(app) = app_mut(main) else {
            return;
        };
        let after = if app.flags.has(Flag::Topmost) {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
        SetWindowPos(main, after, 0, 0, 0, 0, flags);
        for hwnd in [app.log.hwnd, app.party_win.hwnd] {
            if !hwnd.is_null() {
                SetWindowPos(hwnd, after, 0, 0, 0, 0, flags);
            }
        }
    }
}

pub(super) unsafe fn apply_dwm(hwnd: HWND) {
    // SAFETY: the caller passes a live window, and each attribute pointer and size describe a live i32.
    unsafe {
        let dark: i32 = 1;
        DwmSetWindowAttribute(
            hwnd,
            crate::num::to_u32(DWMWA_USE_IMMERSIVE_DARK_MODE),
            (&raw const dark).cast::<core::ffi::c_void>(),
            4,
        );
        let round: i32 = DWMWCP_ROUND;
        DwmSetWindowAttribute(
            hwnd,
            crate::num::to_u32(DWMWA_WINDOW_CORNER_PREFERENCE),
            (&raw const round).cast::<core::ffi::c_void>(),
            4,
        );
    }
}

pub(super) unsafe fn create_log_window(main: HWND) {
    // SAFETY: the caller passes the main window, whose user data is null or the live App.
    unsafe {
        let app_ptr = std::ptr::with_exposed_provenance_mut::<App>(
            GetWindowLongPtrW(main, GWLP_USERDATA).cast_unsigned(),
        );
        let Some(app) = app_ptr.as_mut() else {
            return;
        };
        let (w, h) = (app.log.renderer.width, app.log.renderer.height);
        let (x, y) = saved_pos("logpos.txt", w, h).unwrap_or_else(|| default_log_pos(main, w, h));
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
            app_ptr.cast::<core::ffi::c_void>(),
        );
        apply_dwm(hwnd);
        SetLayeredWindowAttributes(hwnd, 0, 0, LWA_ALPHA);
        if let Some(app) = app_ptr.as_mut() {
            app.log.hwnd = hwnd;
        }
    }
}

pub(super) unsafe fn sync_log(main: HWND) {
    // SAFETY: the caller passes the main window, whose user data is null or the live App.
    unsafe {
        let app_ptr = std::ptr::with_exposed_provenance_mut::<App>(
            GetWindowLongPtrW(main, GWLP_USERDATA).cast_unsigned(),
        );
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
        if want != shown {
            fit_windows(app, main);
        }
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
}

pub(super) unsafe fn create_party_window(main: HWND) {
    // SAFETY: the caller passes the main window, whose user data is null or the live App.
    unsafe {
        let app_ptr = std::ptr::with_exposed_provenance_mut::<App>(
            GetWindowLongPtrW(main, GWLP_USERDATA).cast_unsigned(),
        );
        let Some(app) = app_ptr.as_mut() else {
            return;
        };
        let (w, h) = (app.party_win.renderer.width, app.party_win.renderer.height);
        let log = (!app.log.hwnd.is_null() && app.log.visible).then_some(app.log.hwnd);
        let (x, y) =
            saved_pos("partypos.txt", w, h).unwrap_or_else(|| default_party_pos(main, log, w, h));
        let module = GetModuleHandleW(std::ptr::null());
        let class_name = wide("EclipticaHUDParty");
        let title = wide("Ecliptica HUD shared damage");
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
            app_ptr.cast::<core::ffi::c_void>(),
        );
        apply_dwm(hwnd);
        SetLayeredWindowAttributes(hwnd, 0, 0, LWA_ALPHA);
        if let Some(app) = app_ptr.as_mut() {
            app.party_win.hwnd = hwnd;
        }
    }
}

pub(super) unsafe fn sync_party(main: HWND) {
    // SAFETY: the caller passes the main window, whose user data is null or the live App.
    unsafe {
        let app_ptr = std::ptr::with_exposed_provenance_mut::<App>(
            GetWindowLongPtrW(main, GWLP_USERDATA).cast_unsigned(),
        );
        let Some(app) = app_ptr.as_mut() else {
            return;
        };
        let want = app.party_win.visible;
        if want && app.party_win.hwnd.is_null() {
            create_party_window(main);
            sync_topmost(main);
        }
        let Some(app) = app_ptr.as_mut() else {
            return;
        };
        let hwnd = app.party_win.hwnd;
        if hwnd.is_null() {
            return;
        }
        let shown = IsWindowVisible(hwnd) != 0;
        if want != shown {
            fit_windows(app, main);
        }
        if want && !shown {
            app.render_party();
            SetLayeredWindowAttributes(hwnd, 0, app.party_win.alpha, LWA_ALPHA);
            app.party_win.alpha_shown = app.party_win.alpha;
            ShowWindow(hwnd, SW_SHOW);
            save_party_pos(hwnd, true);
        } else if !want && shown {
            ShowWindow(hwnd, SW_HIDE);
            save_party_pos(hwnd, false);
        }
    }
}
