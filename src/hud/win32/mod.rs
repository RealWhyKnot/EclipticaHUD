mod log_proc;
mod main_proc;
mod placement;

mod store;
mod window;

use crate::hud::app::App;
use crate::hud::app::WINDOW_STEPS;
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

use log_proc::*;
use main_proc::*;
use placement::*;
use store::*;
use window::*;

const TIMER_ID: usize = 1;

const WM_MOUSELEAVE: u32 = 0x02a3;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
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
