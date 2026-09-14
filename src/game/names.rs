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

pub fn boss_name(internal: &str) -> &str {
    match internal {
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
        _ => internal,
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
        assert_eq!(stage_name("GMBigcity"), "GM_BigCity");
        assert_eq!(stage_name("Hall of Beginnings"), "Hall of Beginnings");
    }
}
