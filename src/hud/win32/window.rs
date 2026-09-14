use super::*;

pub(super) unsafe fn app_mut(hwnd: HWND) -> Option<&'static mut App> {
    let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
    p.as_mut()
}

pub(super) unsafe fn repaint(app: &mut App, hwnd: HWND) {
    app.render();
    InvalidateRect(hwnd, std::ptr::null(), 0);
}

pub(super) unsafe fn repaint_log(app: &mut App) {
    if !app.log.hwnd.is_null() {
        app.render_log();
        InvalidateRect(app.log.hwnd, std::ptr::null(), 0);
    }
}

pub(super) fn point(lp: LPARAM) -> (i32, i32) {
    (
        (lp & 0xffff) as i16 as i32,
        ((lp >> 16) & 0xffff) as i16 as i32,
    )
}

pub(super) unsafe fn track_leave(hwnd: HWND, tracking: &mut bool) {
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

pub(super) unsafe fn apply_alpha(app: &App, main: HWND) {
    SetLayeredWindowAttributes(main, 0, app.alpha_byte(), LWA_ALPHA);
}

pub(super) unsafe fn apply_size(app: &mut App, main: HWND) {
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

pub(super) unsafe fn sync_topmost(main: HWND) {
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

pub(super) unsafe fn apply_dwm(hwnd: HWND) {
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

pub(super) unsafe fn create_log_window(main: HWND) {
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

pub(super) unsafe fn sync_log(main: HWND) {
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
