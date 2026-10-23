use super::*;

pub(super) fn data_file(name: &str) -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(base).join("EclipticaHUD").join(name))
}

pub(super) fn write_data(name: &str, text: String) {
    if let Some(path) = data_file(name) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, text);
    }
}

pub(super) fn load_text(name: &str) -> Option<String> {
    data_file(name).and_then(|p| std::fs::read_to_string(p).ok())
}

pub(super) fn sound_on_from(text: &str) -> bool {
    text.trim() != "0"
}

fn save_flag(name: &str, on: bool) {
    write_data(name, if on { "1" } else { "0" }.to_string());
}

pub(super) fn load_sound() -> bool {
    load_text("sound.txt").is_none_or(|t| sound_on_from(&t))
}

pub(super) fn save_sound(on: bool) {
    save_flag("sound.txt", on);
}

pub(super) fn load_topmost() -> bool {
    load_text("top.txt").is_none_or(|t| sound_on_from(&t))
}

pub(super) fn save_topmost(on: bool) {
    save_flag("top.txt", on);
}

pub(super) fn discord_on_from(text: &str) -> bool {
    text.trim() == "1"
}

pub(super) fn load_discord() -> bool {
    load_text("discord.txt").is_some_and(|t| discord_on_from(&t))
}

pub(super) fn save_discord(on: bool) {
    save_flag("discord.txt", on);
}

pub(super) fn load_usage() -> bool {
    load_text("usage.txt").is_some_and(|t| discord_on_from(&t))
}

pub(super) fn usage_answered() -> bool {
    load_text("usage.txt").is_some()
}

pub(super) fn save_usage(on: bool) {
    save_flag("usage.txt", on);
}

pub(super) fn party_on_from(text: &str) -> bool {
    text.trim() == "1"
}

pub(super) fn load_party() -> bool {
    load_text("party.txt").is_some_and(|t| party_on_from(&t))
}

pub(super) fn save_party(on: bool) {
    save_flag("party.txt", on);
}

pub(super) fn scale_from(text: &str) -> f32 {
    text.trim()
        .parse::<u32>()
        .map_or(1.0, |p| (p as f32 / 100.0).clamp(MIN_SCALE, MAX_SCALE))
}

pub(super) fn save_scale(scale: f32) {
    write_data("scale.txt", ((scale * 100.0).round() as i32).to_string());
}

pub(super) fn alpha_from(text: &str) -> u8 {
    text.trim().parse::<u32>().map_or(MAX_ALPHA, |p| {
        p.clamp(MIN_ALPHA as u32, MAX_ALPHA as u32) as u8
    })
}

pub(super) fn save_alpha(alpha: u8) {
    write_data("alpha.txt", alpha.to_string());
}

pub(super) fn window_from(text: &str) -> u64 {
    text.trim()
        .parse::<u64>()
        .map_or(crate::game::state::DEFAULT_WINDOW, |w| {
            w.clamp(WINDOW_STEPS[0], WINDOW_STEPS[WINDOW_STEPS.len() - 1])
        })
}

pub(super) fn save_window(window: u64) {
    write_data("window.txt", window.to_string());
}

pub(super) fn skipped_from(text: &str) -> Option<String> {
    Some(text.trim())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

pub(super) fn load_skipped() -> Option<String> {
    load_text("skip_update.txt").and_then(|t| skipped_from(&t))
}

pub(super) fn save_skipped(tag: &str) {
    write_data("skip_update.txt", tag.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skipped_version_parse() {
        assert_eq!(
            skipped_from("v2026.10.29.0\r\n").as_deref(),
            Some("v2026.10.29.0")
        );
        assert_eq!(skipped_from("  \n"), None);
        assert_eq!(skipped_from(""), None);
    }

    #[test]
    fn sound_default_and_parse() {
        assert!(!sound_on_from("0"));
        assert!(!sound_on_from("0\n"));
        assert!(sound_on_from("1"));
        assert!(sound_on_from(""));
        assert!(sound_on_from("garbage"));
    }

    #[test]
    fn scale_and_alpha_parse() {
        assert_eq!(scale_from("150"), 1.5);
        assert_eq!(scale_from("garbage"), 1.0);
        assert_eq!(scale_from("10"), MIN_SCALE);
        assert_eq!(scale_from("900"), MAX_SCALE);
        assert_eq!(alpha_from("70\n"), 70);
        assert_eq!(alpha_from(""), MAX_ALPHA);
        assert_eq!(alpha_from("5"), MIN_ALPHA);
        assert_eq!(alpha_from("300"), MAX_ALPHA);
        assert_eq!(window_from("15"), 15);
        assert_eq!(window_from("x"), 10);
        assert_eq!(window_from("1"), 3);
        assert_eq!(window_from("99"), 30);
    }

    #[test]
    fn party_sharing_defaults_off() {
        assert!(data_file("party.txt").is_some_and(|p| p.ends_with("party.txt")));
        for (text, on) in [
            ("", false),
            ("0", false),
            ("0\n", false),
            ("on", false),
            ("true", false),
            ("11", false),
            ("1", true),
            ("1\r\n", true),
            (" 1 ", true),
        ] {
            assert_eq!(party_on_from(text), on, "{text:?}");
        }
    }

    #[test]
    fn discord_defaults_off() {
        assert!(!discord_on_from(""));
        assert!(!discord_on_from("0"));
        assert!(!discord_on_from("garbage"));
        assert!(discord_on_from(
            "1
"
        ));
    }
}
