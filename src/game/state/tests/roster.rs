use super::*;
use crate::game::state::apply::ROSTER_CAP;

const W1: &str = "[Behaviour] Joining wrld_0fb88df3-2057-4c2f-8e06-e948864378fd:98473~hidden(usr_4e64)~region(use)";
const W2: &str = "[Behaviour] Joining wrld_0fb88df3-2057-4c2f-8e06-e948864378fd:11111~hidden(usr_4e64)~region(use)";
const ENTER: &str = "[Behaviour] Entering Room: Ecliptica - Demo Playtest";

fn names(gs: &GameState) -> Vec<(&str, &str)> {
    gs.roster
        .iter()
        .map(|(u, n)| (u.as_str(), n.as_str()))
        .collect()
}

#[test]
fn own_usr_id_is_kept() {
    let mut gs = GameState::default();
    feed_at(
        &mut gs,
        "08:00:00",
        "User Authenticated: Be\u{430}r (Hands) (usr_4e64-aa)",
    );
    assert_eq!(gs.me.as_deref(), Some("Be\u{430}r (Hands)"));
    assert_eq!(gs.me_usr.as_deref(), Some("usr_4e64-aa"));
    feed_at(&mut gs, "08:00:01", "[Behaviour] OnLeftRoom");
    assert_eq!(gs.me_usr.as_deref(), Some("usr_4e64-aa"));
}

#[test]
fn roster_follows_joins_and_leaves() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "08:00:00", ENTER);
    feed_at(&mut gs, "08:00:00", W1);
    let seq = gs.roster_seq;
    feed_at(
        &mut gs,
        "08:00:01",
        "[Behaviour] OnPlayerJoined WhyKnot (usr_1)",
    );
    feed_at(
        &mut gs,
        "08:00:02",
        "[Behaviour] OnPlayerJoined Be\u{430}r (Hands) (usr_2)",
    );
    assert_eq!(
        names(&gs),
        [("usr_1", "WhyKnot"), ("usr_2", "Be\u{430}r (Hands)")]
    );
    assert!(gs.roster_seq > seq);
    let seq = gs.roster_seq;
    feed_at(
        &mut gs,
        "08:00:03",
        "[Behaviour] OnPlayerJoined WhyKnot (usr_1)",
    );
    assert_eq!(gs.roster_seq, seq);
    feed_at(
        &mut gs,
        "08:00:03",
        "[Behaviour] OnPlayerJoined NewName (usr_1)",
    );
    assert_eq!(names(&gs)[0], ("usr_1", "NewName"));
    assert!(gs.roster_seq > seq);
    feed_at(&mut gs, "08:00:04", "[Behaviour] OnPlayerLeftRoom");
    assert_eq!(gs.roster.len(), 2);
    feed_at(
        &mut gs,
        "08:00:04",
        "[Behaviour] OnPlayerLeft NewName (usr_1)",
    );
    assert_eq!(names(&gs), [("usr_2", "Be\u{430}r (Hands)")]);
    let seq = gs.roster_seq;
    feed_at(
        &mut gs,
        "08:00:05",
        "[Behaviour] OnPlayerLeft Ghost (usr_9)",
    );
    assert_eq!(gs.roster_seq, seq);
    assert_eq!(gs.roster.len(), 1);
}

#[test]
fn roster_clears_on_left_room_and_new_location() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "08:00:00", ENTER);
    feed_at(&mut gs, "08:00:00", W1);
    feed_at(&mut gs, "08:00:01", "[Behaviour] OnPlayerJoined A (usr_1)");
    feed_at(&mut gs, "08:00:02", W1);
    assert_eq!(gs.roster.len(), 1);
    feed_at(&mut gs, "08:00:03", W2);
    assert!(gs.roster.is_empty());
    feed_at(&mut gs, "08:00:04", "[Behaviour] OnPlayerJoined B (usr_2)");
    feed_at(&mut gs, "08:00:05", "[Behaviour] OnLeftRoom");
    assert!(gs.roster.is_empty());
    assert_eq!(gs.location, None);
    feed_at(&mut gs, "08:00:05", "[Behaviour] OnPlayerLeft B (usr_2)");
    feed_at(&mut gs, "08:00:06", ENTER);
    feed_at(&mut gs, "08:00:06", W1);
    feed_at(&mut gs, "08:00:07", "[Behaviour] OnPlayerJoined C (usr_3)");
    assert_eq!(names(&gs), [("usr_3", "C")]);
    feed_at(&mut gs, "08:00:08", "[Behaviour] Entering Room: Sky Dream");
    assert!(gs.roster.is_empty());
    feed_at(&mut gs, "08:00:09", "[Behaviour] OnPlayerJoined D (usr_4)");
    gs.log_rotated();
    assert!(gs.roster.is_empty());
}

#[test]
fn roster_is_bounded() {
    let mut gs = GameState::default();
    for i in 0..ROSTER_CAP + 50 {
        feed_at(
            &mut gs,
            "08:00:00",
            &format!("[Behaviour] OnPlayerJoined P{i} (usr_{i})"),
        );
    }
    assert_eq!(gs.roster.len(), ROSTER_CAP);
    feed_at(&mut gs, "08:00:01", "[Behaviour] OnPlayerLeft P0 (usr_0)");
    feed_at(
        &mut gs,
        "08:00:01",
        "[Behaviour] OnPlayerJoined Late (usr_late)",
    );
    assert_eq!(gs.roster.last().unwrap().1, "Late");
}

#[test]
fn relay_damage_is_the_live_fight_total() {
    let mut gs = GameState::default();
    feed_at(&mut gs, "08:00:00", ENTER);
    feed_at(&mut gs, "08:00:00", W1);
    feed_at(&mut gs, "08:00:01", HALL);
    feed_at(&mut gs, "08:00:02", "Dealing 50 STRIKE damage");
    assert_eq!(gs.relay_dmg(), None);
    feed_at(
        &mut gs,
        "08:01:00",
        "ECLIPTICA - now fighting boss: DarkMouth(Clone) on phase: 0",
    );
    assert_eq!(gs.relay_dmg(), Some(0));
    feed_at(&mut gs, "08:01:01", "Dealing 100 STRIKE damage");
    feed_at(&mut gs, "08:01:02", "Dealing 7 NON-STRIKE damage");
    assert_eq!(gs.relay_dmg(), Some(107));
    feed_at(
        &mut gs,
        "08:01:10",
        "Boss DarkMouth dead, personal damage dealt: ",
    );
    assert_eq!(gs.relay_dmg(), Some(107));
    assert_eq!(gs.fight_dmg, 107);
}
