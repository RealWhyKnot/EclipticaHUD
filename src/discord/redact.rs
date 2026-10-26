const HIDDEN: &str = "[redacted]";

const LINE_CLIP: usize = 1024;

const SECRET_KEYS: &[&str] = &["secret", "join", "spectate", "match"];

fn is_id(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-'
}

fn is_instance(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"~()_-.%".contains(&b)
}

fn run(s: &[u8], ok: fn(u8) -> bool) -> usize {
    s.iter().take_while(|&&b| ok(b)).count()
}

fn starts(s: &[u8], prefix: &[u8]) -> bool {
    s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix)
}

fn quoted_value(s: &[u8]) -> Option<(usize, usize)> {
    let key = SECRET_KEYS.iter().find(|k| {
        let k = k.as_bytes();
        s.len() > k.len() + 1 && &s[1..=k.len()] == k && s[k.len() + 1] == b'"'
    })?;
    let ws = |s: &[u8]| s.iter().take_while(|b| b.is_ascii_whitespace()).count();
    let mut at = key.len() + 2;
    at += ws(&s[at..]);
    if s.get(at) != Some(&b':') {
        return None;
    }
    at += 1;
    at += ws(&s[at..]);
    if s.get(at) != Some(&b'"') {
        return None;
    }
    at += 1;
    let mut end = at;
    while end < s.len() && s[end] != b'"' {
        end += if s[end] == b'\\' { 2 } else { 1 };
    }
    Some((at, end.min(s.len())))
}

fn secret_at(s: &[u8]) -> Option<(usize, usize)> {
    match s[0] {
        b'"' => quoted_value(s),
        b'w' | b'W' if starts(s, b"wrld_") => {
            let colon = 5 + run(&s[5..], is_id);
            if s.get(colon) != Some(&b':') {
                return None;
            }
            let end = colon + 1 + run(&s[colon + 1..], is_instance);
            (end > colon + 1).then_some((colon + 1, end))
        }
        b'u' | b'U' if starts(s, b"usr_") => {
            let end = 4 + run(&s[4..], is_id);
            (end > 4).then_some((4, end))
        }
        b'i' | b'I' if starts(s, b"instanceid=") => {
            let end = 11 + run(&s[11..], is_instance);
            (end > 11).then_some((11, end))
        }
        _ => None,
    }
}

pub(super) fn redact(msg: &str) -> String {
    let b = msg.as_bytes();
    let mut out = String::with_capacity(msg.len());
    let (mut at, mut copied) = (0, 0);
    while at < b.len() {
        if is_instance(b[at]) && (at == 0 || !is_instance(b[at - 1])) {
            let n = run(&b[at..], is_instance);
            if b[at..at + n].contains(&b'~') {
                out.push_str(&msg[copied..at]);
                out.push_str(HIDDEN);
                at += n;
                copied = at;
                continue;
            }
        }
        match secret_at(&b[at..]) {
            Some((keep, end)) => {
                out.push_str(&msg[copied..at + keep]);
                out.push_str(HIDDEN);
                at += end;
                copied = at;
            }
            None => at += 1,
        }
    }
    out.push_str(&msg[copied..]);
    out
}

pub(super) fn log_line(msg: &str) -> String {
    redact(msg)
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(LINE_CLIP)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;

    const UID: &str = "4e64b21b-fbd0-4c12-8b55-c8c500b517b1";
    const LOC: &str = "wrld_0fb88df3-2057-4c2f-8e06-e948864378fd:87887~hidden(usr_4e64b21b-fbd0-4c12-8b55-c8c500b517b1)~region(use)";
    const WORLD: &str = "wrld_0fb88df3-2057-4c2f-8e06-e948864378fd";

    fn leaks(s: &str) -> Option<String> {
        let lower = s.to_ascii_lowercase();
        let b = lower.as_bytes();
        for (i, _) in lower.match_indices("usr_") {
            if b.get(i + 4).is_some_and(|c| c.is_ascii_alphanumeric()) {
                return Some(format!("user id at {i}"));
            }
        }
        for (i, _) in lower.match_indices("instanceid=") {
            if b.get(i + 11).is_some_and(|c| c.is_ascii_alphanumeric()) {
                return Some(format!("instance id at {i}"));
            }
        }
        for needle in [
            "4e64b21b",
            "c8c500b517b1",
            "87887",
            "hidden(",
            "8jov9xedpo",
            "nonce(",
        ] {
            if lower.contains(needle) {
                return Some(format!("{needle:?} found"));
            }
        }
        for (i, _) in lower.match_indices("wrld_") {
            let id = run(&b[i + 5..], is_id);
            if b.get(i + 5 + id) == Some(&b':') && b.get(i + 6 + id).is_some_and(|&c| c != b'[') {
                return Some(format!("location at {i}"));
            }
        }
        None
    }

    fn assert_clean(s: &str) {
        if let Some(why) = leaks(s) {
            panic!("{why}: {s}");
        }
    }

    fn reply_for(location: &str) -> String {
        let mut gs = GameState::default();
        let mut feed = |t: &str, msg: &str| {
            gs.feed(&format!("2026.09.14 {t} Debug      -  {msg}"))
                .unwrap()
        };
        feed("03:59:35", &format!("[Behaviour] Joining {location}"));
        let now = feed(
            "04:02:02",
            "ECLIPTICA - now in stage: Stage_BalboaRuins on phase: 0.19 as class: Nekomancer",
        );
        let a = activity(&gs, now, 1_800_000_000).unwrap();
        assert!(a.contains(location) || a.contains("instanceId="), "{a}");
        format!("{{\"cmd\":\"SET_ACTIVITY\",\"data\":{a},\"evt\":null,\"nonce\":\"3\"}}")
    }

    #[test]
    fn redacts_locations_users_and_join_urls() {
        let reply = reply_for(LOC);
        assert!(reply.contains(LOC) && reply.contains("\"join\":"));
        let line = log_line(&format!("reply {reply}"));
        assert_clean(&line);
        assert!(line.contains("\"join\":\"[redacted]\""), "{line}");
        assert!(
            line.contains(&format!("worldId={WORLD}&instanceId=[redacted]")),
            "{line}"
        );
        assert!(line.contains("\"cmd\":\"SET_ACTIVITY\"") && line.contains("\"nonce\":\"3\""));
        assert!(line.contains("Balboa"), "{line}");

        let long = format!("{LOC}~nonce({UID}{UID}{UID})");
        let reply = reply_for(&long);
        assert!(reply.contains("Join in VRChat") && !reply.contains("\"join\":"));
        assert_clean(&log_line(&reply));

        for msg in [
            format!("joining {LOC}"),
            format!("vrchat://launch?ref=vrchat.com&id={LOC}"),
            format!("{WORLD}:12345~hidden(8JoV9XEdpo)~region(eu)"),
            format!("USR_{UID} and Usr_{UID} and InstanceID=87887~region(us)"),
            format!("{{\"secret\" : \"{LOC}\"}}"),
            format!("{{\"secret\":\"weird \\\"quoted\\\" 8JoV9XEdpo {UID}\"}}"),
            format!("{{\"spectate\":\"{UID}\",\"match\":\"87887\"}}"),
            format!(
                "https://vrchat.com/home/launch?worldId={WORLD}&instanceId=87887~hidden(usr_{UID})"
            ),
        ] {
            assert_clean(&log_line(&msg));
        }
        assert_eq!(
            log_line(&format!("joining {WORLD}")),
            format!("joining {WORLD}")
        );
        assert_eq!(redact(&redact(&reply)), redact(&reply));
    }

    #[test]
    fn every_cut_of_a_reply_stays_clean() {
        let reply = reply_for(LOC);
        let join = format!(
            "{{\"cmd\":\"DISPATCH\",\"data\":{{\"secret\":\"{LOC}\"}},\"evt\":\"ACTIVITY_JOIN\"}}"
        );
        for text in [reply, join] {
            for (i, _) in text.char_indices() {
                assert_clean(&log_line(&text[..i]));
                assert_clean(&log_line(&text[i..]));
                assert_clean(
                    &redact(&text[..i])
                        .chars()
                        .take(DIAG_CLIP)
                        .collect::<String>(),
                );
            }
        }
    }

    #[test]
    fn oversized_and_malformed_payloads_stay_clean() {
        let reply = reply_for(LOC);
        let huge = format!("{}{reply}", "x".repeat(70_000));
        let line = log_line(&format!("connection lost: {huge}"));
        assert!(line.chars().count() <= LINE_CLIP);
        assert_clean(&line);
        let many = reply.repeat(200);
        assert_clean(&redact(&many));
        assert_clean(&log_line(&many));

        let bytes: Vec<u8> = reply
            .bytes()
            .enumerate()
            .map(|(i, b)| if i % 17 == 0 { 0xff } else { b })
            .collect();
        let line = log_line(&String::from_utf8_lossy(&bytes));
        assert!(!line.contains("4e64b21b-fbd0"));
        assert!(!line.contains("87887~hidden"));

        for msg in [
            format!("{{\"join\":\"{LOC}"),
            format!("{{\"join\":\"{LOC}\\"),
            format!("{{\"join\": \t\"\\\"{LOC}"),
            format!("{{\"secret\":\n\"{LOC}\"}}"),
            format!("\u{e9}usr_{UID}\u{e9}{LOC}\u{1F600}"),
            format!("discord error: {{\"message\":\"Invalid secret {LOC}\"\r\n}}"),
            format!(
                "{{\"data\":{{\"code\":4000,\"message\":\"bad\"}},\"secrets\":{{\"join\":\"{LOC}\"}}"
            ),
            format!("\"join\"\"join\":\"{LOC}\""),
            format!("wrld_:{LOC}"),
            format!("{WORLD}{WORLD}:{UID}"),
        ] {
            let line = log_line(&msg);
            assert_clean(&line);
            assert!(!line.contains('\n') && !line.contains('\r'));
        }
        for msg in [
            "",
            "\"",
            "\"join",
            "\"join\"",
            "\"join\":",
            "wrld_",
            "usr_",
            "instanceId=",
        ] {
            assert_eq!(log_line(msg), msg);
        }
    }

    #[test]
    fn diag_never_writes_a_location() {
        take_logged();
        let reply = reply_for(LOC);
        diag(&format!("reply {reply}"));
        diag(&format!("connection lost: {LOC}"));
        diag(&format!(
            "handshake on discord-ipc-0 failed: {{\"secret\":\"{LOC}\"}}"
        ));
        launch_vrchat(LOC);
        let logged = take_logged();
        assert_eq!(logged.len(), 4);
        assert_eq!(logged[3], format!("joining {WORLD}"));
        for line in &logged {
            assert_clean(line);
        }
    }
}
