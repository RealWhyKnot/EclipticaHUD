use super::pose::Hand;
use std::collections::VecDeque;
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Alert {
    TargetYou,
    Tokens,
    BossDown,
    StageClear,
}

pub const ALERTS: [Alert; 4] = [
    Alert::TargetYou,
    Alert::Tokens,
    Alert::BossDown,
    Alert::StageClear,
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pulse {
    pub at: f32,
    pub secs: f32,
    pub amp: f32,
}

pub const FREQ_HZ: f32 = 160.0;
pub const MAX_PULSE_SECS: f32 = 0.15;
pub const MAX_PATTERN_SECS: f32 = 0.4;
pub const GAP_SECS: f64 = 2.0;
pub const SAME_GAP_SECS: f64 = 5.0;
pub const PER_MINUTE: usize = 6;

const fn p(at: f32, secs: f32, amp: f32) -> Pulse {
    Pulse { at, secs, amp }
}

const TARGET_YOU: [Pulse; 2] = [p(0.0, 0.08, 0.8), p(0.15, 0.08, 0.8)];
const TOKENS: [Pulse; 3] = [p(0.0, 0.06, 0.6), p(0.12, 0.06, 0.6), p(0.24, 0.06, 0.6)];
const BOSS_DOWN: [Pulse; 2] = [p(0.0, 0.12, 0.5), p(0.2, 0.12, 0.5)];
const STAGE_CLEAR: [Pulse; 1] = [p(0.0, 0.15, 0.4)];

pub fn pattern(a: Alert) -> &'static [Pulse] {
    match a {
        Alert::TargetYou => &TARGET_YOU,
        Alert::Tokens => &TOKENS,
        Alert::BossDown => &BOSS_DOWN,
        Alert::StageClear => &STAGE_CLEAR,
    }
}

fn slot(a: Alert) -> usize {
    ALERTS.iter().position(|&x| x == a).unwrap_or(0)
}

#[derive(Default)]
pub struct Limiter {
    last: Option<f64>,
    last_kind: [Option<f64>; 4],
    recent: VecDeque<f64>,
}

impl Limiter {
    pub fn allow(&mut self, a: Alert, now: f64) -> bool {
        while self.recent.front().is_some_and(|&t| now - t >= 60.0) {
            self.recent.pop_front();
        }
        let i = slot(a);
        let blocked = self.last.is_some_and(|t| now - t < GAP_SECS)
            || self.last_kind[i].is_some_and(|t| now - t < SAME_GAP_SECS)
            || self.recent.len() >= PER_MINUTE;
        if blocked {
            return false;
        }
        self.last = Some(now);
        self.last_kind[i] = Some(now);
        self.recent.push_back(now);
        true
    }
}

#[derive(Default, Clone, PartialEq, Debug)]
pub struct Seen {
    pub target_since: u64,
    pub target_me: bool,
    pub token_alerts: u64,
    pub clear: Option<u64>,
    pub kill: Option<u64>,
}

pub fn alerts(prev: &Seen, cur: &Seen) -> Vec<Alert> {
    let mut out = Vec::new();
    if cur.target_me && (cur.target_since != prev.target_since || !prev.target_me) {
        out.push(Alert::TargetYou);
    }
    if cur.token_alerts > prev.token_alerts {
        out.push(Alert::Tokens);
    }
    if cur.kill.is_some() && cur.kill != prev.kill {
        out.push(Alert::BossDown);
    }
    if cur.clear.is_some() && cur.clear != prev.clear {
        out.push(Alert::StageClear);
    }
    out
}

const SET: &str = "/actions/hud";
const LEFT: &str = "/actions/hud/out/buzz_left";
const RIGHT: &str = "/actions/hud/out/buzz_right";
const MANIFEST: &str = "actions.json";
const CONTROLLERS: [&str; 10] = [
    "knuckles",
    "oculus_touch",
    "vive_controller",
    "pico_controller",
    "vive_focus3_controller",
    "vive_cosmos_controller",
    "holographic_controller",
    "hpmotioncontroller",
    "frame_controller",
    "generic",
];

pub fn manifest() -> String {
    let defaults: Vec<String> = CONTROLLERS
        .iter()
        .map(|c| format!("{{\"controller_type\":\"{c}\",\"binding_url\":\"bindings_{c}.json\"}}"))
        .collect();
    format!(
        "{{\"default_bindings\":[{}],\"actions\":[{{\"name\":\"{LEFT}\",\"type\":\"vibration\"}},{{\"name\":\"{RIGHT}\",\"type\":\"vibration\"}}],\"action_sets\":[{{\"name\":\"{SET}\",\"usage\":\"leftright\"}}],\"localization\":[{{\"language_tag\":\"en_US\",\"{SET}\":\"Ecliptica HUD\",\"{LEFT}\":\"Buzz left hand\",\"{RIGHT}\":\"Buzz right hand\"}}]}}",
        defaults.join(",")
    )
}

pub fn bindings(controller: &str) -> String {
    format!(
        "{{\"controller_type\":\"{controller}\",\"name\":\"Ecliptica HUD alerts\",\"description\":\"Buzzes the hand that wears the HUD\",\"bindings\":{{\"{SET}\":{{\"haptics\":[{{\"output\":\"{LEFT}\",\"path\":\"/user/hand/left/output/haptic\"}},{{\"output\":\"{RIGHT}\",\"path\":\"/user/hand/right/output/haptic\"}}]}}}}}}"
    )
}

fn write_files(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for c in CONTROLLERS {
        std::fs::write(dir.join(format!("bindings_{c}.json")), bindings(c))?;
    }
    std::fs::write(dir.join(MANIFEST), manifest())
}

pub struct Haptics {
    set: openvr::input::VRActionSetHandle,
    left: openvr::input::VRActionHandle,
    right: openvr::input::VRActionHandle,
}

impl Haptics {
    pub fn init(context: &openvr::Context, dir: &Path) -> Option<Haptics> {
        write_files(dir).ok()?;
        let mut input = context.input().ok()?;
        input.set_action_manifest(&dir.join(MANIFEST)).ok()?;
        Some(Haptics {
            set: input.get_action_set_handle(SET).ok()?,
            left: input.get_action_handle(LEFT).ok()?,
            right: input.get_action_handle(RIGHT).ok()?,
        })
    }

    pub fn fire(&self, context: &openvr::Context, hand: Hand, a: Alert) {
        let Ok(mut input) = context.input() else {
            return;
        };
        let mut active: openvr::input::VRActiveActionSet = unsafe { std::mem::zeroed() };
        active.0.ulActionSet = self.set.0;
        let _ = input.update_actions(std::slice::from_mut(&mut active));
        let action = match hand {
            Hand::Left => self.left,
            Hand::Right => self.right,
        };
        for pulse in pattern(a).iter().filter(|p| p.at < MAX_PATTERN_SECS) {
            let _ = input.trigger_haptic_vibration_action(
                action,
                pulse.at,
                Duration::from_secs_f32(pulse.secs.clamp(0.0, MAX_PULSE_SECS)),
                FREQ_HZ,
                pulse.amp.clamp(0.0, 1.0),
                openvr::input::VRInputValueHandle(0),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulses_stay_short() {
        for a in ALERTS {
            let ps = pattern(a);
            assert!(!ps.is_empty());
            let mut end = 0.0f32;
            for (i, p) in ps.iter().enumerate() {
                assert!(p.secs > 0.0 && p.secs <= MAX_PULSE_SECS, "{a:?}");
                assert!(p.amp > 0.0 && p.amp <= 1.0, "{a:?}");
                assert!(p.at >= end, "{a:?} pulse {i} overlaps");
                end = p.at + p.secs;
            }
            assert!(end <= MAX_PATTERN_SECS, "{a:?} {end}");
        }
    }

    #[test]
    fn patterns_feel_different() {
        for (i, a) in ALERTS.iter().enumerate() {
            for b in &ALERTS[i + 1..] {
                assert_ne!(pattern(*a), pattern(*b), "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn limiter_spaces_out_buzzes() {
        let mut l = Limiter::default();
        assert!(l.allow(Alert::Tokens, 0.0));
        assert!(!l.allow(Alert::TargetYou, 1.0));
        assert!(l.allow(Alert::TargetYou, 2.0));
        assert!(!l.allow(Alert::Tokens, 4.5));
        assert!(l.allow(Alert::Tokens, 5.0));
    }

    #[test]
    fn limiter_caps_buzzes_per_minute() {
        let mut l = Limiter::default();
        let mut fired = Vec::new();
        let mut t = 0.0;
        while t < 120.0 {
            let a = ALERTS[(t as usize / 2) % 4];
            if l.allow(a, t) {
                fired.push(t);
            }
            t += 0.5;
        }
        for (i, &t0) in fired.iter().enumerate() {
            let in_window = fired[i..].iter().take_while(|&&t| t - t0 < 60.0).count();
            assert!(in_window <= PER_MINUTE, "{t0} {in_window}");
        }
        for w in fired.windows(2) {
            assert!(w[1] - w[0] >= GAP_SECS);
        }
        assert_eq!(fired.len(), 2 * PER_MINUTE);
    }

    #[test]
    fn limiter_recovers_after_a_quiet_minute() {
        let mut l = Limiter::default();
        for i in 0..PER_MINUTE {
            assert!(l.allow(ALERTS[i % 4], i as f64 * 5.0));
        }
        assert!(!l.allow(Alert::TargetYou, 40.0));
        assert!(l.allow(Alert::TargetYou, 60.0));
    }

    fn seen() -> Seen {
        Seen {
            target_since: 10,
            target_me: false,
            token_alerts: 0,
            clear: None,
            kill: None,
        }
    }

    #[test]
    fn nothing_changed_means_no_buzz() {
        assert!(alerts(&seen(), &seen()).is_empty());
        let me = Seen {
            target_me: true,
            ..seen()
        };
        assert!(alerts(&me, &me).is_empty());
    }

    #[test]
    fn boss_turning_on_you_buzzes() {
        let cur = Seen {
            target_since: 12,
            target_me: true,
            ..seen()
        };
        assert_eq!(alerts(&seen(), &cur), vec![Alert::TargetYou]);
        let other = Seen {
            target_since: 12,
            ..seen()
        };
        assert!(alerts(&seen(), &other).is_empty());
    }

    #[test]
    fn each_game_event_maps_to_its_alert() {
        let tokens = Seen {
            token_alerts: 1,
            ..seen()
        };
        assert_eq!(alerts(&seen(), &tokens), vec![Alert::Tokens]);
        let kill = Seen {
            kill: Some(50),
            ..seen()
        };
        assert_eq!(alerts(&seen(), &kill), vec![Alert::BossDown]);
        let clear = Seen {
            clear: Some(70),
            ..seen()
        };
        assert_eq!(alerts(&seen(), &clear), vec![Alert::StageClear]);
        let cleared_again = Seen {
            clear: None,
            ..clear.clone()
        };
        assert!(alerts(&clear, &cleared_again).is_empty());
    }

    #[test]
    fn most_urgent_alert_comes_first() {
        let all = Seen {
            target_since: 99,
            target_me: true,
            token_alerts: 3,
            clear: Some(1),
            kill: Some(2),
        };
        assert_eq!(alerts(&seen(), &all), ALERTS.to_vec());
    }

    #[test]
    fn manifest_names_both_hands_and_every_binding_file() {
        let m = manifest();
        for c in CONTROLLERS {
            assert!(m.contains(&format!("bindings_{c}.json")));
            let b = bindings(c);
            assert!(b.contains("/user/hand/left/output/haptic"));
            assert!(b.contains("/user/hand/right/output/haptic"));
            assert!(b.contains(LEFT) && b.contains(RIGHT));
        }
        assert!(m.contains("\"vibration\""));
    }

    #[test]
    fn manifest_files_land_on_disk() {
        let dir = std::env::temp_dir().join(format!("ehud-buzz-test-{}", std::process::id()));
        write_files(&dir).unwrap();
        let m = std::fs::read_to_string(dir.join(MANIFEST)).unwrap();
        assert_eq!(m, manifest());
        for c in CONTROLLERS {
            let path = dir.join(format!("bindings_{c}.json"));
            assert_eq!(std::fs::read_to_string(&path).unwrap(), bindings(c));
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_file(dir.join(MANIFEST)).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
}
