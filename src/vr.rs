use crate::process;
use std::time::Instant;

const OVERLAY_KEY: &str = "whyknot.eclipticahud";
const OVERLAY_NAME: &str = "Ecliptica HUD";
const PARTY_KEY: &str = "whyknot.eclipticahud.party";
const PARTY_NAME: &str = "Ecliptica HUD shared damage";
const WIDTH_METERS: f32 = 0.28;
const RETRY_SECS: u64 = 10;
const MAX_FAILURES: u32 = 3;
const WRIST_TRANSFORM: [[f32; 4]; 3] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, -1.0, 0.05],
    [0.0, 1.0, 0.0, 0.10],
];
const PARTY_SIDE_M: f32 = 0.30;
const PARTY_RISE_M: f32 = 0.0965;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Main,
    Party,
}

fn transform(kind: Kind, scale: f32) -> [[f32; 4]; 3] {
    let mut m = WRIST_TRANSFORM;
    if kind == Kind::Party {
        m[0][3] += PARTY_SIDE_M * scale;
        m[2][3] += PARTY_RISE_M * scale;
    }
    m
}

struct Slot {
    kind: Kind,
    handle: openvr::overlay::OverlayHandle,
    attached: bool,
    needs_frame: bool,
    failures: u32,
    visible: bool,
    alpha: f32,
    scale: f32,
}

struct Inner {
    context: openvr::Context,
    main: Slot,
    party: Option<Slot>,
    party_tried: bool,
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

struct VrFrame<'a> {
    rgba: &'a [u8],
    width: u32,
    height: u32,
    changed: bool,
    visible: bool,
    alpha: f32,
    scale: f32,
}

impl Slot {
    fn create(context: &openvr::Context, kind: Kind, visible: bool) -> Option<Slot> {
        let mut overlay = context.overlay().ok()?;
        let (key, name) = match kind {
            Kind::Main => (OVERLAY_KEY, OVERLAY_NAME),
            Kind::Party => (PARTY_KEY, PARTY_NAME),
        };
        let handle = overlay.create_overlay(key, name).ok()?;
        let _ = overlay.set_width(handle, WIDTH_METERS);
        let _ = overlay.set_visibility(handle, visible);
        Some(Slot {
            kind,
            handle,
            attached: false,
            needs_frame: true,
            failures: 0,
            visible,
            alpha: 1.0,
            scale: 1.0,
        })
    }

    fn attach(&mut self, context: &openvr::Context) {
        if self.attached {
            return;
        }
        let Ok(system) = context.system() else {
            return;
        };
        let Some(index) = system
            .tracked_device_index_for_controller_role(openvr::TrackedControllerRole::LeftHand)
        else {
            return;
        };
        let Ok(mut overlay) = context.overlay() else {
            return;
        };
        let m = openvr::pose::Matrix3x4(transform(self.kind, self.scale));
        if overlay
            .set_transform_tracked_device_relative(self.handle, index, &m)
            .is_ok()
        {
            self.attached = true;
        }
    }

    fn push(&mut self, context: &openvr::Context, f: &VrFrame) {
        if self.visible != f.visible {
            if let Ok(mut overlay) = context.overlay() {
                if overlay.set_visibility(self.handle, f.visible).is_ok() {
                    self.visible = f.visible;
                }
            }
        }
        let alpha = f.alpha.clamp(0.0, 1.0);
        if self.alpha != alpha {
            if let Ok(mut overlay) = context.overlay() {
                if overlay.set_opacity(self.handle, alpha).is_ok() {
                    self.alpha = alpha;
                }
            }
        }
        if self.scale != f.scale {
            if let Ok(mut overlay) = context.overlay() {
                if overlay
                    .set_width(self.handle, WIDTH_METERS * f.scale)
                    .is_ok()
                {
                    self.scale = f.scale;
                    if self.kind == Kind::Party {
                        self.attached = false;
                    }
                }
            }
        }
        self.attach(context);
        if !(f.changed || self.needs_frame) || f.rgba.is_empty() {
            return;
        }
        let ok = match context.overlay() {
            Ok(mut overlay) => overlay
                .set_raw_data(self.handle, f.rgba, f.width as usize, f.height as usize, 4)
                .is_ok(),
            Err(_) => false,
        };
        if ok {
            self.failures = 0;
            self.needs_frame = false;
        } else {
            self.failures += 1;
        }
    }
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
        if !process::running("vrserver.exe") {
            return;
        }
        let context = match unsafe { openvr::init(openvr::ApplicationType::Overlay) } {
            Ok(c) => c,
            Err(_) => return,
        };
        let Some(main) = Slot::create(&context, Kind::Main, true) else {
            return;
        };
        self.inner = Some(Inner {
            context,
            main,
            party: None,
            party_tried: false,
        });
    }

    pub fn status(&self) -> VrStatus {
        match &self.inner {
            None => VrStatus::Off,
            Some(inner) if inner.main.failures > 0 => VrStatus::Failing,
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
        let frame = VrFrame {
            rgba,
            width,
            height,
            changed,
            visible,
            alpha,
            scale,
        };
        inner.main.push(&inner.context, &frame);
        if inner.main.failures >= MAX_FAILURES {
            self.inner = None;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn submit_party(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
        changed: bool,
        visible: bool,
        alpha: f32,
        scale: f32,
    ) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };
        if inner.party.is_none() {
            if !visible || inner.party_tried {
                return;
            }
            inner.party_tried = true;
            inner.party = Slot::create(&inner.context, Kind::Party, false);
        }
        let Some(slot) = inner.party.as_mut() else {
            return;
        };
        if slot.failures >= MAX_FAILURES {
            return;
        }
        let frame = VrFrame {
            rgba,
            width,
            height,
            changed,
            visible,
            alpha,
            scale,
        };
        slot.push(&inner.context, &frame);
        if slot.failures >= MAX_FAILURES {
            if let Ok(mut overlay) = inner.context.overlay() {
                let _ = overlay.set_visibility(slot.handle, false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn party_overlay_sits_beside_the_main_one_with_tops_aligned() {
        let main_h = |s: f32| WIDTH_METERS * s * 640.0 / 360.0;
        let party_h = |s: f32| WIDTH_METERS * s * 392.0 / 360.0;
        for scale in [0.5f32, 1.0, 1.5, 2.0] {
            let m = transform(Kind::Main, scale);
            let p = transform(Kind::Party, scale);
            assert_eq!(m, WRIST_TRANSFORM);
            for r in 0..3 {
                for c in 0..3 {
                    assert_eq!(m[r][c], p[r][c]);
                }
            }
            let main_top = m[2][3] + main_h(scale) / 2.0;
            let party_top = p[2][3] + party_h(scale) / 2.0;
            assert!((main_top - party_top).abs() < 0.001, "{scale}");
            let gap = (p[0][3] - m[0][3]) - WIDTH_METERS * scale;
            assert!((gap - 0.02 * scale).abs() < 0.0005, "{scale} {gap}");
        }
    }
}
