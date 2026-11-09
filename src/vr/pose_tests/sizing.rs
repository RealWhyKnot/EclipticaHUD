use super::*;

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
            let sw = 0.08 + crate::num::to_f32(i) * 0.01;
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
#[ignore = "prints numbers for review"]
fn print_width_sweep() {
    let mut out = String::new();
    for place in [Place::Back, Place::Inside] {
        let arms = arms_for(place);
        let old = fit_world(&arms, FULL, |world| {
            let grip = component(TOUCH_GRIP.0, TOUCH_GRIP.1);
            mul(&mul(world, &invert(&grip)), &OLD_TRANSFORM)
        });
        let _ = writeln!(
            out,
            "{place:?} poses, old transform at 0.28 m\t{old:?}\tmargins {:?}",
            old.margins()
        );
        let mut best: Option<(f32, f32, f32, Fit)> = None;
        let mut width = 0.08f32;
        while width <= 0.3201 {
            let (tilt, spin, fit) = best_angles(place, &arms, width, 3, None);
            let margins = fit.margins();
            let _ = writeln!(
                out,
                "{place:?}\tw {width:.3}\ttilt {tilt}\tspin {spin}\tfacing {:.1}\troll {:.1}\ttext {:.2} deg\tforearm {:.3} m\tview {:.1} deg\teye {:.3} m\tmargins {:.2} {:.2} {:.2} {:.2} {:.2}\tscore {:.3}",
                fit.facing,
                fit.roll,
                fit.text_deg,
                fit.forearm,
                fit.view_deg,
                fit.eye_m,
                margins[0],
                margins[1],
                margins[2],
                margins[3],
                margins[4],
                fit.score()
            );
            if best
                .as_ref()
                .is_none_or(|other| fit.score() > other.3.score())
            {
                best = Some((width, tilt, spin, fit));
            }
            width += 0.01;
        }
        let (w0, t0, s0, _) = best.unwrap();
        let mut fine: Option<(f32, f32, f32, Fit)> = None;
        let mut width = w0 - 0.01;
        while width <= w0 + 0.0101 {
            let (tilt, spin, fit) = best_angles(place, &arms, width, 1, Some((t0, s0, 4)));
            if fine
                .as_ref()
                .is_none_or(|other| fit.score() > other.3.score())
            {
                fine = Some((width, tilt, spin, fit));
            }
            width += 0.0025;
        }
        let (width, tilt, spin, fit) = fine.unwrap();
        let _ = writeln!(
            out,
            "{place:?} best\tw {width:.4}\ttilt {tilt}\tspin {spin}\t{fit:?}\tmargins {:?}\tscore {:.3}",
            fit.margins(),
            fit.score()
        );
    }
    crate::console::say(format_args!("{out}"));
    if let Some(dir) = std::env::var_os("EHUD_VR_OUT") {
        std::fs::write(std::path::Path::new(&dir).join("width_sweep.txt"), out).unwrap();
    }
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
#[ignore = "prints numbers for review"]
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
            let _ = writeln!(
                out,
                "{place:?}\t{label}\tfacing {:.1}\toff the main view centre {:.1} deg\teye {:.3} m\treach along forearm {:.3}..{:.3} m\tclear of arm and hand {:.3} m",
                f.facing, f.off_deg, f.eye_m, f.reach.0, f.reach.1, f.clear_m
            );
        }
    }
    crate::console::say(format_args!("{out}"));
    if let Some(dir) = std::env::var_os("EHUD_VR_OUT") {
        std::fs::write(std::path::Path::new(&dir).join("party_options.txt"), out).unwrap();
    }
}

#[test]
#[ignore = "writes a file for review"]
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
