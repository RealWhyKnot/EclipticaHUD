use super::pose::*;

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
    let a = norm(arm.forearm);
    let dn = back_normal(arm, roll);
    let x = scale(dn, dorsal(hand)[0]);
    let w = cross(a, x);
    let (sb, cb) = tilt_deg.to_radians().sin_cos();
    let y = add(scale(a, cb), scale(w, sb));
    let z = sub(scale(a, sb), scale(w, cb));
    let o = sub(
        arm.wrist,
        add(scale(a, WRIST_ALONG_M), scale(dn, WRIST_DORSAL_M)),
    );
    from_axes(x, y, z, o)
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
    let up_p = norm(sub(UP, scale(vh, dot(UP, vh))));
    let u_p = norm(sub(u, scale(vh, dot(u, vh))));
    let roll = dot(cross(up_p, u_p), vh).atan2(dot(up_p, u_p)).to_degrees();
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

#[test]
fn every_transform_is_a_proper_rotation() {
    for place in [Place::Old, Place::Back, Place::Inside] {
        for hand in [Hand::Left, Hand::Right] {
            for (_, fam) in FAMILIES {
                let grip = for_hand(&component(fam.0, fam.1), hand);
                assert_rigid(&grip);
                for wh in [FULL, BACK, INSIDE] {
                    assert_rigid(&overlay_transform(place, hand, &grip, wh.0, wh.1));
                }
            }
        }
    }
}

#[test]
fn component_rotation_matches_the_render_model_meshes() {
    let g = component(TOUCH_GRIP.0, TOUCH_GRIP.1);
    let up_handle = scale(axis(&g, 2), -1.0);
    assert!((up_handle[1] - 20.6f32.to_radians().sin()).abs() < 1e-4);
    assert!((up_handle[2] + 20.6f32.to_radians().cos()).abs() < 1e-4);
    let back = invert(&g);
    let p = point(&back, point(&g, [0.01, 0.02, 0.03]));
    assert!(sub(p, [0.01, 0.02, 0.03]).iter().all(|d| d.abs() < 1e-5));
}

#[test]
fn old_panel_cannot_face_the_eye_from_a_watch_check() {
    for hand in [Hand::Left, Hand::Right] {
        for (name, fam) in FAMILIES {
            for arm in [WATCH, WATCH_HIGH, WATCH_LOW] {
                let c = case(Place::Old, hand, fam, arm, FULL);
                assert!(
                    c.natural.facing > 80.0,
                    "{name} {} natural {}",
                    arm.name,
                    c.natural.facing
                );
                assert!(
                    c.best_facing > 50.0,
                    "{name} {} best {}",
                    arm.name,
                    c.best_facing
                );
            }
        }
    }
}

#[test]
fn back_panel_faces_the_eye_from_a_watch_check() {
    for hand in [Hand::Left, Hand::Right] {
        for (name, fam) in FAMILIES {
            for arm in WATCHES {
                let c = case(Place::Back, hand, fam, arm, BACK);
                assert!(
                    c.natural.facing < LIMIT_DEG,
                    "{name} {} {}",
                    arm.name,
                    c.natural.facing
                );
                assert!(
                    c.natural.roll.abs() < LIMIT_DEG,
                    "{name} {} roll {}",
                    arm.name,
                    c.natural.roll
                );
                assert!(c.best_roll_delta.abs() < LIMIT_DEG, "{name} {}", arm.name);
            }
        }
    }
}

#[test]
fn inside_panel_faces_the_eye_with_the_palm_toward_the_face() {
    for hand in [Hand::Left, Hand::Right] {
        for (name, fam) in FAMILIES {
            for arm in FACES {
                let c = case(Place::Inside, hand, fam, arm, INSIDE);
                assert!(c.natural.facing < LIMIT_DEG, "{name} {}", c.natural.facing);
                assert!(
                    c.natural.roll.abs() < LIMIT_DEG,
                    "{name} roll {}",
                    c.natural.roll
                );
                assert!(c.best_roll_delta.abs() < LIMIT_DEG, "{name}");
            }
        }
    }
}

#[test]
fn right_hand_is_the_mirror_of_the_left() {
    for place in [Place::Old, Place::Back, Place::Inside] {
        for (name, fam) in FAMILIES {
            for arm in [WATCH, FACE] {
                let l = case(place, Hand::Left, fam, arm, size_of(place));
                let r = case(place, Hand::Right, fam, arm, size_of(place));
                assert!((l.natural.facing - r.natural.facing).abs() < 0.5, "{name}");
                assert!((l.natural.roll + r.natural.roll).abs() < 0.5, "{name}");
                assert!((l.pronation - r.pronation).abs() < 0.6, "{name}");
            }
        }
    }
}

#[test]
fn new_placement_does_not_depend_on_the_controller() {
    let nat = natural_roll(WATCH);
    let base = panel_world(Place::Back, Hand::Left, TOUCH_GRIP, WATCH, nat, BACK);
    for (_, fam) in FAMILIES {
        let p = panel_world(Place::Back, Hand::Left, fam, WATCH, nat, BACK);
        let d = sub(origin(&p), origin(&base));
        assert!(angle_deg(axis(&p, 2), axis(&base, 2)) < 0.05);
        assert!(dot(d, d) < 1e-8);
    }
}

#[test]
fn wrong_controller_guess_stays_within_a_few_degrees() {
    let nat = natural_roll(WATCH);
    for (name, real) in FAMILIES {
        let g = grip_world(WATCH, Hand::Left, nat, FOREARM_TILT_DEG);
        let raw = mul(&g, &invert(&component(real.0, real.1)));
        let guess = component(TOUCH_GRIP.0, TOUCH_GRIP.1);
        let p = mul(
            &raw,
            &overlay_transform(Place::Back, Hand::Left, &guess, BACK.0, BACK.1),
        );
        let v = view(&p);
        assert!(v.facing < 25.0, "{name} {}", v.facing);
    }
}

fn corners(m: &M34, wh: (f32, f32)) -> [V3; 4] {
    let (w, h) = (wh.0 / 2.0, wh.1 / 2.0);
    [(-w, -h), (w, -h), (w, h), (-w, h)].map(|(x, y)| point(m, [x, y, 0.0]))
}

#[test]
fn panel_keeps_clear_of_the_wrist_and_off_the_hand() {
    for place in [Place::Back, Place::Inside] {
        for hand in [Hand::Left, Hand::Right] {
            for wh in [BACK, INSIDE, FULL] {
                let m = panel_in_grip(place, hand, wh.0, wh.1);
                let out = if place == Place::Back {
                    dorsal(hand)
                } else {
                    scale(dorsal(hand), -1.0)
                };
                let mut lowest = f32::MAX;
                let mut nearest = f32::MAX;
                for p in corners(&m, wh) {
                    let rel = sub(p, wrist(hand));
                    lowest = lowest.min(dot(rel, out));
                    nearest = nearest.min(dot(rel, forearm()));
                }
                assert!(
                    (lowest - (SKIN_M + CLEAR_M)).abs() < 1e-4,
                    "{place:?} {lowest}"
                );
                assert!((nearest - WRIST_GAP_M).abs() < 1e-4, "{place:?} {nearest}");
            }
        }
    }
}

#[test]
fn back_panel_reads_left_to_right_toward_the_hand() {
    for hand in [Hand::Left, Hand::Right] {
        let m = panel_in_grip_with(Place::Back, hand, BACK.0, BACK.1, 0.0, 0.0);
        let toward_hand = dot(axis(&m, 0), scale(forearm(), -1.0));
        let want = if hand == Hand::Left { 1.0 } else { -1.0 };
        assert!((toward_hand - want).abs() < 1e-5);
    }
}

#[test]
fn party_panel_keeps_its_old_spot_with_the_old_transform() {
    let main_h = |s: f32| 0.28 * s * 640.0 / 360.0;
    let party_h = |s: f32| 0.28 * s * 392.0 / 360.0;
    for s in [0.5f32, 1.0, 1.5, 2.0] {
        let m = OLD_TRANSFORM;
        let p = party(
            &m,
            Place::Old,
            Hand::Left,
            (0.28 * s, main_h(s)),
            (0.28 * s, party_h(s)),
            SIDE_GAP_M * s,
        );
        assert!((p[0][3] - 0.30 * s).abs() < 1e-5, "{s}");
        assert!((p[2][3] - (0.10 + 0.0965 * s)).abs() < 1e-3, "{s}");
        for r in 0..3 {
            for c in 0..3 {
                assert_eq!(m[r][c], p[r][c]);
            }
        }
    }
}

#[test]
#[ignore]
fn print_comfort_table() {
    let mut out = String::new();
    let arms = [WATCH, WATCH_HIGH, WATCH_LOW, FACE, FACE_LOW];
    for (place, wh) in [
        (Place::Old, FULL),
        (Place::Back, BACK),
        (Place::Inside, INSIDE),
    ] {
        for (name, fam) in FAMILIES {
            for arm in arms {
                for hand in [Hand::Left, Hand::Right] {
                    let c = case(place, hand, fam, arm, wh);
                    out.push_str(&format!(
                        "{place:?}\t{name}\t{hand:?}\t{}\tfacing {:.1}\timage roll {:.1}\tdist {:.2}\tbest facing {:.1}\textra pronation {:.1}\n",
                        arm.name,
                        c.natural.facing,
                        c.natural.roll,
                        c.natural.dist,
                        c.best_facing,
                        c.pronation
                    ));
                }
            }
        }
    }
    for t in [0.0f32, 5.0, 10.0, 15.0, 20.0, 25.0, 30.0] {
        let mut worst = 0.0f32;
        for arm in [WATCH, WATCH_HIGH, WATCH_LOW] {
            let nat = natural_roll(arm);
            let g = grip_world(arm, Hand::Left, nat, t);
            let p = mul(&g, &panel_in_grip(Place::Back, Hand::Left, BACK.0, BACK.1));
            worst = worst.max(view(&p).facing);
        }
        out.push_str(&format!(
            "true forearm tilt {t}\tworst back facing {worst:.1}\n"
        ));
    }
    println!("{out}");
    if let Some(dir) = std::env::var_os("EHUD_VR_OUT") {
        std::fs::write(std::path::Path::new(&dir).join("comfort_table.txt"), out).unwrap();
    }
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
    pub fn margins(&self) -> [f32; 5] {
        [
            1.0 - self.facing.max(self.roll) / FACE_MAX_DEG,
            self.text_deg / TEXT_MIN_DEG - 1.0,
            1.0 - self.forearm / FOREARM_M,
            1.0 - self.view_deg / VIEW_MAX_DEG,
            self.eye_m / EYE_MIN_M - 1.0,
        ]
    }

    pub fn score(&self) -> f32 {
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
    let mut f = Fit {
        facing: 0.0,
        roll: 0.0,
        text_deg: f32::MAX,
        forearm: 0.0,
        view_deg: 0.0,
        eye_m: f32::MAX,
    };
    for &(arm, nat) in arms {
        let g = grip_world(arm, Hand::Left, nat, FOREARM_TILT_DEG);
        let p = panel(&g);
        let v = view(&p);
        f.facing = f.facing.max(v.facing);
        f.roll = f.roll.max(v.roll.abs());
        let n = axis(&p, 2);
        let to_c = sub(origin(&p), EYE);
        for c in corners(&p, wh) {
            let to_eye = sub(EYE, c);
            let d = dot(to_eye, to_eye).sqrt();
            let slant = angle_deg(n, to_eye).to_radians().cos().max(0.0);
            f.text_deg = f.text_deg.min((cap * slant / d).atan().to_degrees());
            f.view_deg = f.view_deg.max(angle_deg(to_c, sub(c, EYE)));
        }
        let near = sub(nearest_on_panel(&p, wh, EYE), EYE);
        f.eye_m = f.eye_m.min(dot(near, near).sqrt());
    }
    f
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
            ((t as i32 - r).max(0)..=(t as i32 + r)).step_by(step as usize),
            ((s as i32 - r)..=(s as i32 + r)).step_by(step as usize),
        ),
        None => (
            (0..=75).step_by(step as usize),
            (-90..=90).step_by(step as usize),
        ),
    };
    let mut best: Option<(f32, f32, Fit)> = None;
    for t in t_range {
        for s in s_range.clone() {
            let f = fit(place, arms, w, t as f32, s as f32);
            if best.as_ref().is_none_or(|b| f.score() > b.2.score()) {
                best = Some((t as f32, s as f32, f));
            }
        }
    }
    best.unwrap()
}

#[test]
fn chosen_width_wins_the_width_sweep() {
    for (place, w, tilt, spin) in [
        (Place::Back, BACK_WIDTH_M, BACK_TILT_DEG, BACK_SPIN_DEG),
        (
            Place::Inside,
            INSIDE_WIDTH_M,
            INSIDE_TILT_DEG,
            INSIDE_SPIN_DEG,
        ),
    ] {
        let arms = arms_for(place);
        let chosen = fit(place, &arms, w, tilt, spin).score();
        let mut best = (0.0f32, f32::MIN);
        for i in 0..=24 {
            let sw = 0.08 + i as f32 * 0.01;
            let score = best_angles(place, &arms, sw, 3, None).2.score();
            if score > best.1 {
                best = (sw, score);
            }
        }
        assert!(
            best.0 > 0.085 && best.0 < 0.315,
            "{place:?} edge {}",
            best.0
        );
        assert!(
            (best.0 - w).abs() <= 0.0101,
            "{place:?} sweep {} chosen {w}",
            best.0
        );
        assert!(chosen >= best.1 - 1e-3, "{place:?} {chosen} < {}", best.1);
    }
}

#[test]
fn full_panel_keeps_to_the_forearm_and_off_the_face() {
    for (place, w, tilt, spin) in [
        (Place::Back, BACK_WIDTH_M, BACK_TILT_DEG, BACK_SPIN_DEG),
        (
            Place::Inside,
            INSIDE_WIDTH_M,
            INSIDE_TILT_DEG,
            INSIDE_SPIN_DEG,
        ),
    ] {
        let f = fit(place, &arms_for(place), w, tilt, spin);
        assert!(f.forearm < FOREARM_M + 0.002, "{place:?} {f:?}");
        assert!(f.eye_m > EYE_MIN_M, "{place:?} {f:?}");
        assert!(
            f.facing < LIMIT_DEG && f.roll < LIMIT_DEG,
            "{place:?} {f:?}"
        );
        assert!(f.text_deg > 0.4, "{place:?} {f:?}");
    }
}

#[test]
fn new_widths_ignore_the_desktop_scale_and_old_keeps_it() {
    for s in [0.5f32, 1.0, 2.0] {
        assert_eq!(panel_width(Place::Old, 0.28 * s), 0.28 * s);
        assert_eq!(panel_width(Place::Back, 0.28 * s), BACK_WIDTH_M);
        assert_eq!(panel_width(Place::Inside, 0.28 * s), INSIDE_WIDTH_M);
    }
}

#[test]
#[ignore]
fn print_width_sweep() {
    let mut out = String::new();
    for place in [Place::Back, Place::Inside] {
        let arms = arms_for(place);
        let old = fit_world(&arms, FULL, |g| {
            let grip = component(TOUCH_GRIP.0, TOUCH_GRIP.1);
            mul(&mul(g, &invert(&grip)), &OLD_TRANSFORM)
        });
        out.push_str(&format!(
            "{place:?} poses, old transform at 0.28 m\t{old:?}\tmargins {:?}\n",
            old.margins()
        ));
        let mut best: Option<(f32, f32, f32, Fit)> = None;
        let mut w = 0.08f32;
        while w <= 0.3201 {
            let (t, s, f) = best_angles(place, &arms, w, 3, None);
            let m = f.margins();
            out.push_str(&format!(
                "{place:?}\tw {w:.3}\ttilt {t}\tspin {s}\tfacing {:.1}\troll {:.1}\ttext {:.2} deg\tforearm {:.3} m\tview {:.1} deg\teye {:.3} m\tmargins {:.2} {:.2} {:.2} {:.2} {:.2}\tscore {:.3}\n",
                f.facing, f.roll, f.text_deg, f.forearm, f.view_deg, f.eye_m, m[0], m[1], m[2], m[3], m[4], f.score()
            ));
            if best.as_ref().is_none_or(|b| f.score() > b.3.score()) {
                best = Some((w, t, s, f));
            }
            w += 0.01;
        }
        let (w0, t0, s0, _) = best.unwrap();
        let mut fine: Option<(f32, f32, f32, Fit)> = None;
        let mut w = w0 - 0.01;
        while w <= w0 + 0.0101 {
            let (t, s, f) = best_angles(place, &arms, w, 1, Some((t0, s0, 4)));
            if fine.as_ref().is_none_or(|b| f.score() > b.3.score()) {
                fine = Some((w, t, s, f));
            }
            w += 0.0025;
        }
        let (w, t, s, f) = fine.unwrap();
        out.push_str(&format!(
            "{place:?} best\tw {w:.4}\ttilt {t}\tspin {s}\t{f:?}\tmargins {:?}\tscore {:.3}\n",
            f.margins(),
            f.score()
        ));
    }
    println!("{out}");
    if let Some(dir) = std::env::var_os("EHUD_VR_OUT") {
        std::fs::write(std::path::Path::new(&dir).join("width_sweep.txt"), out).unwrap();
    }
}

const SHOULDER: V3 = [-0.19, 1.50, 0.02];
const PARTY_ASPECT: f32 = 420.0 / 360.0;

fn segment_distance(p: V3, a: V3, b: V3) -> f32 {
    let ab = sub(b, a);
    let t = (dot(sub(p, a), ab) / dot(ab, ab)).clamp(0.0, 1.0);
    let d = sub(p, add(a, scale(ab, t)));
    dot(d, d).sqrt()
}

fn party_options(main: &M34, wh: (f32, f32)) -> [(&'static str, M34); 4] {
    let pw = (wh.0, wh.0 * PARTY_ASPECT);
    let gap = SIDE_GAP_M * wh.0 / 0.28;
    let dy = (wh.1 + pw.1) / 2.0 + gap;
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
    let w = panel_width(place, 0.28);
    let wh = (w, w * ASPECT);
    let pw = (w, w * PARTY_ASPECT);
    let arms: &[Arm] = if place == Place::Inside {
        &FACES
    } else {
        &WATCHES
    };
    let shoulder = match hand {
        Hand::Left => SHOULDER,
        Hand::Right => [-SHOULDER[0], SHOULDER[1], SHOULDER[2]],
    };
    let mut f = PartyFit {
        facing: 0.0,
        off_deg: 0.0,
        eye_m: f32::MAX,
        reach: (f32::MAX, f32::MIN),
        clear_m: f32::MAX,
    };
    for arm in arms {
        let arm = arm_for(*arm, hand);
        let g = grip_world(arm, hand, natural_roll(arm), FOREARM_TILT_DEG);
        let main = mul(&g, &panel_in_grip(place, hand, wh.0, wh.1));
        let p = side(&main, wh);
        let a = norm(arm.forearm);
        let elbow = add(arm.wrist, scale(a, 0.26));
        let hand_end = sub(arm.wrist, scale(a, 0.18));
        f.facing = f.facing.max(view(&p).facing);
        let to_main = sub(origin(&main), EYE);
        for c in corners(&p, pw) {
            f.off_deg = f.off_deg.max(angle_deg(to_main, sub(c, EYE)));
            let r = dot(sub(c, arm.wrist), a);
            f.reach = (f.reach.0.min(r), f.reach.1.max(r));
        }
        for k in 0..=20 {
            for j in 0..=20 {
                let q = point(
                    &p,
                    [
                        (k as f32 / 20.0 - 0.5) * pw.0,
                        (j as f32 / 20.0 - 0.5) * pw.1,
                        0.0,
                    ],
                );
                f.clear_m = f
                    .clear_m
                    .min(segment_distance(q, elbow, shoulder) - 0.05)
                    .min(segment_distance(q, arm.wrist, elbow) - 0.035)
                    .min(segment_distance(q, arm.wrist, hand_end) - 0.04);
            }
        }
        let near = sub(nearest_on_panel(&p, pw, EYE), EYE);
        f.eye_m = f.eye_m.min(dot(near, near).sqrt());
    }
    f
}

#[test]
fn party_panel_stays_off_the_arm_and_hand() {
    for place in [Place::Back, Place::Inside] {
        for hand in [Hand::Left, Hand::Right] {
            let f = party_fit(place, hand, |m, wh| {
                let pw = (wh.0, wh.0 * PARTY_ASPECT);
                party(m, place, hand, wh, pw, SIDE_GAP_M * wh.0 / 0.28)
            });
            assert!(f.clear_m > 0.05, "{place:?} {hand:?} {f:?}");
            assert!(f.reach.1 < FOREARM_M, "{place:?} {hand:?} {f:?}");
            assert!(f.eye_m > EYE_MIN_M, "{place:?} {hand:?} {f:?}");
        }
    }
}

#[test]
#[ignore]
fn print_party_options() {
    let mut out = String::new();
    for place in [Place::Back, Place::Inside] {
        for i in 0..4 {
            let mut label = "";
            let f = party_fit(place, Hand::Left, |m, wh| {
                let (l, p) = party_options(m, wh)[i];
                label = l;
                p
            });
            out.push_str(&format!(
                "{place:?}\t{label}\tfacing {:.1}\toff the main view centre {:.1} deg\teye {:.3} m\treach along forearm {:.3}..{:.3} m\tclear of arm and hand {:.3} m\n",
                f.facing, f.off_deg, f.eye_m, f.reach.0, f.reach.1, f.clear_m
            ));
        }
    }
    println!("{out}");
    if let Some(dir) = std::env::var_os("EHUD_VR_OUT") {
        std::fs::write(std::path::Path::new(&dir).join("party_options.txt"), out).unwrap();
    }
}
fn v3(p: V3) -> String {
    format!("[{:.4},{:.4},{:.4}]", p[0], p[1], p[2])
}

#[test]
#[ignore]
fn write_geometry_json() {
    let Some(dir) = std::env::var_os("EHUD_VR_OUT") else {
        return;
    };
    let mut parts = Vec::new();
    for (label, place, arm, wh) in [
        ("old_watch", Place::Old, WATCH, FULL),
        ("old_face", Place::Old, FACE, FULL),
        ("back_watch", Place::Back, WATCH, BACK),
        ("inside_face", Place::Inside, FACE, INSIDE),
    ] {
        let nat = natural_roll(arm);
        let p = panel_world(place, Hand::Left, TOUCH_GRIP, arm, nat, wh);
        let g = grip_world(arm, Hand::Left, nat, FOREARM_TILT_DEG);
        let a = norm(arm.forearm);
        let v = view(&p);
        let cs: Vec<String> = corners(&p, wh).iter().map(|c| v3(*c)).collect();
        let pw = (wh.0, wh.0 * PARTY_ASPECT);
        let side = party(&p, place, Hand::Left, wh, pw, SIDE_GAP_M * wh.0 / 0.28);
        let ps: Vec<String> = corners(&side, pw).iter().map(|c| v3(*c)).collect();
        parts.push(format!(
            "\"{label}\":{{\"arm\":\"{}\",\"wrist\":{},\"elbow\":{},\"knuckles\":{},\"dorsal\":{},\"grip\":{},\"corners\":[{}],\"party\":[{}],\"shoulder\":{},\"center\":{},\"normal\":{},\"right\":{},\"up\":{},\"facing\":{:.1},\"roll\":{:.1},\"dist\":{:.3},\"w\":{},\"h\":{}}}",
            arm.name,
            v3(arm.wrist),
            v3(add(arm.wrist, scale(a, 0.26))),
            v3(sub(arm.wrist, scale(a, 0.09))),
            v3(back_normal(arm, nat)),
            v3(origin(&g)),
            cs.join(","),
            ps.join(","),
            v3(SHOULDER),
            v3(origin(&p)),
            v3(axis(&p, 2)),
            v3(axis(&p, 0)),
            v3(axis(&p, 1)),
            v.facing,
            v.roll,
            v.dist,
            wh.0,
            wh.1
        ));
    }
    let json = format!("{{\"eye\":{},{}}}", v3(EYE), parts.join(","));
    std::fs::write(std::path::Path::new(&dir).join("geometry.json"), json).unwrap();
}
