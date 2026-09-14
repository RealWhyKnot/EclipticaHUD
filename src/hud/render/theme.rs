pub(super) const BG: u32 = rgb(0x14, 0x14, 0x1c);
pub(super) const CARD: u32 = rgb(0x1d, 0x1d, 0x29);
pub(super) const CARD_HI: u32 = rgb(0x2a, 0x2a, 0x3c);
pub(super) const TEXT: u32 = rgb(0xe9, 0xe9, 0xf2);
pub(super) const DIM: u32 = rgb(0x94, 0x94, 0xac);
pub(super) const ACCENT: u32 = rgb(0x8a, 0x6c, 0xff);
pub(super) const DANGER: u32 = rgb(0xff, 0x5c, 0x7c);
pub(super) const AMBER: u32 = rgb(0xff, 0xc8, 0x57);
pub(super) const GOOD: u32 = rgb(0x57, 0xd9, 0x9a);

pub(super) const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn ease_out_cubic(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

pub(super) fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |shift: u32| {
        let ca = ((a >> shift) & 0xff) as f32;
        let cb = ((b >> shift) & 0xff) as f32;
        (lerp(ca, cb, t.clamp(0.0, 1.0)) as u32) << shift
    };
    ch(0) | ch(8) | ch(16)
}

pub(super) const F_TINY: usize = 0;
pub(super) const F_BODY: usize = 1;
pub(super) const F_LABEL: usize = 2;
pub(super) const F_BOSS: usize = 3;
pub(super) const F_BIG: usize = 4;
pub(super) const F_GLYPH: usize = 5;

pub(super) const GLYPH_CLOSE: &str = "\u{E8BB}";
pub(super) const GLYPH_MUTE: &str = "\u{E74F}";
pub(super) const GLYPH_PREV: &str = "\u{E76B}";
pub(super) const GLYPH_NEXT: &str = "\u{E76C}";
pub(super) const GLYPH_LOG: &str = "\u{E81C}";
pub(super) const GLYPH_PIN: &str = "\u{E718}";
pub(super) const GLYPH_UNPIN: &str = "\u{E77A}";
pub(super) const GLYPH_SETTINGS: &str = "\u{E713}";
pub(super) const GLYPH_MINUS: &str = "\u{E738}";
pub(super) const GLYPH_PLUS: &str = "\u{E710}";
