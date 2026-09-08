#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    BossFight {
        name: String,
    },
    BossDead {
        name: String,
    },
    StrikeTotal(u64),
    NonStrikeTotal(u64),
    DealtStrike(u64),
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
    StageProgress(u32),
    Intermission,
    Lobby,
    RoomLeft,
    PlayerDead,
    TokenSpawn {
        rune: bool,
        chance: u32,
    },
}

impl Event {
    pub fn label(&self) -> &'static str {
        match self {
            Event::BossFight { .. } => "boss_fight",
            Event::BossDead { .. } => "boss_dead",
            Event::StrikeTotal(_) => "strike_total",
            Event::NonStrikeTotal(_) => "non_strike_total",
            Event::DealtStrike(_) => "dealt_strike",
            Event::DamageTaken { .. } => "damage_taken",
            Event::Ownership { .. } => "ownership",
            Event::Stage { .. } => "stage",
            Event::StageProgress(_) => "stage_progress",
            Event::Intermission => "intermission",
            Event::Lobby => "lobby",
            Event::RoomLeft => "room_left",
            Event::PlayerDead => "player_dead",
            Event::TokenSpawn { .. } => "token_spawn",
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
        let n = rest.strip_suffix(" STRIKE damage")?.parse().ok()?;
        return Some(Event::DealtStrike(n));
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
            let (name, _) = rest.split_once("(Clone)")?;
            return Some(Event::BossFight {
                name: name.to_string(),
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
    if let Some(rest) = msg.strip_prefix("Advancing Stage Progress to: ") {
        return Some(Event::StageProgress(rest.trim().parse().ok()?));
    }
    if let Some(rest) = msg.strip_prefix("Boss ") {
        let name = rest
            .strip_suffix("dead, personal damage dealt: ")?
            .trim_end();
        return Some(Event::BossDead {
            name: name.to_string(),
        });
    }
    if msg.trim_end() == "[Behaviour] OnLeftRoom" {
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
    None
}

#[cfg(test)]
mod tests {
    use super::*;

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
                name: "ObisidusPhase2".into()
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
            Some(Event::DealtStrike(140))
        );
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
        assert!(
            split_line("  at \u{cc}\u{ce}\u{ce}\u{cc}.OnLeftRoom () [0x00000] in <0>:0 ").is_none()
        );
        assert_eq!(
            parse_msg("[Behaviour] Entering Room: Ecliptica - Demo Playtest"),
            None
        );
    }

    #[test]
    fn player_dead() {
        let l =
            split_line("2026.09.08 08:20:42 Debug      -  Local controller dead, switching off.")
                .unwrap();
        assert_eq!(parse_msg(l.msg), Some(Event::PlayerDead));
        assert_eq!(parse_msg("Tracking boss as defeated in-run."), None);
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
        assert_eq!(parse_msg("spawn token, Maybe, 5"), None);
    }

    #[test]
    fn stage_progress() {
        assert_eq!(
            parse_msg("Advancing Stage Progress to: 5"),
            Some(Event::StageProgress(5))
        );
        assert_eq!(parse_msg("Advancing Stage Event: 4"), None);
        assert_eq!(parse_msg("Advancing Stage Progress to: x"), None);
    }

    #[test]
    fn non_events() {
        assert_eq!(parse_msg("Retiring Enemy POOL ID19"), None);
        assert_eq!(parse_msg("2.5"), None);
        assert_eq!(parse_msg("ECLIPTICA saving SESSION ID 19854"), None);
        assert_eq!(parse_msg("Backup Active, swapping..."), None);
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
