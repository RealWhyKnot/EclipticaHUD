mod log_proc;
mod main_proc;
mod party_proc;
mod placement;

mod store;
mod window;

use crate::history::History;
use crate::hud::app::App;
use crate::hud::app::{WINDOW_STEPS, fit_scale};
use crate::hud::flags::Flag;
use crate::hud::render::{
    Hit, LOG_H, LOG_W, LOGICAL_H, LOGICAL_W, LogHit, MAX_ALPHA, MAX_SCALE, MIN_ALPHA, MIN_SCALE,
    PARTY_H, PARTY_W, PartyHit, Renderer,
};
use crate::update;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DwmSetWindowAttribute,
};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, EndPaint, GetMonitorInfoW, InvalidateRect, MONITOR_DEFAULTTONEAREST,
    MONITOR_DEFAULTTONULL, MONITOR_FROM_FLAGS, MONITORINFO, MonitorFromRect, PAINTSTRUCT, SRCCOPY,
    ScreenToClient,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem, GetDpiForWindow,
    SetProcessDpiAwarenessContext,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_ESCAPE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DispatchMessageW, GW_OWNER, GWLP_USERDATA,
    GetCursorPos, GetMessageW, GetWindow, GetWindowLongPtrW, GetWindowRect, HTCAPTION, HTCLIENT,
    HWND_NOTOPMOST, HWND_TOPMOST, IDC_ARROW, IDC_HAND, IDC_SIZENS, IsWindowVisible, LWA_ALPHA,
    LoadCursorW, MSG, PostMessageW, PostQuitMessage, RegisterClassW, SPI_GETWORKAREA, SW_HIDE,
    SW_SHOW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageW,
    SetCursor, SetLayeredWindowAttributes, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    SystemParametersInfoW, TranslateMessage, WM_CAPTURECHANGED, WM_CLOSE, WM_DESTROY,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCCREATE, WM_NCHITTEST, WM_NCLBUTTONDOWN,
    WM_NCMOUSEMOVE, WM_PAINT, WM_SETCURSOR, WM_TIMER, WNDCLASSW, WS_EX_APPWINDOW, WS_EX_LAYERED,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use log_proc::log_wndproc;
use main_proc::wndproc;
use party_proc::party_wndproc;
use placement::{
    default_log_pos, default_party_pos, default_pos, keep_on_screen, load_pos, save_log_pos,
    save_party_pos, save_pos, saved_pos, work_of,
};
use store::{
    alpha_from, board_forgotten, board_from, board_key, data_file, load_board, load_discord,
    load_party, load_skipped, load_sound, load_text, load_topmost, load_usage, save_alpha,
    save_board, save_discord, save_party, save_scale, save_skipped, save_sound, save_topmost,
    save_usage, save_window, scale_from, usage_answered, window_from, write_data,
};
use window::{
    MOVING, app_mut, apply_alpha, apply_dwm, apply_size, dpi_changed, drag_window, fit_windows,
    keep_all_on_screen, point, repaint, repaint_log, repaint_party, sync_log, sync_party,
    sync_topmost, track_leave,
};

const TIMER_ID: usize = 1;

const WM_MOUSELEAVE: u32 = 0x02a3;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub(crate) fn run() {
    update::cleanup_old();
    update::spawn_check(load_skipped());
    // SAFETY: the App outlives the message loop that holds its pointer, and every buffer passed here outlives its call.
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let dpi = GetDpiForSystem();
        let mut app = App::new(dpi);
        app.flags.set(Flag::Sound, load_sound());
        app.flags.set(Flag::Topmost, load_topmost());
        app.set_scale(load_text("scale.txt").map_or(1.0, |t| scale_from(&t)));
        app.set_alpha(load_text("alpha.txt").map_or(MAX_ALPHA, |t| alpha_from(&t)));
        app.set_window(
            load_text("window.txt").map_or(crate::game::state::DEFAULT_WINDOW, |t| window_from(&t)),
        );
        app.set_history(data_file("history.db").and_then(|p| History::open(&p)));
        app.backfill_history();
        app.restore_usage(load_usage());
        app.flags.set(Flag::UsageAsk, !usage_answered());
        crate::usage::spawn(data_file("history.db"));
        let (board_on, board_deleting) = board_from(&load_board());
        if board_on {
            app.set_board(true);
        }
        if board_deleting {
            app.delete_board();
        }
        crate::board::spawn(board_key(), board_forgotten);
        if load_discord() {
            app.set_discord(true);
        }
        app.set_party(load_party());
        app.set_vr_config(crate::vr::load_config());
        let (w, h) = (app.renderer.width, app.renderer.height);

        let module = GetModuleHandleW(std::ptr::null());
        let class_name = wide("EclipticaHUD");
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = module;
        wc.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
        wc.lpszClassName = class_name.as_ptr();
        RegisterClassW(&raw const wc);
        let log_class = wide("EclipticaHUDLog");
        let mut lc: WNDCLASSW = std::mem::zeroed();
        lc.lpfnWndProc = Some(log_wndproc);
        lc.hInstance = module;
        lc.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
        lc.lpszClassName = log_class.as_ptr();
        RegisterClassW(&raw const lc);
        let party_class = wide("EclipticaHUDParty");
        let mut pc: WNDCLASSW = std::mem::zeroed();
        pc.lpfnWndProc = Some(party_wndproc);
        pc.hInstance = module;
        pc.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
        pc.lpszClassName = party_class.as_ptr();
        RegisterClassW(&raw const pc);

        let (x, y) = saved_pos("pos.txt", w, h).unwrap_or_else(|| default_pos(w, h));
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
            (&raw mut app).cast::<core::ffi::c_void>(),
        );
        apply_dwm(hwnd);
        apply_alpha(&app, hwnd);

        app.tick();
        app.render();
        fit_windows(&mut app, hwnd);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        sync_topmost(hwnd);
        SetTimer(hwnd, TIMER_ID, 1000, None);
        if load_pos("logpos.txt").is_some_and(|(_, _, open)| open) {
            app.log_open();
            sync_log(hwnd);
        }
        if load_pos("partypos.txt").is_some_and(|(_, _, open)| open) {
            app.party_open();
            sync_party(hwnd);
        }

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&raw mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&raw const msg);
            DispatchMessageW(&raw const msg);
        }
    }
    crate::relay::HUB.shutdown(std::time::Duration::from_millis(2500));
    if update::restart_pending() {
        update::relaunch();
    }
}
