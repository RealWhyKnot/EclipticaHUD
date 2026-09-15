pub fn split_source(source: &str) -> (&str, &str) {
    let s = source.trim();
    if let Some(rest) = s.strip_prefix('(') {
        if let Some(pos) = rest.rfind(')') {
            return (rest[..pos].trim(), rest[pos + 1..].trim());
        }
    }
    ("", s)
}

fn words(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let mut prev: Option<char> = None;
    for c in s.chars() {
        if c == '_' {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            prev = Some(' ');
            continue;
        }
        let boundary = match prev {
            Some(p) => {
                (c.is_ascii_uppercase() && p.is_ascii_lowercase())
                    || (c.is_ascii_digit() && !p.is_ascii_digit() && p != ' ')
                    || (c.is_ascii_alphabetic() && p.is_ascii_digit())
            }
            None => false,
        };
        if boundary && !out.ends_with(' ') {
            out.push(' ');
        }
        if out.is_empty() || out.ends_with(' ') {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
        prev = Some(c);
    }
    out.trim().to_string()
}

pub fn pretty_attack(attack: &str) -> String {
    let mut s = attack.trim();
    let mut suffix = "";
    if let Some(open) = s.rfind(" (") {
        if s.ends_with(')') {
            let inner = &s[open + 2..s.len() - 1];
            if !inner.bytes().all(|b| b.is_ascii_digit()) {
                suffix = inner;
            }
            s = &s[..open];
        }
    }
    let s = s.strip_prefix("attack_").unwrap_or(s);
    let s = s.strip_suffix("_VFX").unwrap_or(s);
    let s = s.strip_suffix("Hitbox").unwrap_or(s);
    let s = s
        .strip_suffix("Damage")
        .filter(|r| !r.is_empty())
        .unwrap_or(s);
    if s.is_empty() {
        return "hit".to_string();
    }
    let mut out = words(s);
    if !suffix.is_empty() {
        out.push_str(&format!(" ({})", suffix.to_lowercase()));
    }
    out
}

fn attacker_name(who: &str) -> String {
    if let Some(key) = who
        .strip_prefix("[Missing Key \"")
        .and_then(|r| r.strip_suffix("\"]"))
    {
        let key = key.strip_prefix("e_").unwrap_or(key);
        return match key {
            "VirtueBeam" => "Black Virtue".to_string(),
            "GravetenderOrb" => "Gravetender Orb".to_string(),
            other => words(other),
        };
    }
    who.to_string()
}

pub fn describe_source(source: &str) -> (String, String) {
    let (who, attack) = split_source(source);
    if !who.is_empty() {
        return (attacker_name(who), pretty_attack(attack));
    }
    if !attack.is_empty() {
        return ("enemy".to_string(), pretty_attack(attack));
    }
    ("unknown".to_string(), "no source".to_string())
}

pub fn generic_attacker(who: &str) -> bool {
    matches!(who, "enemy" | "unknown")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_shapes() {
        assert_eq!(
            split_source("(Khepri) attack_Claws2"),
            ("Khepri", "attack_Claws2")
        );
        assert_eq!(split_source("attack_Spit (2)"), ("", "attack_Spit (2)"));
        assert_eq!(
            split_source("machinegunShooter2"),
            ("", "machinegunShooter2")
        );
        assert_eq!(split_source(""), ("", ""));
        assert_eq!(
            split_source("([Missing Key \"e_VirtueBeam\"]) damageTick"),
            ("[Missing Key \"e_VirtueBeam\"]", "damageTick")
        );
        assert_eq!(pretty_attack("attack_Spit (2)"), "Spit");
        assert_eq!(pretty_attack("attack_Claws2"), "Claws 2");
        assert_eq!(pretty_attack("machinegunShooter2"), "Machinegun Shooter 2");
        assert_eq!(pretty_attack("Frost Shots (1)"), "Frost Shots");
        assert_eq!(pretty_attack("attack_BasicSlam"), "Basic Slam");
        assert_eq!(pretty_attack("FrostAuraDamage"), "Frost Aura");
        assert_eq!(pretty_attack("NukeHitbox (BIG)"), "Nuke (big)");
        assert_eq!(pretty_attack("LightningHitbox (13)"), "Lightning");
        assert_eq!(pretty_attack("GunSwing_VFX"), "Gun Swing");
        assert_eq!(pretty_attack("satellite_4"), "Satellite 4");
        assert_eq!(pretty_attack("projectile1Aimed"), "Projectile 1 Aimed");
        assert_eq!(pretty_attack("attack_DespairNuke"), "Despair Nuke");
        assert_eq!(pretty_attack("damageTick"), "Damage Tick");
        assert_eq!(pretty_attack("NX-Obsidian"), "NX-Obsidian");
        assert_eq!(pretty_attack(""), "hit");
        let d = |s: &str| {
            let (w, a) = describe_source(s);
            format!("{w}|{a}")
        };
        assert_eq!(d("(Yuki) frostBeam"), "Yuki|Frost Beam");
        assert_eq!(d("(Khepri) attack_Claws2"), "Khepri|Claws 2");
        assert_eq!(d("(The Gravetender) attack_roar"), "The Gravetender|Roar");
        assert_eq!(d("attack_Spit (2)"), "enemy|Spit");
        assert_eq!(d("machinegunShooter2"), "enemy|Machinegun Shooter 2");
        assert_eq!(
            d("([Missing Key \"e_VirtueBeam\"]) damageTick"),
            "Black Virtue|Damage Tick"
        );
        assert_eq!(
            d("([Missing Key \"e_GravetenderOrb\"]) damageAura"),
            "Gravetender Orb|Damage Aura"
        );
        assert_eq!(d("([Missing Key \"e_NewThing\"]) zap"), "New Thing|Zap");
        assert_eq!(d("(dmg)"), "dmg|hit");
        assert_eq!(d(""), "unknown|no source");
        assert!(generic_attacker("enemy"));
        assert!(!generic_attacker("Yuki"));
    }
}
