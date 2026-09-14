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
    visible: bool,
    alpha: f32,
    scale: f32,
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

pub fn process_ids(exe: &str) -> Vec<u32> {
    let mut ids = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return ids;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut more = Process32FirstW(snap, &mut entry) != 0;
        while more {
            let name: Vec<u16> = entry
                .szExeFile
                .iter()
                .copied()
                .take_while(|&c| c != 0)
                .collect();
            if String::from_utf16_lossy(&name).eq_ignore_ascii_case(exe) {
                ids.push(entry.th32ProcessID);
            }
            more = Process32NextW(snap, &mut entry) != 0;
        }
        windows_sys::Win32::Foundation::CloseHandle(snap);
    }
    ids
}

pub fn process_running(exe: &str) -> bool {
    !process_ids(exe).is_empty()
}

impl VrOverlay {
    pub fn new() -> Self {
        VrOverlay {
            inner: None,
            last_try: None,
        }
    }

    fn try_init(&mut self) {
        self.last_try = Some(Instant::now());
        if !process_running("vrserver.exe") {
            return;
        }
        let context = match unsafe { openvr::init(openvr::ApplicationType::Overlay) } {
            Ok(c) => c,
            Err(_) => return,
        };
        let Ok(mut overlay) = context.overlay() else {
            return;
        };
        let Ok(handle) = overlay.create_overlay(OVERLAY_KEY, OVERLAY_NAME) else {
            return;
        };
        let _ = overlay.set_width(handle, WIDTH_METERS);
        let _ = overlay.set_visibility(handle, true);
        self.inner = Some(Inner {
            context,
            handle,
            attached: false,
            needs_frame: true,
            failures: 0,
            visible: true,
            alpha: 1.0,
            scale: 1.0,
        });
    }

    fn attach(inner: &mut Inner) {
        if inner.attached {
            return;
        }
        let Ok(system) = inner.context.system() else {
            return;
        };
        let Some(index) = system
            .tracked_device_index_for_controller_role(openvr::TrackedControllerRole::LeftHand)
        else {
            return;
        };
        let Ok(mut overlay) = inner.context.overlay() else {
            return;
        };
        let transform = openvr::pose::Matrix3x4(WRIST_TRANSFORM);
        if overlay
            .set_transform_tracked_device_relative(inner.handle, index, &transform)
            .is_ok()
        {
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

    #[allow(clippy::too_many_arguments)]
    pub fn submit(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
        changed: bool,
        visible: bool,
        alpha: f32,
        scale: f32,
    ) {
        if self.inner.is_none() {
            let due = self
                .last_try
                .is_none_or(|t| t.elapsed().as_secs() >= RETRY_SECS);
            if due {
                self.try_init();
            }
        }
        let Some(inner) = self.inner.as_mut() else {
            return;
        };
        Self::attach(inner);
        if inner.visible != visible {
            if let Ok(mut overlay) = inner.context.overlay() {
                if overlay.set_visibility(inner.handle, visible).is_ok() {
                    inner.visible = visible;
                }
            }
        }
        let alpha = alpha.clamp(0.0, 1.0);
        if inner.alpha != alpha {
            if let Ok(mut overlay) = inner.context.overlay() {
                if overlay.set_opacity(inner.handle, alpha).is_ok() {
                    inner.alpha = alpha;
                }
            }
        }
        if inner.scale != scale {
            if let Ok(mut overlay) = inner.context.overlay() {
                if overlay
                    .set_width(inner.handle, WIDTH_METERS * scale)
                    .is_ok()
                {
                    inner.scale = scale;
                }
            }
        }
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
