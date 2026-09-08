use std::time::Instant;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Diagnostics::ToolHelp::*;

const OVERLAY_KEY: &str = "whyknot.eclipticahud";
const OVERLAY_NAME: &str = "Ecliptica HUD";
const WIDTH_METERS: f32 = 0.28;
const RETRY_SECS: u64 = 10;
const WRIST_TRANSFORM: [[f32; 4]; 3] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, -1.0, 0.05],
    [0.0, 1.0, 0.0, 0.10],
];

struct Inner {
    context: openvr::Context,
    handle: openvr::overlay::OverlayHandle,
    attached: bool,
    needs_frame: bool,
    failures: u32,
}

pub struct VrOverlay {
    inner: Option<Inner>,
    last_try: Option<Instant>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum VrStatus {
    Off,
    On,
    Failing,
}

fn steamvr_running() -> bool {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut more = Process32FirstW(snap, &mut entry) != 0;
        while more && !found {
            let name: Vec<u16> = entry.szExeFile.iter().copied().take_while(|&c| c != 0).collect();
            let name = String::from_utf16_lossy(&name);
            found = name.eq_ignore_ascii_case("vrserver.exe");
            more = Process32NextW(snap, &mut entry) != 0;
        }
        windows_sys::Win32::Foundation::CloseHandle(snap);
        found
    }
}

impl VrOverlay {
    pub fn new() -> Self {
        VrOverlay { inner: None, last_try: None }
    }

    fn try_init(&mut self) {
        self.last_try = Some(Instant::now());
        if !steamvr_running() {
            return;
        }
        let context = match unsafe { openvr::init(openvr::ApplicationType::Overlay) } {
            Ok(c) => c,
            Err(_) => return,
        };
        let Ok(mut overlay) = context.overlay() else { return };
        let Ok(handle) = overlay.create_overlay(OVERLAY_KEY, OVERLAY_NAME) else { return };
        let _ = overlay.set_width(handle, WIDTH_METERS);
        let _ = overlay.set_visibility(handle, true);
        self.inner = Some(Inner { context, handle, attached: false, needs_frame: true, failures: 0 });
    }

    fn attach(inner: &mut Inner) {
        if inner.attached {
            return;
        }
        let Ok(system) = inner.context.system() else { return };
        let Some(index) =
            system.tracked_device_index_for_controller_role(openvr::TrackedControllerRole::LeftHand)
        else {
            return;
        };
        let Ok(mut overlay) = inner.context.overlay() else { return };
        let transform = openvr::pose::Matrix3x4(WRIST_TRANSFORM);
        if overlay.set_transform_tracked_device_relative(inner.handle, index, &transform).is_ok() {
            inner.attached = true;
        }
    }

    pub fn status(&self) -> VrStatus {
        match &self.inner {
            None => VrStatus::Off,
            Some(inner) if inner.failures > 0 => VrStatus::Failing,
            Some(_) => VrStatus::On,
        }
    }

    pub fn submit(&mut self, rgba: &[u8], width: u32, height: u32, changed: bool) {
        if self.inner.is_none() {
            let due = self.last_try.is_none_or(|t| t.elapsed().as_secs() >= RETRY_SECS);
            if due {
                self.try_init();
            }
        }
        let Some(inner) = self.inner.as_mut() else { return };
        Self::attach(inner);
        if !(changed || inner.needs_frame) || rgba.is_empty() {
            return;
        }
        let ok = match inner.context.overlay() {
            Ok(mut overlay) => overlay
                .set_raw_data(inner.handle, rgba, width as usize, height as usize, 4)
                .is_ok(),
            Err(_) => false,
        };
        if ok {
            inner.failures = 0;
            inner.needs_frame = false;
        } else {
            inner.failures += 1;
            if inner.failures >= 3 {
                self.inner = None;
            }
        }
    }
}
