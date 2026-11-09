use super::*;

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
#[ignore = "prints numbers for review"]
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
                    let _ = writeln!(
                        out,
                        "{place:?}\t{name}\t{hand:?}\t{}\tfacing {:.1}\timage roll {:.1}\tdist {:.2}\tbest facing {:.1}\textra pronation {:.1}",
                        arm.name,
                        c.natural.facing,
                        c.natural.roll,
                        c.natural.dist,
                        c.best_facing,
                        c.pronation
                    );
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
        let _ = writeln!(out, "true forearm tilt {t}\tworst back facing {worst:.1}");
    }
    crate::console::say(format_args!("{out}"));
    if let Some(dir) = std::env::var_os("EHUD_VR_OUT") {
        std::fs::write(std::path::Path::new(&dir).join("comfort_table.txt"), out).unwrap();
    }
}
