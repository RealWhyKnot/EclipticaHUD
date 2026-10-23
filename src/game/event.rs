#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    BossFight {
        name: String,
        phase: String,
    },
    BossDead {
        name: String,
    },
    StrikeTotal(u64),
    NonStrikeTotal(u64),
    Dealt {
        n: u64,
        strike: bool,
    },
    DamageTaken {
        amount: u64,
        source: String,
    },
    Ownership {
        object: String,
        player: String,
    },
    Stage {
        name: String,
        progress: f32,
        class: String,
    },
    EnemySpawn {
        slot: u32,
        kind: u32,
    },
    EnemyRetire(u32),
    EnemyName(String),
    Intermission,
    Lobby,
    RoomLeft,
    PlayerDead,
    TokenSpawn {
        rune: bool,
        chance: u32,
    },
    SessionSave(String),
    RoomJoin(String),
    RoomEnter(String),
    SelfName {
        name: String,
        usr: String,
    },
    PlayerJoined {
        name: String,
        usr: String,
    },
    PlayerLeft {
        name: String,
        usr: String,
    },
    WorldLine(String),
}

const WORLD_PREFIXES: &[&str] = &[
    "ECLIPTICA",
    "ENEMY ",
    "Enemy ",
    "Boss ",
    "Tracking boss",
    "Backup Active",
    "No backup active",
    "Advancing Stage",
    "fading out music",
    "Local controller",
    "STRIKE DMG",
    "NON-STRIKE DMG",
    "Dealing ",
    "damage has been taken",
    "spawn token",
    "Initializing Enemy",
    "Retiring Enemy",
    "Removed player",
    "ownership of ",
    "[Behaviour] Restored player",
    "[Behaviour] I am MASTER",
    "[Behaviour] No targets to encode on ",
];

impl Event {
    pub fn label(&self) -> &'static str {
        match self {
            Event::BossFight { .. } => "boss_fight",
            Event::BossDead { .. } => "boss_dead",
            Event::StrikeTotal(_) => "strike_total",
            Event::NonStrikeTotal(_) => "non_strike_total",
            Event::Dealt { .. } => "dealt",
            Event::DamageTaken { .. } => "damage_taken",
            Event::Ownership { .. } => "ownership",
            Event::Stage { .. } => "stage",
            Event::EnemySpawn { .. } => "enemy_spawn",
            Event::EnemyRetire(_) => "enemy_retire",
            Event::EnemyName(_) => "enemy_name",
            Event::Intermission => "intermission",
            Event::Lobby => "lobby",
            Event::RoomLeft => "room_left",
            Event::PlayerDead => "player_dead",
            Event::TokenSpawn { .. } => "token_spawn",
            Event::SessionSave(_) => "session_save",
            Event::RoomJoin(_) => "room_join",
            Event::RoomEnter(_) => "room_enter",
            Event::SelfName { .. } => "self_name",
            Event::PlayerJoined { .. } => "player_joined",
            Event::PlayerLeft { .. } => "player_left",
            Event::WorldLine(_) => "world_line",
        }
    }
}

pub struct Line<'a> {
    pub ts: u64,
    pub msg: &'a str,
}

pub fn split_line(raw: &str) -> Option<Line<'_>> {
    let b = raw.as_bytes();
    if b.len() < 34 || b[4] != b'.' || b[7] != b'.' || b[10] != b' ' {
        return None;
    }
    let ts = parse_ts(&raw[..19])?;
    let rest = &raw[19..];
    let sep = rest.find("-  ")?;
    Some(Line {
        ts,
        msg: &rest[sep + 3..],
    })
}

fn parse_ts(s: &str) -> Option<u64> {
    let num = |r: &str| r.parse::<u64>().ok();
    let y = num(s.get(0..4)?)?;
    let mo = num(s.get(5..7)?)?;
    let d = num(s.get(8..10)?)?;
    let h = num(s.get(11..13)?)?;
    let mi = num(s.get(14..16)?)?;
    let sec = num(s.get(17..19)?)?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let (y, mo) = if mo <= 2 { (y - 1, mo + 12) } else { (y, mo) };
    let era_days = 365 * y + y / 4 - y / 100 + y / 400 + (153 * (mo + 1)) / 5 + d - 1;
    Some(era_days * 86400 + h * 3600 + mi * 60 + sec)
}

pub fn parse_msg(msg: &str) -> Option<Event> {
    parse_known(msg).or_else(|| {
        let msg = msg.trim_end();
        WORLD_PREFIXES
            .iter()
            .any(|p| msg.starts_with(p))
            .then(|| Event::WorldLine(msg.to_string()))
    })
}

fn parse_known(msg: &str) -> Option<Event> {
    if let Some(rest) = msg.strip_prefix("ownership of ") {
        let (object, player) = rest.split_once(" transferred to ")?;
        let player = player.trim_end();
        if player.is_empty() {
            return None;
        }
        return Some(Event::Ownership {
            object: object.to_string(),
            player: player.to_string(),
        });
    }
    if let Some(rest) = msg.strip_prefix("Dealing ") {
        let (n, kind) = rest.split_once(' ')?;
        let strike = match kind.trim_end() {
            "STRIKE damage" => true,
            "NON-STRIKE damage" => false,
            _ => return None,
        };
        return Some(Event::Dealt {
            n: n.parse().ok()?,
            strike,
        });
    }
    if let Some(rest) = msg.strip_prefix("damage has been taken: ") {
        let (amount, source) = rest.split_once(", from source:")?;
        return Some(Event::DamageTaken {
            amount: amount.parse().ok()?,
            source: source.trim().to_string(),
        });
    }
    if let Some(rest) = msg.strip_prefix("ECLIPTICA - now ") {
        if let Some(rest) = rest.strip_prefix("fighting boss: ") {
            let (name, rest) = rest.split_once("(Clone)")?;
            let phase = rest.split_once(" on phase: ").map_or("", |(_, p)| p.trim());
            return Some(Event::BossFight {
                name: name.to_string(),
                phase: phase.to_string(),
            });
        }
        if let Some(rest) = rest.strip_prefix("in stage: ") {
            let (name, rest) = rest.split_once(" on phase: ")?;
            let (progress, class) = rest.split_once(" as class: ")?;
            let name = name.strip_prefix("Stage_").unwrap_or(name);
            return Some(Event::Stage {
                name: name.to_string(),
                progress: progress.parse().ok()?,
                class: class.trim().to_string(),
            });
        }
        if rest.starts_with("in intermission") {
            return Some(Event::Intermission);
        }
        if rest.starts_with("in lobby") {
            return Some(Event::Lobby);
        }
        return None;
    }
    if let Some(rest) = msg.strip_prefix("Boss ") {
        let name = rest
            .strip_suffix("dead, personal damage dealt: ")?
            .trim_end();
        return Some(Event::BossDead {
            name: name.to_string(),
        });
    }
    if let Some(rest) = msg.strip_prefix("[Behaviour] Entering Room: ") {
        return Some(Event::RoomEnter(rest.trim_end().to_string()));
    }
    if let Some(rest) = msg.strip_prefix("[Behaviour] Joining wrld_") {
        return Some(Event::RoomJoin(format!("wrld_{}", rest.trim_end())));
    }
    if let Some(rest) = msg.strip_prefix("User Authenticated: ") {
        let (name, usr) = name_and_usr(rest)?;
        return Some(Event::SelfName { name, usr });
    }
    if let Some(rest) = msg.strip_prefix("[Behaviour] OnPlayerJoined ") {
        let (name, usr) = name_and_usr(rest)?;
        return Some(Event::PlayerJoined { name, usr });
    }
    if let Some(rest) = msg.strip_prefix("[Behaviour] OnPlayerLeft ") {
        let (name, usr) = name_and_usr(rest)?;
        return Some(Event::PlayerLeft { name, usr });
    }
    if let Some(rest) = msg.strip_prefix("ECLIPTICA saving SESSION ID ") {
        return Some(Event::SessionSave(rest.trim().to_string()));
    }
    if msg.trim_end() == "[Behaviour] OnLeftRoom"
        || msg.starts_with("VRCApplication: HandleApplicationQuit")
    {
        return Some(Event::RoomLeft);
    }
    if msg.trim_end() == "Local controller dead, switching off." {
        return Some(Event::PlayerDead);
    }
    if let Some(rest) = msg.strip_prefix("spawn token, ") {
        let (b, n) = rest.split_once(", ")?;
        let rune = match b {
            "True" => true,
            "False" => false,
            _ => return None,
        };
        return Some(Event::TokenSpawn {
            rune,
            chance: n.trim().parse().ok()?,
        });
    }
    if let Some(rest) = msg.strip_prefix("STRIKE DMG: ") {
        return Some(Event::StrikeTotal(rest.trim().parse().ok()?));
    }
    if let Some(rest) = msg.strip_prefix("NON-STRIKE DMG: ") {
        return Some(Event::NonStrikeTotal(rest.trim().parse().ok()?));
    }
    if let Some(rest) = msg.strip_prefix("Initializing Enemy POOL ID") {
        let (slot, kind) = rest.split_once(" as ENEMY ID ")?;
        return Some(Event::EnemySpawn {
            slot: slot.parse().ok()?,
            kind: kind.trim().parse().ok()?,
        });
    }
    if let Some(rest) = msg.strip_prefix("Retiring Enemy POOL ID") {
        return Some(Event::EnemyRetire(rest.trim().parse().ok()?));
    }
    if let Some(rest) = msg.strip_prefix("[Behaviour] No targets to encode on ") {
        let name = rest.trim_end().strip_suffix("(Clone)")?;
        return Some(Event::EnemyName(name.to_string()));
    }
    None
}

fn name_and_usr(rest: &str) -> Option<(String, String)> {
    let (name, id) = rest.trim_end().rsplit_once(" (usr_")?;
    let id = id.strip_suffix(')')?;
    let name = name.trim();
    if name.is_empty()
        || id.is_empty()
        || id.len() > 64
        || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return None;
    }
    Some((name.to_string(), format!("usr_{id}")))
}

pub fn fmt_clock(ts: u64) -> String {
    format!("{:02}:{:02}:{:02}", ts / 3600 % 24, ts / 60 % 60, ts % 60)
}

fn month_start(y: u64, mo: u64) -> u64 {
    365 * y + y / 4 - y / 100 + y / 400 + (153 * (mo + 1)) / 5
}

pub fn civil(ts: u64) -> (u64, u64, u64) {
    let days = ts / 86400;
    let mut y = days * 400 / 146097;
    while month_start(y + 1, 3) <= days {
        y += 1;
    }
    while y > 0 && month_start(y, 3) > days {
        y -= 1;
    }
    let mo = (3..=14)
        .rev()
        .find(|&m| month_start(y, m) <= days)
        .unwrap_or(3);
    let d = days - month_start(y, mo) + 1;
    if mo > 12 {
        (y + 1, mo - 12, d)
    } else {
        (y, mo, d)
    }
}

pub fn fmt_date(ts: u64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let (y, mo, d) = civil(ts);
    format!("{} {d} {y}", MONTHS[(mo as usize - 1).min(11)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_round_trips_parse_ts() {
        for y in [1999u64, 2000, 2024, 2026, 2100] {
            for mo in 1..=12u64 {
                for d in [1u64, 15, 28] {
                    let s = format!("{y:04}.{mo:02}.{d:02} 23:59:58");
                    let ts = parse_ts(&s).unwrap();
                    assert_eq!(civil(ts), (y, mo, d), "{s}");
                }
            }
        }
        let leap = parse_ts("2024.02.29 00:00:00").unwrap();
        assert_eq!(civil(leap), (2024, 2, 29));
        assert_eq!(civil(leap + 86400), (2024, 3, 1));
        let eve = parse_ts("2026.12.31 12:00:00").unwrap();
        assert_eq!(civil(eve + 86400), (2027, 1, 1));
        assert_eq!(
            fmt_date(parse_ts("2026.10.22 14:05:00").unwrap()),
            "Oct 22 2026"
        );
    }

    #[test]
    fn ownership_unicode() {
        let ev = parse_msg("ownership of ManalyteBig transferred to Be\u{430}rHands").unwrap();
        assert_eq!(
            ev,
            Event::Ownership {
                object: "ManalyteBig".into(),
                player: "Be\u{430}rHands".into()
            }
        );
        let ev = parse_msg("ownership of BigWolf transferred to \u{1d04}\u{29c}\u{1d07}\u{1d05}\u{1d1c}\u{493} \u{6c17}\u{307e}\u{3050}\u{308c}").unwrap();
        match ev {
            Event::Ownership { player, .. } => assert_eq!(player.chars().count(), 11),
            _ => panic!(),
        }
    }

    #[test]
    fn boss_lines() {
        assert_eq!(
            parse_msg("ECLIPTICA - now fighting boss: ObisidusPhase2(Clone) on phase: 0.7223684"),
            Some(Event::BossFight {
                name: "ObisidusPhase2".into(),
                phase: "0.7223684".into()
            })
        );
        assert_eq!(
            parse_msg("ECLIPTICA - now fighting boss: Hakli(Clone) (UnityEngine.GameObject)"),
            Some(Event::BossFight {
                name: "Hakli".into(),
                phase: String::new()
            })
        );
        assert_eq!(
            parse_msg("Boss Kakarot dead, personal damage dealt: "),
            Some(Event::BossDead {
                name: "Kakarot".into()
            })
        );
        assert_eq!(
            parse_msg("STRIKE DMG: 4793"),
            Some(Event::StrikeTotal(4793))
        );
        assert_eq!(
            parse_msg("NON-STRIKE DMG: 0"),
            Some(Event::NonStrikeTotal(0))
        );
    }

    #[test]
    fn damage_lines() {
        assert_eq!(
            parse_msg("Dealing 140 STRIKE damage"),
            Some(Event::Dealt {
                n: 140,
                strike: true
            })
        );
        assert_eq!(
            parse_msg("Dealing 5 NON-STRIKE damage"),
            Some(Event::Dealt {
                n: 5,
                strike: false
            })
        );
        assert_eq!(parse_known("Dealing 5 MAGIC damage"), None);
        assert_eq!(
            parse_msg("damage has been taken: 12, from source: (Khepri) attack_Claws2"),
            Some(Event::DamageTaken {
                amount: 12,
                source: "(Khepri) attack_Claws2".into()
            })
        );
        assert_eq!(
            parse_msg("damage has been taken: 2, from source: "),
            Some(Event::DamageTaken {
                amount: 2,
                source: String::new()
            })
        );
        assert_eq!(
            parse_msg("damage has been taken: 2, from source:"),
            Some(Event::DamageTaken {
                amount: 2,
                source: String::new()
            })
        );
    }

    #[test]
    fn stage_line() {
        let ev = parse_msg(
            "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer",
        )
        .unwrap();
        assert_eq!(
            ev,
            Event::Stage {
                name: "Hall of Beginnings".into(),
                progress: 0.0,
                class: "Spellhammer".into()
            }
        );
        assert_eq!(
            parse_msg("ECLIPTICA - now in intermission"),
            Some(Event::Intermission)
        );
        assert_eq!(parse_msg("ECLIPTICA - now in lobby"), Some(Event::Lobby));
    }

    #[test]
    fn room_left() {
        let l = split_line("2026.09.06 21:44:44 Debug      -  [Behaviour] OnLeftRoom").unwrap();
        assert_eq!(parse_msg(l.msg), Some(Event::RoomLeft));
        assert_eq!(
            parse_msg("VRCApplication: HandleApplicationQuit at 8111.613"),
            Some(Event::RoomLeft)
        );
        assert!(
            split_line("  at \u{cc}\u{ce}\u{ce}\u{cc}.OnLeftRoom () [0x00000] in <0>:0 ").is_none()
        );
    }

    #[test]
    fn player_dead() {
        let l =
            split_line("2026.09.08 08:20:42 Debug      -  Local controller dead, switching off.")
                .unwrap();
        assert_eq!(parse_msg(l.msg), Some(Event::PlayerDead));
        assert_eq!(parse_known("Tracking boss as defeated in-run."), None);
    }

    #[test]
    fn token_spawn() {
        assert_eq!(
            parse_msg("spawn token, False, 0"),
            Some(Event::TokenSpawn {
                rune: false,
                chance: 0
            })
        );
        assert_eq!(
            parse_msg("spawn token, True, 55"),
            Some(Event::TokenSpawn {
                rune: true,
                chance: 55
            })
        );
        assert_eq!(parse_known("spawn token, Maybe, 5"), None);
    }

    #[test]
    fn session_save_and_room_join() {
        assert_eq!(
            parse_msg("ECLIPTICA saving SESSION ID 19854"),
            Some(Event::SessionSave("19854".into()))
        );
        assert_eq!(
            parse_known("ECLIPTICA MASTER Setting SESSION ID to 2505"),
            None
        );
        assert_eq!(
            parse_msg("[Behaviour] Joining wrld_0fb88df3-2057-4c2f-8e06-e948864378fd:87887~hidden(usr_4e64b21b-fbd0-4c12-8b55-c8c500b517b1)~region(use)\r"),
            Some(Event::RoomJoin(
                "wrld_0fb88df3-2057-4c2f-8e06-e948864378fd:87887~hidden(usr_4e64b21b-fbd0-4c12-8b55-c8c500b517b1)~region(use)".into()
            ))
        );
        assert_eq!(
            parse_known("[Behaviour] Joining or Creating Room: Ecliptica - Demo Playtest"),
            None
        );
        assert_eq!(
            parse_msg("[Behaviour] Entering Room: Ecliptica - Demo Playtest\r"),
            Some(Event::RoomEnter("Ecliptica - Demo Playtest".into()))
        );
        assert_eq!(
            parse_msg("[Behaviour] Entering Room: Sky Dream"),
            Some(Event::RoomEnter("Sky Dream".into()))
        );
    }

    #[test]
    fn self_name() {
        let l = split_line(
            "2026.10.22 13:44:35 Debug      -  User Authenticated: WhyKnot (usr_4e64b21b-fbd0-4c12-8b55-c8c500b517b1)\r",
        )
        .unwrap();
        assert_eq!(
            parse_msg(l.msg),
            Some(Event::SelfName {
                name: "WhyKnot".into(),
                usr: "usr_4e64b21b-fbd0-4c12-8b55-c8c500b517b1".into()
            })
        );
        assert_eq!(
            parse_msg("User Authenticated: Be\u{430}r (Hands) (usr_1)"),
            Some(Event::SelfName {
                name: "Be\u{430}r (Hands)".into(),
                usr: "usr_1".into()
            })
        );
        assert_eq!(parse_known("User Authenticated:  (usr_1)"), None);
        assert_eq!(parse_known("User Authenticated: WhyKnot"), None);
        assert_eq!(parse_known("User Authenticated: WhyKnot (usr_)"), None);
        assert_eq!(parse_known("User Authenticated: WhyKnot (usr_1"), None);
        assert_eq!(parse_known("User Authenticated: WhyKnot (usr_a b)"), None);
    }

    fn joined(name: &str, usr: &str) -> Option<Event> {
        Some(Event::PlayerJoined {
            name: name.into(),
            usr: usr.into(),
        })
    }

    fn left(name: &str, usr: &str) -> Option<Event> {
        Some(Event::PlayerLeft {
            name: name.into(),
            usr: usr.into(),
        })
    }

    #[test]
    fn player_joined_and_left() {
        let l = split_line(
            "2026.10.22 13:44:49 Debug      -  [Behaviour] OnPlayerJoined WhyKnot (usr_4e64b21b-fbd0-4c12-8b55-c8c500b517b1)\r",
        )
        .unwrap();
        assert_eq!(
            parse_msg(l.msg),
            joined("WhyKnot", "usr_4e64b21b-fbd0-4c12-8b55-c8c500b517b1")
        );
        let l = split_line(
            "2026.10.22 13:48:37 Debug      -  [Behaviour] OnPlayerLeft WhyKnot (usr_4e64b21b-fbd0-4c12-8b55-c8c500b517b1)",
        )
        .unwrap();
        assert_eq!(
            parse_msg(l.msg),
            left("WhyKnot", "usr_4e64b21b-fbd0-4c12-8b55-c8c500b517b1")
        );
        assert_eq!(
            parse_msg("[Behaviour] OnPlayerJoined Be\u{430}r (Hands) (usr_97dd)"),
            joined("Be\u{430}r (Hands)", "usr_97dd")
        );
        assert_eq!(
            parse_msg("[Behaviour] OnPlayerJoined  two  spaces  (x) (usr_1)"),
            joined("two  spaces  (x)", "usr_1")
        );
        assert_eq!(
            parse_msg("[Behaviour] OnPlayerLeft (usr_fake) (usr_2)"),
            left("(usr_fake)", "usr_2")
        );
        assert_eq!(
            parse_msg("[Behaviour] OnPlayerJoined \u{6c17}\u{307e}\u{3050}\u{308c} (usr_3)"),
            joined("\u{6c17}\u{307e}\u{3050}\u{308c}", "usr_3")
        );
    }

    #[test]
    fn player_lines_that_are_not_join_or_leave() {
        assert_eq!(parse_known("[Behaviour] OnPlayerLeftRoom"), None);
        assert_eq!(parse_known("[Behaviour] OnPlayerLeftRoom\r"), None);
        assert_eq!(
            parse_known("[Behaviour] OnPlayerLeftRoom WhyKnot (usr_1)"),
            None
        );
        assert_eq!(
            parse_known("[Behaviour] OnPlayerJoinComplete WhyKnot"),
            None
        );
        assert_eq!(
            parse_known("[Behaviour] OnPlayerEnteredRoom WhyKnot (usr_1)"),
            None
        );
        assert_eq!(parse_known("[Behaviour] OnPlayerJoined WhyKnot"), None);
        assert_eq!(parse_known("[Behaviour] OnPlayerLeft WhyKnot"), None);
        assert_eq!(parse_known("[Behaviour] OnPlayerJoined (usr_1)"), None);
        assert_eq!(parse_known("[Behaviour] OnPlayerJoined  (usr_1)"), None);
        assert_eq!(
            parse_known("[Behaviour] OnPlayerJoined WhyKnot (usr_)"),
            None
        );
        assert_eq!(
            parse_known("[Behaviour] OnPlayerJoined WhyKnot (usr_1) trailing"),
            None
        );
        assert_eq!(parse_known("OnPlayerJoined WhyKnot (usr_1)"), None);
        assert_eq!(
            parse_msg(&format!(
                "[Behaviour] OnPlayerJoined A (usr_{})",
                "a".repeat(65)
            )),
            None
        );
        assert_eq!(parse_msg("[Behaviour] OnLeftRoom"), Some(Event::RoomLeft));
    }

    #[test]
    fn empty_class_parses() {
        assert_eq!(
            parse_msg("ECLIPTICA - now in stage: Stage_ProtoColony on phase: 0.1220348 as class: "),
            Some(Event::Stage {
                name: "ProtoColony".into(),
                progress: 0.1220348,
                class: String::new(),
            })
        );
    }

    #[test]
    fn enemy_pool() {
        assert_eq!(
            parse_msg("Retiring Enemy POOL ID19"),
            Some(Event::EnemyRetire(19))
        );
        assert_eq!(
            parse_msg("Initializing Enemy POOL ID17 as ENEMY ID 87"),
            Some(Event::EnemySpawn { slot: 17, kind: 87 })
        );
        assert_eq!(
            parse_msg("[Behaviour] No targets to encode on Neko1(Clone)"),
            Some(Event::EnemyName("Neko1".into()))
        );
        assert_eq!(
            parse_msg("[Behaviour] No targets to encode on Ice Squid(Clone)"),
            Some(Event::EnemyName("Ice Squid".into()))
        );
        assert_eq!(
            parse_known("Initializing Enemy POOL IDx as ENEMY ID 45"),
            None
        );
        assert_eq!(parse_known("Backup Active, swapping..."), None);
        assert_eq!(parse_known("No backup active, creating new."), None);
    }

    #[test]
    fn unparsed_world_lines_are_kept_whole() {
        for msg in [
            "Tracking boss as defeated in-run.",
            "Backup Active, swapping...",
            "No backup active, creating new.",
            "Advancing Stage Progress to: 5",
            "fading out music #3",
            "ENEMY Kakarot INTRO SLAP.",
            "ECLIPTICA MASTER Setting SESSION ID to 2505",
            "spawn token, Maybe, 5",
            "Removed player Lena",
            "[Behaviour] I am MASTER",
        ] {
            assert_eq!(parse_msg(msg), Some(Event::WorldLine(msg.into())), "{msg}");
        }
        for msg in [
            "2.5",
            "[Behaviour] OnPlayerLeftRoom",
            "OnPlayerJoined WhyKnot (usr_1)",
            "[Behaviour] OnPlayerJoined WhyKnot",
            "Microphone device changed",
        ] {
            assert_eq!(parse_msg(msg), None, "{msg}");
        }
    }

    #[test]
    fn non_events() {
        assert_eq!(parse_known("Advancing Stage Progress to: 5"), None);
        assert_eq!(parse_known("2.5"), None);
        assert_eq!(parse_known("Tracking boss as defeated in-run."), None);
    }

    #[test]
    fn line_split() {
        let l = split_line("2026.09.07 09:03:54 Debug      -  Dealing 90 STRIKE damage").unwrap();
        assert_eq!(l.msg, "Dealing 90 STRIKE damage");
        let l2 = split_line("2026.09.07 09:03:55 Warning    -  ENEMY Kakarot INTRO SLAP.").unwrap();
        assert_eq!(l2.ts, l.ts + 1);
        assert!(split_line("  at UnityEngine.EventSystems.ExecuteEvents.Execute").is_none());
        assert!(split_line("").is_none());
    }
}
