use crate::game::run::base_name;
use std::borrow::Cow;

pub fn phase_name(x: f32) -> &'static str {
    if x < 0.2 {
        "Primal"
    } else if x < 0.4 {
        "Penumbral"
    } else if x < 0.6 {
        "Antumbral"
    } else if x < 0.8 {
        "Umbral"
    } else if x < 1.0 {
        "Eclipse"
    } else {
        "Eye of the Eclipse"
    }
}

pub fn player_summon(name: &str) -> bool {
    name.strip_prefix("Neko")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn known_boss(id: &str) -> Option<&'static str> {
    Some(match id {
        "DarkMouth" => "Darkmouth",
        "FlyLord" => "Beelzebub",
        "Nan" => "NaN",
        "BuffNoob" => "Buff Noob",
        "QueenBug" => "Vesra",
        "Gravetender" => "The Gravetender",
        "BlackLily" => "The Black Lily",
        "Melon" => "Melgor Johnson",
        "JackedPumpkin" => "Jacked O' Lantern",
        "M41D" => "M-41-D",
        "ConeHead" => "Cone Head",
        "GoldenGrouch" => "Golden Grouch",
        "AntKing" => "Khepri",
        "Oone" => "O-One",
        "NeoPilot" => "Neo Pilot",
        "JimBringer" => "Jim C. Bringer",
        "Obisidus" => "Irides",
        "ManalyteAncient" => "Abaddon",
        "Hakli" => "Ha'Kli",
        _ => return None,
    })
}

fn boss_id(internal: &str) -> &str {
    let id = internal.trim();
    base_name(id.strip_suffix("(Clone)").unwrap_or(id).trim_end())
}

fn readable(id: &str) -> String {
    let chars: Vec<char> = id
        .chars()
        .map(|c| {
            if c == '_' || c.is_whitespace() {
                ' '
            } else {
                c
            }
        })
        .collect();
    let mut out = String::with_capacity(chars.len() + chars.len() / 4);
    for (i, &c) in chars.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| chars[j]);
        let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
        let split = prev.is_some_and(|p| {
            (c.is_uppercase() && p.is_lowercase())
                || (c.is_uppercase() && p.is_uppercase() && next_lower)
                || (c.is_ascii_digit() && p.is_lowercase())
                || (c.is_uppercase() && p.is_ascii_digit() && next_lower)
        });
        if split {
            out.push(' ');
        }
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn boss_name(internal: &str) -> Cow<'_, str> {
    let id = boss_id(internal);
    if let Some(name) = known_boss(id) {
        return Cow::Borrowed(name);
    }
    let shown = readable(id);
    if shown.is_empty() || shown == internal {
        Cow::Borrowed(internal)
    } else {
        Cow::Owned(shown)
    }
}

pub fn stage_name(internal: &str) -> &str {
    match internal {
        "BalboaRuins" => "Balboa Ruins",
        "Bringer" => "Bringer's Desert",
        "CopiedCity" => "Copied City",
        "GMBigcity" => "GM_BigCity",
        "GMFuncFlat" => "GM_Func_Flat",
        "LostElysia" => "Lost Elysia",
        "OuterHeven" => "Outer Heven",
        "ProtoColony" => "Proto Colony",
        "VRCHub" => "VRChat Hub",
        _ => internal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_boundaries() {
        assert_eq!(phase_name(0.0), "Primal");
        assert_eq!(phase_name(0.2), "Penumbral");
        assert_eq!(phase_name(0.4324887), "Antumbral");
        assert_eq!(phase_name(0.8158672), "Eclipse");
        assert_eq!(phase_name(1.0), "Eye of the Eclipse");
    }

    #[test]
    fn mapping_and_passthrough() {
        assert_eq!(boss_name("FlyLord"), "Beelzebub");
        assert_eq!(boss_name("Oone"), "O-One");
        assert_eq!(boss_name("Obisidus"), "Irides");
        assert_eq!(boss_name("ManalyteAncient"), "Abaddon");
        assert_eq!(boss_name("Kodama"), "Kodama");
        assert_eq!(boss_name("Hakli"), "Ha'Kli");
        assert_eq!(boss_name("Gilgamesh"), "Gilgamesh");
        assert_eq!(stage_name("OuterHeven"), "Outer Heven");
        assert_eq!(stage_name("Hallow"), "Hallow");
        assert_eq!(stage_name("GMBigcity"), "GM_BigCity");
        assert_eq!(stage_name("Hall of Beginnings"), "Hall of Beginnings");
    }

    #[test]
    fn unknown_bosses_get_readable_names() {
        assert_eq!(boss_name("OnyxPilot"), "Onyx Pilot");
        assert_eq!(boss_name("QueenVolt"), "Queen Volt");
        assert_eq!(boss_name("Zaveah"), "Zaveah");
        assert_eq!(boss_name("Bravera"), "Bravera");
        assert_eq!(boss_name("Corus"), "Corus");
        assert_eq!(boss_name("Steven"), "Steven");
        assert_eq!(boss_name("OnyxPilotPhase2"), "Onyx Pilot");
        assert_eq!(boss_name("OnyxPilot(Clone)"), "Onyx Pilot");
        assert_eq!(boss_name("  OnyxPilotPhase3(Clone) "), "Onyx Pilot");
        assert_eq!(boss_name("FlyLordPhase2"), "Beelzebub");
        assert_eq!(boss_name("Hakli(Clone)"), "Ha'Kli");
        assert_eq!(boss_name("Phase2"), "Phase 2");
        assert_eq!(boss_name("Big__Boss_"), "Big Boss");
        assert_eq!(boss_name("VRCHub"), "VRC Hub");
        assert_eq!(boss_name("Boss10"), "Boss 10");
        assert_eq!(boss_name("Mk2Golem"), "Mk 2 Golem");
        assert_eq!(boss_name("R2D2"), "R2D2");
        assert_eq!(boss_name("ABCDEFGH"), "ABCDEFGH");
        assert_eq!(boss_name("BOSS_KING"), "BOSS KING");
    }

    #[test]
    fn odd_boss_ids_never_break() {
        assert_eq!(boss_name(""), "");
        assert_eq!(boss_name("x"), "x");
        assert_eq!(boss_name("___"), "___");
        assert_eq!(boss_name("(Clone)"), "(Clone)");
        assert_eq!(boss_name("ÉclairBête"), "Éclair Bête");
        assert_eq!(boss_name("東京Boss"), "東京Boss");
        assert_eq!(
            boss_name("Ωmega\u{301}Lord").replace(' ', ""),
            "Ωmega\u{301}Lord"
        );
        assert!(matches!(boss_name("Kodama"), Cow::Borrowed("Kodama")));
        let huge = "AbCd".repeat(5000);
        let shown = boss_name(&huge);
        assert_eq!(shown.chars().count(), 5000 * 4 + 9999);
        assert!(shown.starts_with("Ab Cd Ab"));
        assert_eq!(shown.trim(), shown);
        let caps = "Q".repeat(10_000);
        assert_eq!(boss_name(&caps), caps);
        let wide = "界".repeat(3000);
        assert_eq!(boss_name(&wide), wide);
    }

    #[test]
    fn readable_is_stable() {
        for id in [
            "OnyxPilot",
            "VRCHub",
            "Mk2Golem",
            "Big__Boss_",
            "ÉclairBête",
            "",
        ] {
            let once = readable(id);
            assert_eq!(readable(&once), once, "{id}");
        }
    }
}
