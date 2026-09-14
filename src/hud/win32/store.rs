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

pub(super) fn load_sound() -> bool {
    data_file("sound.txt")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .is_none_or(|t| sound_on_from(&t))
}

pub(super) fn save_sound(on: bool) {
    write_data("sound.txt", if on { "1" } else { "0" }.to_string());
}

pub(super) fn load_topmost() -> bool {
    data_file("top.txt")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .is_none_or(|t| sound_on_from(&t))
}

pub(super) fn save_topmost(on: bool) {
    write_data("top.txt", if on { "1" } else { "0" }.to_string());
}

pub(super) fn discord_on_from(text: &str) -> bool {
    text.trim() == "1"
}

pub(super) fn load_discord() -> bool {
    data_file("discord.txt")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .is_some_and(|t| discord_on_from(&t))
}

pub(super) fn save_discord(on: bool) {
    write_data("discord.txt", if on { "1" } else { "0" }.to_string());
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

#[cfg(test)]
mod tests {
    use super::*;

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
