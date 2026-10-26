mod pose;
#[cfg(test)]
mod pose_tests;

use crate::process;
use pose::M34;
use std::time::Instant;

pub use pose::{Hand, Place};

const OVERLAY_KEY: &str = "whyknot.eclipticahud";
const OVERLAY_NAME: &str = "Ecliptica HUD";
const PARTY_KEY: &str = "whyknot.eclipticahud.party";
const PARTY_NAME: &str = "Ecliptica HUD shared damage";
pub const WIDTH_METERS: f32 = 0.28;
const RETRY_SECS: u64 = 10;
const MAX_FAILURES: u32 = 3;
const MAX_GRIP_OFFSET_M: f32 = 0.25;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VrConfig {
    pub hand: Hand,
    pub place: Place,
}

impl Default for VrConfig {
    fn default() -> Self {
        VrConfig {
            hand: Hand::Left,
            place: Place::Back,
        }
    }
}

fn apply(cfg: &mut VrConfig, key: &str, value: &str) {
    let v = value.trim().to_ascii_lowercase();
    match key.trim().to_ascii_lowercase().as_str() {
        "hand" => match v.as_str() {
            "left" | "l" => cfg.hand = Hand::Left,
            "right" | "r" => cfg.hand = Hand::Right,
            _ => {}
        },
        "place" => match v.as_str() {
            "old" => cfg.place = Place::Old,
            "back" => cfg.place = Place::Back,
            "inside" => cfg.place = Place::Inside,
            _ => {}
        },
        _ => {}
    }
}

pub fn config_from(text: &str, env: &dyn Fn(&str) -> Option<String>) -> VrConfig {
    let mut cfg = VrConfig::default();
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=') {
            apply(&mut cfg, k, v);
        }
    }
    for key in ["hand", "place"] {
        if let Some(v) = env(&format!("EHUD_VR_{}", key.to_ascii_uppercase())) {
            apply(&mut cfg, key, &v);
        }
    }
    cfg
}

pub fn load_config() -> VrConfig {
    let text = std::env::var_os("APPDATA")
        .map(|b| {
            std::path::PathBuf::from(b)
                .join("EclipticaHUD")
                .join("vr.txt")
        })
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    config_from(&text, &|k| std::env::var(k).ok())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Main,
    Party,
}

fn role(hand: Hand) -> openvr::TrackedControllerRole {
    match hand {
        Hand::Left => openvr::TrackedControllerRole::LeftHand,
        Hand::Right => openvr::TrackedControllerRole::RightHand,
    }
}

fn plausible_grip(m: &M34) -> bool {
    let o = pose::origin(m);
    let det = pose::dot(
        pose::axis(m, 0),
        pose::cross(pose::axis(m, 1), pose::axis(m, 2)),
    );
    m.iter().flatten().all(|v| v.is_finite())
        && pose::dot(o, o).sqrt() < MAX_GRIP_OFFSET_M
        && (det - 1.0).abs() < 0.01
}

fn grip_for(context: &openvr::Context, index: openvr::TrackedDeviceIndex, hand: Hand) -> M34 {
    let Ok(system) = context.system() else {
        return pose::grip_for_model("", hand);
    };
    let Ok(model) =
        system.string_tracked_device_property(index, openvr::property::RenderModelName_String)
    else {
        return pose::grip_for_model("", hand);
    };
    if let Ok(models) = context.render_models() {
        let state: openvr::ControllerState = unsafe { std::mem::zeroed() };
        let mode = openvr::render_models::ControllerMode::default();
        for name in [c"openxr_grip", c"handgrip"] {
            if let Some(s) = models.component_state(&model, name, &state, &mode) {
                if plausible_grip(&s.tracking_to_component_local) {
                    return s.tracking_to_component_local;
                }
            }
        }
    }
    pose::grip_for_model(&model.to_string_lossy(), hand)
}

struct Slot {
    kind: Kind,
    handle: openvr::overlay::OverlayHandle,
    placed: Option<(u32, M34)>,
    needs_frame: bool,
    failures: u32,
    visible: bool,
    alpha: f32,
    width_m: f32,
}

struct Inner {
    context: openvr::Context,
    main: Slot,
    party: Option<Slot>,
    party_tried: bool,
    grip: Option<(u32, Hand, M34)>,
    main_wh: (f32, f32),
}

pub struct VrOverlay {
    inner: Option<Inner>,
    last_try: Option<Instant>,
    config: VrConfig,
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
    width_m: f32,
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
            placed: None,
            needs_frame: true,
            failures: 0,
            visible,
            alpha: 1.0,
            width_m: WIDTH_METERS,
        })
    }

    fn place(&mut self, context: &openvr::Context, target: Option<(u32, M34)>) {
        let Some((index, m)) = target else {
            return;
        };
        if self.placed == Some((index, m)) {
            return;
        }
        let Ok(mut overlay) = context.overlay() else {
            return;
        };
        let device = openvr::TrackedDeviceIndex(index);
        if overlay
            .set_transform_tracked_device_relative(self.handle, device, &openvr::pose::Matrix3x4(m))
            .is_ok()
        {
            self.placed = Some((index, m));
        }
    }

    fn push(&mut self, context: &openvr::Context, f: &VrFrame, target: Option<(u32, M34)>) {
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
        if self.width_m != f.width_m {
            if let Ok(mut overlay) = context.overlay() {
                if overlay.set_width(self.handle, f.width_m).is_ok() {
                    self.width_m = f.width_m;
                }
            }
        }
        self.place(context, target);
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

fn frame_wh(f: &VrFrame) -> (f32, f32) {
    let h = if f.width == 0 {
        0.0
    } else {
        f.width_m * f.height as f32 / f.width as f32
    };
    (f.width_m, h)
}

impl Inner {
    fn target(&mut self, cfg: VrConfig, kind: Kind, wh: (f32, f32)) -> Option<(u32, M34)> {
        let system = self.context.system().ok()?;
        let index = system.tracked_device_index_for_controller_role(role(cfg.hand))?;
        let grip = match self.grip {
            Some((i, h, g)) if i == index.0 && h == cfg.hand => g,
            _ => {
                let g = grip_for(&self.context, index, cfg.hand);
                self.grip = Some((index.0, cfg.hand, g));
                g
            }
        };
        let main =
            pose::overlay_transform(cfg.place, cfg.hand, &grip, self.main_wh.0, self.main_wh.1);
        let m = match kind {
            Kind::Main => main,
            Kind::Party => pose::party(
                &main,
                cfg.place,
                cfg.hand,
                self.main_wh,
                wh,
                pose::SIDE_GAP_M * wh.0 / WIDTH_METERS,
            ),
        };
        Some((index.0, m))
    }
}

impl VrOverlay {
    pub fn new() -> Self {
        VrOverlay {
            inner: None,
            last_try: None,
            config: VrConfig::default(),
        }
    }

    pub fn set_config(&mut self, config: VrConfig) {
        self.config = config;
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
            grip: None,
            main_wh: (0.0, 0.0),
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
        let frame = &VrFrame {
            rgba,
            width,
            height,
            changed,
            visible,
            alpha,
            width_m: pose::panel_width(self.config.place, WIDTH_METERS * scale),
        };
        if self.inner.is_none() {
            let due = self
                .last_try
                .is_none_or(|t| t.elapsed().as_secs() >= RETRY_SECS);
            if due {
                self.try_init();
            }
        }
        let cfg = self.config;
        let Some(inner) = self.inner.as_mut() else {
            return;
        };
        let wh = frame_wh(frame);
        if wh.1 > 0.0 {
            inner.main_wh = wh;
        }
        let target = inner.target(cfg, Kind::Main, wh);
        inner.main.push(&inner.context, frame, target);
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
        let cfg = self.config;
        let frame = &VrFrame {
            rgba,
            width,
            height,
            changed,
            visible,
            alpha,
            width_m: pose::panel_width(cfg.place, WIDTH_METERS * scale),
        };
        let Some(inner) = self.inner.as_mut() else {
            return;
        };
        if inner.party.is_none() {
            if !frame.visible || inner.party_tried {
                return;
            }
            inner.party_tried = true;
            inner.party = Slot::create(&inner.context, Kind::Party, false);
        }
        let wh = frame_wh(frame);
        let target = inner.target(cfg, Kind::Party, wh);
        let Some(slot) = inner.party.as_mut() else {
            return;
        };
        debug_assert_eq!(slot.kind, Kind::Party);
        if slot.failures >= MAX_FAILURES {
            return;
        }
        slot.push(&inner.context, frame, target);
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

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn config_defaults_to_left_back() {
        let c = config_from("", &no_env);
        assert_eq!(c, VrConfig::default());
        assert_eq!((c.hand, c.place), (Hand::Left, Place::Back));
    }

    #[test]
    fn config_reads_the_file_and_env_wins() {
        let text = "hand = Right\nplace=inside\nlook=bar\nbuzz=off\njunk\n=\nhand=sideways\n";
        let c = config_from(text, &no_env);
        assert_eq!((c.hand, c.place), (Hand::Right, Place::Inside));
        let env = |k: &str| match k {
            "EHUD_VR_PLACE" => Some("old".to_string()),
            _ => None,
        };
        let c = config_from(text, &env);
        assert_eq!((c.hand, c.place), (Hand::Right, Place::Old));
    }

    #[test]
    fn oversized_config_text_is_ignored_safely() {
        let long = format!("hand={}\nplace={}\n", "x".repeat(100_000), "é".repeat(5000));
        assert_eq!(config_from(&long, &no_env), VrConfig::default());
    }

    #[test]
    fn grip_sanity_check_rejects_garbage() {
        assert!(plausible_grip(&pose::component(
            pose::TOUCH_GRIP.0,
            pose::TOUCH_GRIP.1
        )));
        let mut far = pose::component(pose::INDEX_GRIP.0, pose::INDEX_GRIP.1);
        far[2][3] = 0.5;
        assert!(!plausible_grip(&far));
        assert!(!plausible_grip(&[[0.0; 4]; 3]));
        let mut nan = pose::OLD_TRANSFORM;
        nan[0][0] = f32::NAN;
        assert!(!plausible_grip(&nan));
        assert!(!plausible_grip(&pose::mirror(&pose::from_axes(
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0; 3]
        ))));
    }

    #[test]
    fn frame_height_follows_the_texture_aspect() {
        let f = VrFrame {
            rgba: &[],
            width: 480,
            height: 240,
            changed: false,
            visible: true,
            alpha: 1.0,
            width_m: pose::BACK_WIDTH_M,
        };
        let (w, h) = frame_wh(&f);
        assert_eq!(w, pose::BACK_WIDTH_M);
        assert!((h - pose::BACK_WIDTH_M / 2.0).abs() < 1e-6);
        assert_eq!(frame_wh(&VrFrame { width: 0, ..f }).1, 0.0);
    }

    #[test]
    fn party_overlay_sits_beside_the_main_one_with_tops_aligned() {
        let main_h = |s: f32| WIDTH_METERS * s * 640.0 / 360.0;
        let party_h = |s: f32| WIDTH_METERS * s * 392.0 / 360.0;
        for scale in [0.5f32, 1.0, 1.5, 2.0] {
            let main_wh = (WIDTH_METERS * scale, main_h(scale));
            let party_wh = (WIDTH_METERS * scale, party_h(scale));
            let m = pose::OLD_TRANSFORM;
            let p = pose::party(
                &m,
                Place::Old,
                Hand::Left,
                main_wh,
                party_wh,
                pose::SIDE_GAP_M * party_wh.0 / WIDTH_METERS,
            );
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
