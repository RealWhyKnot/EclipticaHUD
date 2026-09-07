use crate::parse::{fmt_clock, GameState, Mode};
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::*;

pub const LOGICAL_W: i32 = 360;
pub const LOGICAL_H: i32 = 560;

const BG: u32 = rgb(0x14, 0x14, 0x1c);
const CARD: u32 = rgb(0x1d, 0x1d, 0x29);
const TEXT: u32 = rgb(0xe9, 0xe9, 0xf2);
const DIM: u32 = rgb(0x94, 0x94, 0xac);
const ACCENT: u32 = rgb(0x8a, 0x6c, 0xff);
const DANGER: u32 = rgb(0xff, 0x5c, 0x7c);
const AMBER: u32 = rgb(0xff, 0xc8, 0x57);

const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}

pub struct Renderer {
    pub width: i32,
    pub height: i32,
    pub dc: HDC,
    bitmap: HBITMAP,
    pub bits: *mut u8,
    scale: f32,
    fonts: [HFONT; 5],
}

const F_TINY: usize = 0;
const F_BODY: usize = 1;
const F_LABEL: usize = 2;
const F_BOSS: usize = 3;
const F_BIG: usize = 4;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

impl Renderer {
    pub fn new(dpi: u32) -> Self {
        let scale = dpi as f32 / 96.0;
        let width = (LOGICAL_W as f32 * scale) as i32;
        let height = (LOGICAL_H as f32 * scale) as i32;
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = width;
            info.bmiHeader.biHeight = -height;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
            SelectObject(dc, bitmap as _);
            SetBkMode(dc, TRANSPARENT as _);
            let font = |pt: i32, weight: i32| {
                let name = wide("Segoe UI\0");
                CreateFontW(
                    -((pt as f32 * scale) as i32),
                    0, 0, 0, weight, 0, 0, 0,
                    DEFAULT_CHARSET as u32,
                    OUT_DEFAULT_PRECIS as u32,
                    CLIP_DEFAULT_PRECIS as u32,
                    CLEARTYPE_QUALITY as u32,
                    (DEFAULT_PITCH | FF_DONTCARE) as u32,
                    name.as_ptr(),
                )
            };
            let fonts = [
                font(11, FW_NORMAL as i32),
                font(13, FW_NORMAL as i32),
                font(11, FW_SEMIBOLD as i32),
                font(16, FW_SEMIBOLD as i32),
                font(26, FW_BOLD as i32),
            ];
            Renderer { width, height, dc, bitmap, bits: bits as *mut u8, scale, fonts }
        }
    }

    fn px(&self, v: i32) -> i32 {
        (v as f32 * self.scale) as i32
    }

    fn fill(&self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        unsafe {
            let brush = CreateSolidBrush(color);
            let r = RECT { left: self.px(x), top: self.px(y), right: self.px(x + w), bottom: self.px(y + h) };
            FillRect(self.dc, &r, brush);
            DeleteObject(brush as _);
        }
    }

    fn text(&self, x: i32, y: i32, w: i32, font: usize, color: u32, flags: u32, s: &str) {
        unsafe {
            SelectObject(self.dc, self.fonts[font] as _);
            SetTextColor(self.dc, color);
            let mut r = RECT {
                left: self.px(x),
                top: self.px(y),
                right: self.px(x + w),
                bottom: self.px(y) + self.px(40),
            };
            let mut buf = wide(s);
            DrawTextW(
                self.dc,
                buf.as_mut_ptr(),
                buf.len() as i32,
                &mut r,
                flags | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
        }
    }

    pub fn draw(&mut self, gs: &mut GameState, now: u64, flash: bool) {
        const M: i32 = 14;
        const W: i32 = LOGICAL_W - 2 * M;
        self.fill(0, 0, LOGICAL_W, LOGICAL_H, BG);

        self.text(M, 10, W, F_LABEL, ACCENT, DT_LEFT, "ECLIPTICA HUD");
        self.text(M, 10, W - 20, F_TINY, DIM, DT_RIGHT, "x");
        let status = match gs.mode {
            Mode::Idle => "waiting for a run".to_string(),
            Mode::Lobby => "in lobby".to_string(),
            Mode::Intermission => "intermission".to_string(),
            Mode::Stage => format!(
                "{}   {:.0}%   {}",
                gs.stage,
                gs.progress * 100.0,
                gs.class
            ),
        };
        self.text(M, 28, W, F_BODY, TEXT, DT_LEFT, &status);

        self.fill(M, 54, W, 96, CARD);
        match (&gs.boss, &gs.target) {
            (Some(boss), target) => {
                self.text(M + 12, 60, W - 24, F_LABEL, DIM, DT_LEFT, "BOSS");
                self.text(M + 60, 60, W - 84, F_BOSS, TEXT, DT_LEFT, boss);
                self.text(M + 12, 86, W - 24, F_LABEL, DIM, DT_LEFT, "TARGET");
                let color = if flash { ACCENT } else { TEXT };
                match target {
                    Some(t) => {
                        self.text(M + 12, 100, W - 24, F_BIG, color, DT_LEFT, t);
                        let held = now.saturating_sub(gs.target_since);
                        self.text(M + 12, 100, W - 24, F_TINY, DIM, DT_RIGHT, &format!("{held}s"));
                    }
                    None => self.text(M + 12, 100, W - 24, F_BOSS, DIM, DT_LEFT, "-"),
                }
            }
            (None, _) => {
                self.text(M + 12, 60, W - 24, F_LABEL, DIM, DT_LEFT, "BOSS");
                self.text(M + 12, 84, W - 24, F_BOSS, DIM, DT_LEFT, "no boss active");
            }
        }

        self.fill(M, 158, W, 92, CARD);
        let col = (W - 24) / 3;
        let stat = |r: &Renderer, i: i32, label: &str, value: String| {
            let x = M + 12 + i * col;
            r.text(x, 164, col, F_LABEL, DIM, DT_LEFT, label);
            r.text(x, 180, col, F_BOSS, AMBER, DT_LEFT, &value);
        };
        stat(self, 0, "DPS 10s", gs.rolling_dps(now).to_string());
        stat(self, 1, "FIGHT DPS", gs.fight_dps(now).to_string());
        stat(self, 2, "FIGHT DMG", group_digits(gs.fight_dmg));
        match &gs.last_kill {
            Some(k) => {
                let line = format!(
                    "last kill  {}   {} strike + {} other",
                    k.boss,
                    group_digits(k.strike),
                    group_digits(k.non_strike)
                );
                self.text(M + 12, 214, W - 24, F_BODY, TEXT, DT_LEFT, &line);
            }
            None => self.text(M + 12, 214, W - 24, F_BODY, DIM, DT_LEFT, "no kills yet"),
        }

        self.text(M, 258, W, F_LABEL, DIM, DT_LEFT, "DAMAGE TAKEN");
        let mut y = 274;
        for hit in gs.taken.iter().rev().take(3) {
            let src = if hit.source.is_empty() { "environment" } else { &hit.source };
            self.text(M, y, 40, F_BODY, DANGER, DT_LEFT, &hit.amount.to_string());
            self.text(M + 44, y, W - 110, F_BODY, TEXT, DT_LEFT, src);
            self.text(M, y, W, F_TINY, DIM, DT_RIGHT, &fmt_clock(hit.ts));
            y += 20;
        }
        if gs.taken.is_empty() {
            self.text(M, y, W, F_BODY, DIM, DT_LEFT, "none");
        }

        self.text(M, 340, W, F_LABEL, DIM, DT_LEFT, "TARGET HISTORY");
        let mut y = 356;
        for entry in gs.history.iter().rev() {
            if y > LOGICAL_H - 24 {
                break;
            }
            self.text(M, y, 58, F_BODY, DIM, DT_LEFT, &fmt_clock(entry.ts));
            self.text(M + 62, y, 150, F_BODY, TEXT, DT_LEFT, &entry.player);
            self.text(M + 216, y, W - 216, F_BODY, DIM, DT_LEFT, &entry.boss);
            y += 20;
        }
        if gs.history.is_empty() {
            self.text(M, y, W, F_BODY, DIM, DT_LEFT, "none yet");
        }
        unsafe { GdiFlush() };
    }

    pub fn close_hit(&self, x: i32, y: i32) -> bool {
        x > self.width - self.px(34) && y < self.px(30)
    }

    pub fn rgba(&self, out: &mut Vec<u8>) {
        let n = (self.width * self.height * 4) as usize;
        out.resize(n, 0);
        let src = unsafe { std::slice::from_raw_parts(self.bits, n) };
        for (d, s) in out.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = 255;
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            for f in self.fonts {
                DeleteObject(f as _);
            }
            DeleteObject(self.bitmap as _);
            DeleteDC(self.dc);
        }
    }
}

fn group_digits(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}
