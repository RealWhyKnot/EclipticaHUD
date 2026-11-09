use super::pose::*;
use std::fmt::Write as _;

mod placement;
mod sizing;

const EYE: V3 = [0.0, 1.65, 0.0];
const UP: V3 = [0.0, 1.0, 0.0];
const FULL: (f32, f32) = (0.28, 0.28 * 640.0 / 360.0);
const BACK: (f32, f32) = (BACK_WIDTH_M, BACK_WIDTH_M * ASPECT);
const INSIDE: (f32, f32) = (INSIDE_WIDTH_M, INSIDE_WIDTH_M * ASPECT);
const LIMIT_DEG: f32 = 20.0;

fn size_of(place: Place) -> (f32, f32) {
    match place {
        Place::Old => FULL,
        Place::Back => BACK,
        Place::Inside => INSIDE,
    }
}

#[derive(Clone, Copy)]
pub(super) struct Arm {
    pub name: &'static str,
    pub wrist: V3,
    pub forearm: V3,
    pub face_palm: bool,
}

pub(super) const WATCH: Arm = Arm {
    name: "watch check, palm down",
    wrist: [-0.02, 1.32, -0.30],
    forearm: [-0.80, -0.30, 0.52],
    face_palm: false,
};
pub(super) const WATCH_HIGH: Arm = Arm {
    name: "watch check, forearm raised",
    wrist: [-0.03, 1.40, -0.30],
    forearm: [-0.72, -0.55, 0.42],
    face_palm: false,
};
pub(super) const WATCH_LOW: Arm = Arm {
    name: "watch check, hand low",
    wrist: [0.0, 1.20, -0.32],
    forearm: [-0.85, -0.10, 0.52],
    face_palm: false,
};
pub(super) const FACE: Arm = Arm {
    name: "palm toward face",
    wrist: [-0.05, 1.42, -0.28],
    forearm: [-0.35, -0.80, 0.30],
    face_palm: true,
};

pub(super) const FACE_LOW: Arm = Arm {
    name: "palm toward face, hand low",
    wrist: [-0.04, 1.30, -0.30],
    forearm: [-0.55, -0.45, 0.55],
    face_palm: true,
};

pub(super) const WATCHES: [Arm; 3] = [WATCH, WATCH_HIGH, WATCH_LOW];
pub(super) const FACES: [Arm; 2] = [FACE, FACE_LOW];

pub(super) const FAMILIES: [(&str, (V3, V3)); 3] = [
    ("Index", INDEX_GRIP),
    ("Touch", TOUCH_GRIP),
    ("Vive", VIVE_GRIP),
];

pub(super) fn arm_for(arm: Arm, hand: Hand) -> Arm {
    match hand {
        Hand::Left => arm,
        Hand::Right => Arm {
            wrist: [-arm.wrist[0], arm.wrist[1], arm.wrist[2]],
            forearm: [-arm.forearm[0], arm.forearm[1], arm.forearm[2]],
            ..arm
        },
    }
}

fn roll_basis(a: V3) -> (V3, V3) {
    let u1 = norm(sub(UP, scale(a, dot(UP, a))));
    (u1, cross(a, u1))
}

pub(super) fn back_normal(arm: Arm, roll: f32) -> V3 {
    let a = norm(arm.forearm);
    let (u1, u2) = roll_basis(a);
    let (s, c) = roll.to_radians().sin_cos();
    add(scale(u1, c), scale(u2, s))
}

pub(super) fn grip_world(arm: Arm, hand: Hand, roll: f32, tilt_deg: f32) -> M34 {
    let along = norm(arm.forearm);
    let dn = back_normal(arm, roll);
    let x_axis = scale(dn, dorsal(hand)[0]);
    let side = cross(along, x_axis);
    let (sb, cb) = tilt_deg.to_radians().sin_cos();
    let y_axis = add(scale(along, cb), scale(side, sb));
    let z_axis = sub(scale(along, sb), scale(side, cb));
    let origin = sub(
        arm.wrist,
        add(scale(along, WRIST_ALONG_M), scale(dn, WRIST_DORSAL_M)),
    );
    from_axes(x_axis, y_axis, z_axis, origin)
}

pub(super) fn panel_world(
    place: Place,
    hand: Hand,
    family: (V3, V3),
    arm: Arm,
    roll: f32,
    wh: (f32, f32),
) -> M34 {
    let g = grip_world(arm, hand, roll, FOREARM_TILT_DEG);
    let grip = for_hand(&component(family.0, family.1), hand);
    let raw = mul(&g, &invert(&grip));
    mul(&raw, &overlay_transform(place, hand, &grip, wh.0, wh.1))
}

pub(super) struct View {
    pub facing: f32,
    pub roll: f32,
    pub dist: f32,
}

pub(super) fn view(panel: &M34) -> View {
    let c = origin(panel);
    let v = sub(EYE, c);
    let vh = norm(v);
    let n = axis(panel, 2);
    let u = axis(panel, 1);
    let up_flat = norm(sub(UP, scale(vh, dot(UP, vh))));
    let side_flat = norm(sub(u, scale(vh, dot(u, vh))));
    let roll = dot(cross(up_flat, side_flat), vh)
        .atan2(dot(up_flat, side_flat))
        .to_degrees();
    View {
        facing: angle_deg(n, v),
        roll,
        dist: dot(v, v).sqrt(),
    }
}

fn sweep(f: impl Fn(f32) -> f32) -> (f32, f32) {
    let mut best = (0.0, f32::MAX);
    let mut r = -180.0;
    while r < 180.0 {
        let v = f(r);
        if v < best.1 {
            best = (r, v);
        }
        r += 0.25;
    }
    best
}

pub(super) fn natural_roll(arm: Arm) -> f32 {
    let a = norm(arm.forearm);
    let skin = if arm.face_palm { -1.0 } else { 1.0 };
    sweep(|r| {
        let dn = back_normal(arm, r);
        let p = add(add(arm.wrist, scale(a, 0.04)), scale(dn, 0.025 * skin));
        angle_deg(scale(dn, skin), sub(EYE, p))
    })
    .0
}

pub(super) fn wrap(d: f32) -> f32 {
    let mut d = d % 360.0;
    if d > 180.0 {
        d -= 360.0;
    }
    if d < -180.0 {
        d += 360.0;
    }
    d
}

pub(super) struct Case {
    pub natural: View,
    pub best_roll_delta: f32,
    pub best_facing: f32,
    pub pronation: f32,
}

pub(super) fn case(place: Place, hand: Hand, family: (V3, V3), arm: Arm, wh: (f32, f32)) -> Case {
    let arm = arm_for(arm, hand);
    let nat = natural_roll(arm);
    let natural = view(&panel_world(place, hand, family, arm, nat, wh));
    let (best, best_facing) = sweep(|r| view(&panel_world(place, hand, family, arm, r, wh)).facing);
    let delta = wrap(best - nat);
    let pronation = if hand == Hand::Left { -delta } else { delta };
    Case {
        natural,
        best_roll_delta: delta,
        best_facing,
        pronation,
    }
}

fn det(m: &M34) -> f32 {
    dot(axis(m, 0), cross(axis(m, 1), axis(m, 2)))
}

fn assert_rigid(m: &M34) {
    assert!((det(m) - 1.0).abs() < 1e-4, "det {}", det(m));
    for i in 0..3 {
        for j in 0..3 {
            let want = if i == j { 1.0 } else { 0.0 };
            assert!((dot(axis(m, i), axis(m, j)) - want).abs() < 1e-4);
        }
    }
}

fn corners(m: &M34, wh: (f32, f32)) -> [V3; 4] {
    let (w, h) = (wh.0 / 2.0, wh.1 / 2.0);
    [(-w, -h), (w, -h), (w, h), (-w, h)].map(|(x, y)| point(m, [x, y, 0.0]))
}

pub(super) const ASPECT: f32 = 640.0 / 360.0;
pub(super) const SMALL_TEXT_PX: f32 = 11.0;
pub(super) const CAP_EM: f32 = 0.70;
pub(super) const TEXT_MIN_DEG: f32 = 8.0 / 15.0;
pub(super) const FOREARM_M: f32 = 0.25;
pub(super) const VIEW_MAX_DEG: f32 = 25.0;
pub(super) const EYE_MIN_M: f32 = 0.25;
pub(super) const FACE_MAX_DEG: f32 = 15.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct Fit {
    pub facing: f32,
    pub roll: f32,
    pub text_deg: f32,
    pub forearm: f32,
    pub view_deg: f32,
    pub eye_m: f32,
}

impl Fit {
    pub(super) fn margins(&self) -> [f32; 5] {
        [
            1.0 - self.facing.max(self.roll) / FACE_MAX_DEG,
            self.text_deg / TEXT_MIN_DEG - 1.0,
            1.0 - self.forearm / FOREARM_M,
            1.0 - self.view_deg / VIEW_MAX_DEG,
            self.eye_m / EYE_MIN_M - 1.0,
        ]
    }

    pub(super) fn score(&self) -> f32 {
        self.margins().into_iter().fold(f32::MAX, f32::min)
    }
}

fn nearest_on_panel(m: &M34, wh: (f32, f32), p: V3) -> V3 {
    let rel = sub(p, origin(m));
    let x = dot(rel, axis(m, 0)).clamp(-wh.0 / 2.0, wh.0 / 2.0);
    let y = dot(rel, axis(m, 1)).clamp(-wh.1 / 2.0, wh.1 / 2.0);
    point(m, [x, y, 0.0])
}

pub(super) fn fit(place: Place, arms: &[(Arm, f32)], w: f32, tilt: f32, spin: f32) -> Fit {
    let wh = (w, w * ASPECT);
    let local = panel_in_grip_with(place, Hand::Left, wh.0, wh.1, tilt, spin);
    let mut f = fit_world(arms, wh, |g| mul(g, &local));
    f.forearm = corners(&local, wh)
        .iter()
        .map(|c| dot(sub(*c, wrist(Hand::Left)), forearm()))
        .fold(0.0, f32::max);
    f
}

pub(super) fn fit_world(arms: &[(Arm, f32)], wh: (f32, f32), panel: impl Fn(&M34) -> M34) -> Fit {
    let cap = CAP_EM * SMALL_TEXT_PX * wh.0 / 360.0;
    let mut fit = Fit {
        facing: 0.0,
        roll: 0.0,
        text_deg: f32::MAX,
        forearm: 0.0,
        view_deg: 0.0,
        eye_m: f32::MAX,
    };
    for &(arm, nat) in arms {
        let grip = grip_world(arm, Hand::Left, nat, FOREARM_TILT_DEG);
        let placed = panel(&grip);
        let seen = view(&placed);
        fit.facing = fit.facing.max(seen.facing);
        fit.roll = fit.roll.max(seen.roll.abs());
        let normal = axis(&placed, 2);
        let to_c = sub(origin(&placed), EYE);
        for corner in corners(&placed, wh) {
            let to_eye = sub(EYE, corner);
            let distance = dot(to_eye, to_eye).sqrt();
            let slant = angle_deg(normal, to_eye).to_radians().cos().max(0.0);
            fit.text_deg = fit
                .text_deg
                .min((cap * slant / distance).atan().to_degrees());
            fit.view_deg = fit.view_deg.max(angle_deg(to_c, sub(corner, EYE)));
        }
        let near = sub(nearest_on_panel(&placed, wh, EYE), EYE);
        fit.eye_m = fit.eye_m.min(dot(near, near).sqrt());
    }
    fit
}

pub(super) fn arms_for(place: Place) -> Vec<(Arm, f32)> {
    let arms: &[Arm] = match place {
        Place::Inside => &FACES,
        _ => &WATCHES,
    };
    arms.iter().map(|&a| (a, natural_roll(a))).collect()
}

pub(super) fn best_angles(
    place: Place,
    arms: &[(Arm, f32)],
    w: f32,
    step: i32,
    around: Option<(f32, f32, i32)>,
) -> (f32, f32, Fit) {
    let (t_range, s_range) = match around {
        Some((t, s, r)) => (
            ((crate::num::to_i32(t) - r).max(0)..=(crate::num::to_i32(t) + r))
                .step_by(crate::num::to_usize(step)),
            ((crate::num::to_i32(s) - r)..=(crate::num::to_i32(s) + r))
                .step_by(crate::num::to_usize(step)),
        ),
        None => (
            (0..=75).step_by(crate::num::to_usize(step)),
            (-90..=90).step_by(crate::num::to_usize(step)),
        ),
    };
    let mut best: Option<(f32, f32, Fit)> = None;
    for t in t_range {
        for s in s_range.clone() {
            let f = fit(place, arms, w, crate::num::to_f32(t), crate::num::to_f32(s));
            if best.as_ref().is_none_or(|b| f.score() > b.2.score()) {
                best = Some((crate::num::to_f32(t), crate::num::to_f32(s), f));
            }
        }
    }
    best.unwrap()
}

const SHOULDER: V3 = [-0.19, 1.50, 0.02];
const PARTY_ASPECT: f32 = 420.0 / 360.0;

fn segment_distance(from: V3, start: V3, end: V3) -> f32 {
    let ab = sub(end, start);
    let along = (dot(sub(from, start), ab) / dot(ab, ab)).clamp(0.0, 1.0);
    let offset = sub(from, add(start, scale(ab, along)));
    dot(offset, offset).sqrt()
}

fn party_options(main: &M34, wh: (f32, f32)) -> [(&'static str, M34); 4] {
    let pw = (wh.0, wh.0 * PARTY_ASPECT);
    let gap = SIDE_GAP_M * wh.0 / 0.28;
    let dy = f32::midpoint(wh.1, pw.1) + gap;
    [
        ("toward elbow", beside(main, -1.0, wh, pw, gap)),
        ("toward hand", beside(main, 1.0, wh, pw, gap)),
        ("above", shifted(main, 0.0, dy)),
        ("below", shifted(main, 0.0, -dy)),
    ]
}

#[derive(Debug)]
struct PartyFit {
    facing: f32,
    off_deg: f32,
    eye_m: f32,
    reach: (f32, f32),
    clear_m: f32,
}

fn party_fit(place: Place, hand: Hand, mut side: impl FnMut(&M34, (f32, f32)) -> M34) -> PartyFit {
    let width = panel_width(place, 0.28);
    let wh = (width, width * ASPECT);
    let pw = (width, width * PARTY_ASPECT);
    let arms: &[Arm] = if place == Place::Inside {
        &FACES
    } else {
        &WATCHES
    };
    let shoulder = match hand {
        Hand::Left => SHOULDER,
        Hand::Right => [-SHOULDER[0], SHOULDER[1], SHOULDER[2]],
    };
    let mut fit = PartyFit {
        facing: 0.0,
        off_deg: 0.0,
        eye_m: f32::MAX,
        reach: (f32::MAX, f32::MIN),
        clear_m: f32::MAX,
    };
    for arm in arms {
        let arm = arm_for(*arm, hand);
        let grip = grip_world(arm, hand, natural_roll(arm), FOREARM_TILT_DEG);
        let main = mul(&grip, &panel_in_grip(place, hand, wh.0, wh.1));
        let placed = side(&main, wh);
        let along = norm(arm.forearm);
        let elbow = add(arm.wrist, scale(along, 0.26));
        let hand_end = sub(arm.wrist, scale(along, 0.18));
        fit.facing = fit.facing.max(view(&placed).facing);
        let to_main = sub(origin(&main), EYE);
        for corner in corners(&placed, pw) {
            fit.off_deg = fit.off_deg.max(angle_deg(to_main, sub(corner, EYE)));
            let along_m = dot(sub(corner, arm.wrist), along);
            fit.reach = (fit.reach.0.min(along_m), fit.reach.1.max(along_m));
        }
        for column in 0..=20 {
            for row in 0..=20 {
                let sample = point(
                    &placed,
                    [
                        (crate::num::to_f32(column) / 20.0 - 0.5) * pw.0,
                        (crate::num::to_f32(row) / 20.0 - 0.5) * pw.1,
                        0.0,
                    ],
                );
                fit.clear_m = fit
                    .clear_m
                    .min(segment_distance(sample, elbow, shoulder) - 0.05)
                    .min(segment_distance(sample, arm.wrist, elbow) - 0.035)
                    .min(segment_distance(sample, arm.wrist, hand_end) - 0.04);
            }
        }
        let near = sub(nearest_on_panel(&placed, pw, EYE), EYE);
        fit.eye_m = fit.eye_m.min(dot(near, near).sqrt());
    }
    fit
}

fn v3(p: V3) -> String {
    format!("[{:.4},{:.4},{:.4}]", p[0], p[1], p[2])
}
