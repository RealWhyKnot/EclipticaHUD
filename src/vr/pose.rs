pub type M34 = [[f32; 4]; 3];
pub type V3 = [f32; 3];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hand {
    Left,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    Old,
    Back,
    Inside,
}

pub const OLD_TRANSFORM: M34 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, -1.0, 0.05],
    [0.0, 1.0, 0.0, 0.10],
];

pub const INDEX_GRIP: (V3, V3) = ([-0.003851, 0.003715, 0.075948], [15.392, -2.071, 0.303]);
pub const TOUCH_GRIP: (V3, V3) = ([0.007, -0.001_829_41, 0.101_948_2], [20.6, 0.0, 0.0]);
pub const VIVE_GRIP: (V3, V3) = ([0.0, 0.003, 0.097], [5.037, 0.0, 0.0]);

pub const FOREARM_TILT_DEG: f32 = 15.0;
pub const WRIST_ALONG_M: f32 = 0.07;
pub const WRIST_DORSAL_M: f32 = 0.015;
pub const SKIN_M: f32 = 0.02;
pub const CLEAR_M: f32 = 0.02;
pub const WRIST_GAP_M: f32 = 0.01;
pub const SIDE_GAP_M: f32 = 0.02;
pub const BACK_TILT_DEG: f32 = 7.0;
pub const BACK_SPIN_DEG: f32 = -5.0;
pub const BACK_WIDTH_M: f32 = 0.20;
pub const INSIDE_TILT_DEG: f32 = 35.0;
pub const INSIDE_SPIN_DEG: f32 = 65.0;
pub const INSIDE_WIDTH_M: f32 = 0.1775;

pub fn panel_width(place: Place, old_width: f32) -> f32 {
    match place {
        Place::Old => old_width,
        Place::Back => BACK_WIDTH_M,
        Place::Inside => INSIDE_WIDTH_M,
    }
}

pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[cfg(test)]
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

#[cfg(test)]
pub fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l <= f32::EPSILON {
        return a;
    }
    scale(a, 1.0 / l)
}

#[cfg(test)]
pub fn angle_deg(a: V3, b: V3) -> f32 {
    dot(norm(a), norm(b)).clamp(-1.0, 1.0).acos().to_degrees()
}

pub fn from_axes(x: V3, y: V3, z: V3, t: V3) -> M34 {
    [
        [x[0], y[0], z[0], t[0]],
        [x[1], y[1], z[1], t[1]],
        [x[2], y[2], z[2], t[2]],
    ]
}

pub fn axis(m: &M34, c: usize) -> V3 {
    [m[0][c], m[1][c], m[2][c]]
}

pub fn origin(m: &M34) -> V3 {
    axis(m, 3)
}

pub fn mul(a: &M34, b: &M34) -> M34 {
    let mut out = [[0.0; 4]; 3];
    for (r, row) in out.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            let mut v = (0..3).map(|k| a[r][k] * b[k][c]).sum::<f32>();
            if c == 3 {
                v += a[r][3];
            }
            *cell = v;
        }
    }
    out
}

#[cfg(test)]
pub fn invert(m: &M34) -> M34 {
    let mut out = [[0.0; 4]; 3];
    for (r, row) in out.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().take(3).enumerate() {
            *cell = m[c][r];
        }
    }
    let t = origin(m);
    for row in out.iter_mut() {
        row[3] = -(row[0] * t[0] + row[1] * t[1] + row[2] * t[2]);
    }
    out
}

#[cfg(test)]
pub fn point(m: &M34, p: V3) -> V3 {
    let d = dir(m, p);
    [d[0] + m[0][3], d[1] + m[1][3], d[2] + m[2][3]]
}

#[cfg(test)]
pub fn dir(m: &M34, v: V3) -> V3 {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

pub fn component(origin: V3, rot_deg: V3) -> M34 {
    let (sx, cx) = rot_deg[0].to_radians().sin_cos();
    let (sy, cy) = rot_deg[1].to_radians().sin_cos();
    let (sz, cz) = rot_deg[2].to_radians().sin_cos();
    let rx = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, cx, -sx, 0.0],
        [0.0, sx, cx, 0.0],
    ];
    let ry = [
        [cy, 0.0, sy, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [-sy, 0.0, cy, 0.0],
    ];
    let rz = [
        [cz, -sz, 0.0, 0.0],
        [sz, cz, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ];
    let mut m = mul(&mul(&rx, &ry), &rz);
    for (r, row) in m.iter_mut().enumerate() {
        row[3] = origin[r];
    }
    m
}

pub fn mirror(m: &M34) -> M34 {
    let s = [-1.0f32, 1.0, 1.0];
    let mut out = *m;
    for (r, row) in out.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().take(3).enumerate() {
            *cell *= s[r] * s[c];
        }
        row[3] *= s[r];
    }
    out
}

pub fn for_hand(m: &M34, hand: Hand) -> M34 {
    match hand {
        Hand::Left => *m,
        Hand::Right => mirror(m),
    }
}

pub fn grip_for_model(model: &str, hand: Hand) -> M34 {
    let m = model.to_ascii_lowercase();
    let (o, r) = if m.contains("knu") || m.contains("index") {
        INDEX_GRIP
    } else if m.contains("vive_controller") || m.contains("vr_controller_vive") {
        VIVE_GRIP
    } else {
        TOUCH_GRIP
    };
    for_hand(&component(o, r), hand)
}

pub fn forearm() -> V3 {
    let (s, c) = FOREARM_TILT_DEG.to_radians().sin_cos();
    [0.0, c, s]
}

pub fn dorsal(hand: Hand) -> V3 {
    match hand {
        Hand::Left => [-1.0, 0.0, 0.0],
        Hand::Right => [1.0, 0.0, 0.0],
    }
}

pub fn wrist(hand: Hand) -> V3 {
    add(
        scale(forearm(), WRIST_ALONG_M),
        scale(dorsal(hand), WRIST_DORSAL_M),
    )
}

pub fn panel_in_grip(place: Place, hand: Hand, w: f32, h: f32) -> M34 {
    let (tilt, spin) = match place {
        Place::Inside => (INSIDE_TILT_DEG, INSIDE_SPIN_DEG),
        _ => (BACK_TILT_DEG, BACK_SPIN_DEG),
    };
    panel_in_grip_with(place, hand, w, h, tilt, spin)
}

pub fn panel_in_grip_with(place: Place, hand: Hand, w: f32, h: f32, tilt: f32, spin: f32) -> M34 {
    let a = forearm();
    let d = dorsal(hand);
    let sign = if hand == Hand::Left { 1.0 } else { -1.0 };
    let (out, m) = match place {
        Place::Old | Place::Back => {
            let right = scale(a, -sign);
            let base = from_axes(right, cross(d, right), d, [0.0; 3]);
            let rot = component([0.0; 3], [0.0, tilt * sign, spin * sign]);
            (d, mul(&base, &rot))
        }
        Place::Inside => {
            let n = scale(d, -1.0);
            let up = scale(a, -1.0);
            let base = from_axes(cross(up, n), up, n, [0.0; 3]);
            let rot = component([0.0; 3], [-tilt, 0.0, spin * sign]);
            (n, mul(&base, &rot))
        }
    };
    let half = |v: V3| dot(axis(&m, 0), v).abs() * w / 2.0 + dot(axis(&m, 1), v).abs() * h / 2.0;
    let c = add(
        add(wrist(hand), scale(a, WRIST_GAP_M + half(a))),
        scale(out, SKIN_M + CLEAR_M + half(out)),
    );
    let mut m = m;
    for (r, row) in m.iter_mut().enumerate() {
        row[3] = c[r];
    }
    m
}

pub fn overlay_transform(place: Place, hand: Hand, grip: &M34, w: f32, h: f32) -> M34 {
    match place {
        Place::Old => for_hand(&OLD_TRANSFORM, hand),
        _ => mul(grip, &panel_in_grip(place, hand, w, h)),
    }
}

pub fn shifted(m: &M34, dx: f32, dy: f32) -> M34 {
    let t = [
        [1.0, 0.0, 0.0, dx],
        [0.0, 1.0, 0.0, dy],
        [0.0, 0.0, 1.0, 0.0],
    ];
    mul(m, &t)
}

pub fn side_sign(hand: Hand) -> f32 {
    match hand {
        Hand::Left => 1.0,
        Hand::Right => -1.0,
    }
}

pub fn beside(main: &M34, sign: f32, main_wh: (f32, f32), side_wh: (f32, f32), gap: f32) -> M34 {
    let dx = sign * ((main_wh.0 + side_wh.0) / 2.0 + gap);
    let dy = (main_wh.1 - side_wh.1) / 2.0;
    shifted(main, dx, dy)
}

pub fn above(main: &M34, main_wh: (f32, f32), side_wh: (f32, f32), gap: f32) -> M34 {
    shifted(main, 0.0, (main_wh.1 + side_wh.1) / 2.0 + gap)
}

pub fn party(
    main: &M34,
    place: Place,
    hand: Hand,
    main_wh: (f32, f32),
    side_wh: (f32, f32),
    gap: f32,
) -> M34 {
    match place {
        Place::Old => beside(main, side_sign(hand), main_wh, side_wh, gap),
        _ => above(main, main_wh, side_wh, gap),
    }
}
