use crate::state::{base_name, fmt_clock, phase_num, GameState, Mode};
use crate::update::{Badge, VERSION};
use crate::vr::VrStatus;
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::*;

pub const LOGICAL_W: i32 = 360;
pub const LOGICAL_H: i32 = 600;

pub const CLOSE_HIT: (i32, i32, i32, i32) = (324, 0, 36, 36);
pub const CLOSE_BTN: (i32, i32, i32, i32) = (328, 8, 24, 24);
pub const UPDATE_HIT: (i32, i32) = (210, LOGICAL_H - 32);
pub const TARGET_HIT: (i32, i32, i32, i32) = (26, 126, 308, 40);
pub const RUN_PREV_HIT: (i32, i32, i32, i32) = (14, 54, 26, 24);
pub const RUN_NEXT_HIT: (i32, i32, i32, i32) = (320, 54, 26, 24);
pub const FIGHT_PREV_HIT: (i32, i32, i32, i32) = (240, 86, 26, 20);
pub const FIGHT_NEXT_HIT: (i32, i32, i32, i32) = (308, 86, 26, 20);
pub const PHASE_HIT: (i32, i32, i32, i32) = (220, 110, 114, 20);

const BG: u32 = rgb(0x14, 0x14, 0x1c);
const CARD: u32 = rgb(0x1d, 0x1d, 0x29);
const TEXT: u32 = rgb(0xe9, 0xe9, 0xf2);
const DIM: u32 = rgb(0x94, 0x94, 0xac);
const ACCENT: u32 = rgb(0x8a, 0x6c, 0xff);
const DANGER: u32 = rgb(0xff, 0x5c, 0x7c);
const AMBER: u32 = rgb(0xff, 0xc8, 0x57);
const GOOD: u32 = rgb(0x57, 0xd9, 0x9a);

const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn ease_out_cubic(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |shift: u32| {
        let ca = ((a >> shift) & 0xff) as f32;
        let cb = ((b >> shift) & 0xff) as f32;
        (lerp(ca, cb, t.clamp(0.0, 1.0)) as u32) << shift
    };
    ch(0) | ch(8) | ch(16)
}

pub struct Frame {
    pub now: u64,
    pub flash_t: f32,
    pub hover_close: bool,
    pub pressed_close: bool,
    pub vr: VrStatus,
    pub log_ok: bool,
    pub progress_shown: f32,
    pub dps_frac_shown: f32,
    pub update: Badge,
    pub sound_on: bool,
    pub view_run: Option<usize>,
    pub view_group: Option<usize>,
    pub view_phase: Option<usize>,
    pub run_sel: bool,
    pub group_sel: bool,
}

impl Frame {
    fn live(&self) -> bool {
        !self.run_sel && !self.group_sel && self.view_phase.is_none()
    }
}

pub struct Renderer {
    pub width: i32,
    pub height: i32,
    pub dc: HDC,
    bitmap: HBITMAP,
    pub bits: *mut u8,
    scale: f32,
    fonts: [HFONT; 6],
}

const F_TINY: usize = 0;
const F_BODY: usize = 1;
const F_LABEL: usize = 2;
const F_BOSS: usize = 3;
const F_BIG: usize = 4;
const F_GLYPH: usize = 5;

const GLYPH_CLOSE: &str = "\u{E8BB}";
const GLYPH_MUTE: &str = "\u{E74F}";
const GLYPH_PREV: &str = "\u{E76B}";
const GLYPH_NEXT: &str = "\u{E76C}";

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
            let bitmap = CreateDIBSection(
                dc,
                &info,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            );
            SelectObject(dc, bitmap as _);
            SetBkMode(dc, TRANSPARENT as _);
            let font = |pt: i32, weight: i32, face: &str| {
                let name = wide(face);
                CreateFontW(
                    -((pt as f32 * scale) as i32),
                    0,
                    0,
                    0,
                    weight,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET as u32,
                    OUT_DEFAULT_PRECIS as u32,
                    CLIP_DEFAULT_PRECIS as u32,
                    CLEARTYPE_QUALITY as u32,
                    (DEFAULT_PITCH | FF_DONTCARE) as u32,
                    name.as_ptr(),
                )
            };
            let fonts = [
                font(11, FW_NORMAL as i32, "Segoe UI\0"),
                font(13, FW_NORMAL as i32, "Segoe UI\0"),
                font(11, FW_SEMIBOLD as i32, "Segoe UI\0"),
                font(16, FW_SEMIBOLD as i32, "Segoe UI\0"),
                font(26, FW_BOLD as i32, "Segoe UI\0"),
                font(10, FW_NORMAL as i32, "Segoe MDL2 Assets\0"),
            ];
            Renderer {
                width,
                height,
                dc,
                bitmap,
                bits: bits as *mut u8,
                scale,
                fonts,
            }
        }
    }

    fn px(&self, v: i32) -> i32 {
        (v as f32 * self.scale) as i32
    }

    fn fill(&self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        unsafe {
            let brush = CreateSolidBrush(color);
            let r = RECT {
                left: self.px(x),
                top: self.px(y),
                right: self.px(x + w),
                bottom: self.px(y + h),
            };
            FillRect(self.dc, &r, brush);
            DeleteObject(brush as _);
        }
    }

    fn rround(&self, x: i32, y: i32, w: i32, h: i32, r: i32, color: u32) {
        unsafe {
            let old_pen = SelectObject(self.dc, GetStockObject(NULL_PEN) as _);
            let brush = CreateSolidBrush(color);
            let old_brush = SelectObject(self.dc, brush as _);
            RoundRect(
                self.dc,
                self.px(x),
                self.px(y),
                self.px(x + w) + 1,
                self.px(y + h) + 1,
                self.px(r * 2),
                self.px(r * 2),
            );
            SelectObject(self.dc, old_brush);
            SelectObject(self.dc, old_pen);
            DeleteObject(brush as _);
        }
    }

    fn bar(&self, x: i32, y: i32, w: i32, h: i32, frac: f32, color: u32) {
        self.rround(x, y, w, h, h / 2, CARD);
        let fw = (w as f32 * frac.clamp(0.0, 1.0)) as i32;
        if fw >= h {
            self.rround(x, y, fw, h, h / 2, color);
        }
    }

    fn dot(&self, x: i32, y: i32, d: i32, color: u32) {
        self.rround(x, y, d, d, d / 2, color);
    }

    #[allow(clippy::too_many_arguments)]
    fn text(&self, x: i32, y: i32, w: i32, font: usize, color: u32, flags: u32, s: &str) {
        self.text_rect(x, y, w, 40, font, color, flags, s);
    }

    #[allow(clippy::too_many_arguments)]
    fn text_rect(
        &self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        font: usize,
        color: u32,
        flags: u32,
        s: &str,
    ) {
        unsafe {
            SelectObject(self.dc, self.fonts[font] as _);
            SetTextColor(self.dc, color);
            let mut r = RECT {
                left: self.px(x),
                top: self.px(y),
                right: self.px(x + w),
                bottom: self.px(y) + self.px(h),
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

    pub fn draw(&mut self, gs: &mut GameState, f: &Frame) {
        const M: i32 = 14;
        const W: i32 = LOGICAL_W - 2 * M;
        let now = f.now;
        self.fill(0, 0, LOGICAL_W, LOGICAL_H, BG);

        self.text(M, 10, W, F_LABEL, ACCENT, DT_LEFT, "ECLIPTICA HUD");
        let (bx, by, bw, bh) = CLOSE_BTN;
        let glyph_color = if f.pressed_close {
            self.rround(bx, by, bw, bh, 6, ACCENT);
            BG
        } else if f.hover_close {
            self.rround(bx, by, bw, bh, 6, CARD);
            TEXT
        } else {
            DIM
        };
        self.text_rect(
            bx,
            by,
            bw,
            bh,
            F_GLYPH,
            glyph_color,
            DT_CENTER | DT_VCENTER,
            GLYPH_CLOSE,
        );

        let (status, status_color) = match gs.mode {
            Mode::Idle => ("waiting for a run".to_string(), DIM),
            Mode::Lobby => ("in lobby".to_string(), AMBER),
            Mode::Intermission => ("intermission".to_string(), ACCENT),
            Mode::Stage => (
                format!("{}   {:.0}%   {}", gs.stage, gs.progress * 100.0, gs.class),
                GOOD,
            ),
        };
        self.text(M, 28, W, F_BODY, status_color, DT_LEFT, &status);
        if gs.is_dead(now) {
            self.text(M, 28, W, F_BODY, DANGER, DT_RIGHT, "DEAD");
        }
        if gs.mode == Mode::Stage {
            self.bar(M, 47, W, 4, f.progress_shown, GOOD);
        }

        let arrow = |r: &Renderer, hit: (i32, i32, i32, i32), on: bool, glyph: &str| {
            let (x, y, w, h) = hit;
            let color = if on { TEXT } else { CARD };
            r.text_rect(x, y, w, h, F_GLYPH, color, DT_CENTER | DT_VCENTER, glyph);
        };
        arrow(
            self,
            RUN_PREV_HIT,
            f.view_run.is_some_and(|i| i > 0),
            GLYPH_PREV,
        );
        arrow(self, RUN_NEXT_HIT, f.run_sel, GLYPH_NEXT);
        let (run_label, run_color) = match f.view_run {
            None => ("no runs yet".to_string(), DIM),
            Some(i) if f.live() => (format!("RUN {}/{}", i + 1, gs.runs.len()), DIM),
            Some(i) => {
                let r = &gs.runs[i];
                let mut s = format!(
                    "RUN {}/{}   {}   {}",
                    i + 1,
                    gs.runs.len(),
                    &fmt_clock(r.start_ts)[..5],
                    r.stage
                );
                if r.deaths > 0 {
                    s.push_str(&format!("   {}", fmt_deaths(r.deaths)));
                }
                (s, TEXT)
            }
        };
        self.text_rect(
            44,
            54,
            LOGICAL_W - 88,
            24,
            F_LABEL,
            run_color,
            DT_CENTER | DT_VCENTER,
            &run_label,
        );

        self.rround(M, 82, W, 96, 8, CARD);
        let viewed_run = f.view_run.map(|i| &gs.runs[i]);
        let groups = viewed_run.map(|r| r.groups()).unwrap_or_default();
        let viewed_group = viewed_run.and_then(|r| {
            f.view_group
                .and_then(|i| groups.get(i))
                .map(|g| &r.fights[g.clone()])
        });
        let hist = viewed_group.map(|fights| match f.view_phase.and_then(|i| fights.get(i)) {
            Some(ph) => (
                ph.name.as_str(),
                ph.start_ts,
                ph.dmg,
                ph.kill,
                ph,
                fights.len(),
            ),
            None => {
                let last = &fights[fights.len() - 1];
                (
                    base_name(&last.name),
                    fights[0].start_ts,
                    fights.iter().map(|p| p.dmg).sum(),
                    fights
                        .iter()
                        .filter_map(|p| p.kill)
                        .reduce(|a, b| (a.0 + b.0, a.1 + b.1)),
                    last,
                    fights.len(),
                )
            }
        });
        self.text(M + 12, 88, W - 24, F_LABEL, DIM, DT_LEFT, "BOSS");
        if !groups.is_empty() {
            arrow(
                self,
                FIGHT_PREV_HIT,
                f.view_group.is_some_and(|i| i > 0),
                GLYPH_PREV,
            );
            arrow(self, FIGHT_NEXT_HIT, f.group_sel, GLYPH_NEXT);
            let idx = format!(
                "{}/{}",
                f.view_group.map_or(groups.len(), |i| i + 1),
                groups.len()
            );
            self.text_rect(266, 86, 42, 20, F_TINY, DIM, DT_CENTER | DT_VCENTER, &idx);
        }
        if f.live() {
            match (&gs.boss, &gs.target) {
                (Some(boss), target) => {
                    let pn = phase_num(boss);
                    let shown = if pn > 1 {
                        format!("{} (P{pn})", base_name(boss))
                    } else {
                        boss.clone()
                    };
                    self.text(M + 60, 88, 166, F_BOSS, TEXT, DT_LEFT, &shown);
                    self.text(M + 12, 114, W - 24, F_LABEL, DIM, DT_LEFT, "TARGET");
                    if !f.sound_on {
                        self.text(M + 12, 114, W - 24, F_GLYPH, DIM, DT_RIGHT, GLYPH_MUTE);
                    }
                    let color = mix(ACCENT, TEXT, ease_out_cubic(f.flash_t));
                    match target {
                        Some(t) => {
                            self.text(M + 12, 128, W - 24, F_BIG, color, DT_LEFT, t);
                            let held = now.saturating_sub(gs.target_since);
                            self.text(
                                M + 12,
                                128,
                                W - 24,
                                F_TINY,
                                DIM,
                                DT_RIGHT,
                                &format!("{held}s"),
                            );
                        }
                        None => self.text(M + 12, 128, W - 24, F_BOSS, DIM, DT_LEFT, "-"),
                    }
                }
                (None, _) => {
                    self.text(M + 12, 112, W - 24, F_BOSS, DIM, DT_LEFT, "no boss active");
                }
            }
        } else {
            match hist {
                Some((name, start, _, _, last, n_phases)) => {
                    self.text(M + 60, 88, 166, F_BOSS, TEXT, DT_LEFT, name);
                    self.text(M + 12, 114, W - 24, F_LABEL, DIM, DT_LEFT, "RESULT");
                    if n_phases > 1 {
                        let chip = match f.view_phase {
                            Some(i) => format!("phase {}/{n_phases}", i + 1),
                            None => format!("{n_phases} phases"),
                        };
                        let (cx, cy, cw, ch) = PHASE_HIT;
                        self.text_rect(cx, cy, cw, ch, F_TINY, DIM, DT_RIGHT | DT_VCENTER, &chip);
                    }
                    let (res, color) = match (last.kill, last.end_ts) {
                        (Some(_), _) => ("killed", GOOD),
                        (None, Some(_)) => ("unfinished", DIM),
                        (None, None) => ("in progress", AMBER),
                    };
                    self.text(M + 12, 128, W - 24, F_BOSS, color, DT_LEFT, res);
                    let end = last.end_ts.unwrap_or(now);
                    self.text(
                        M + 12,
                        128,
                        W - 24,
                        F_TINY,
                        DIM,
                        DT_RIGHT,
                        &fmt_dur(end.saturating_sub(start)),
                    );
                }
                None => self.text(M + 12, 112, W - 24, F_BOSS, DIM, DT_LEFT, "no boss fights"),
            }
        }

        self.rround(M, 186, W, 92, 8, CARD);
        let col = (W - 24) / 3;
        let stat = |r: &Renderer, i: i32, label: &str, value: String| {
            let x = M + 12 + i * col;
            r.text(x, 192, col, F_LABEL, DIM, DT_LEFT, label);
            r.text(x, 208, col, F_BOSS, AMBER, DT_LEFT, &value);
        };
        if f.live() {
            stat(self, 0, "DPS 10s", gs.rolling_dps(now).to_string());
            stat(self, 1, "FIGHT DPS", gs.fight_dps(now).to_string());
            stat(self, 2, "FIGHT DMG", group_digits(gs.fight_dmg));
            if gs.boss.is_some() {
                self.bar(M + 12, 233, W - 24, 3, f.dps_frac_shown, ACCENT);
            }
            match &gs.last_kill {
                Some(k) => {
                    let line = format!(
                        "last kill  {}   {} strike + {} other",
                        k.boss,
                        group_digits(k.strike),
                        group_digits(k.non_strike)
                    );
                    self.text(M + 12, 242, W - 24, F_BODY, TEXT, DT_LEFT, &line);
                }
                None => self.text(M + 12, 242, W - 24, F_BODY, DIM, DT_LEFT, "no kills yet"),
            }
        } else if let Some((_, start, dmg, kill, last, _)) = hist {
            let end = last.end_ts.unwrap_or(now);
            let dur = end.saturating_sub(start);
            stat(self, 0, "DMG", group_digits(dmg));
            stat(self, 1, "DPS", (dmg / dur.max(1)).to_string());
            stat(self, 2, "TIME", fmt_dur(dur));
            match kill {
                Some((s, ns)) => {
                    let line = format!(
                        "kill  {} strike + {} other",
                        group_digits(s),
                        group_digits(ns)
                    );
                    self.text(M + 12, 242, W - 24, F_BODY, TEXT, DT_LEFT, &line);
                }
                None => {
                    self.text(
                        M + 12,
                        242,
                        W - 24,
                        F_BODY,
                        DIM,
                        DT_LEFT,
                        "no kill recorded",
                    );
                }
            }
        } else {
            self.text(M + 12, 208, W - 24, F_BODY, DIM, DT_LEFT, "no data");
        }

        self.text(M, 286, W, F_LABEL, DIM, DT_LEFT, "DAMAGE TAKEN");
        let live_deaths = gs
            .runs
            .last()
            .filter(|r| r.end_ts.is_none())
            .map_or(0, |r| r.deaths);
        if live_deaths > 0 {
            self.text(M, 286, W, F_TINY, DIM, DT_RIGHT, &fmt_deaths(live_deaths));
        }
        let mut y = 302;
        for hit in gs.taken.iter().rev().take(3) {
            let src = if hit.source.is_empty() {
                "environment"
            } else {
                &hit.source
            };
            self.text(M, y, 40, F_BODY, DANGER, DT_LEFT, &hit.amount.to_string());
            self.text(M + 44, y, W - 110, F_BODY, TEXT, DT_LEFT, src);
            self.text(M, y, W, F_TINY, DIM, DT_RIGHT, &fmt_clock(hit.ts));
            y += 20;
        }
        if gs.taken.is_empty() {
            self.text(M, y, W, F_BODY, DIM, DT_LEFT, "none");
        }

        self.text(M, 368, W, F_LABEL, DIM, DT_LEFT, "TARGET HISTORY");
        let mut y = 384;
        for entry in gs.history.iter().rev() {
            if y > LOGICAL_H - 52 {
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

        let fy = LOGICAL_H - 26;
        let vr_color = match f.vr {
            VrStatus::On => GOOD,
            VrStatus::Off => DIM,
            VrStatus::Failing => DANGER,
        };
        self.dot(M, fy + 4, 8, vr_color);
        self.text(M + 14, fy, 80, F_TINY, DIM, DT_LEFT, "STEAMVR");
        let log_color = if f.log_ok { GOOD } else { AMBER };
        self.dot(M + 96, fy + 4, 8, log_color);
        self.text(M + 110, fy, 80, F_TINY, DIM, DT_LEFT, "LOG");
        let (utext, ucolor) = match &f.update {
            Badge::None => (VERSION.to_string(), DIM),
            Badge::Ready(tag) => (format!("update {tag}"), ACCENT),
            Badge::Installing => ("updating".to_string(), AMBER),
            Badge::Failed => ("update failed".to_string(), DANGER),
        };
        self.text(M, fy, W, F_TINY, ucolor, DT_RIGHT, &utext);
        unsafe { GdiFlush() };
    }

    pub fn close_hit(&self, x: i32, y: i32) -> bool {
        x >= self.px(CLOSE_HIT.0) && y < self.px(CLOSE_HIT.1 + CLOSE_HIT.3)
    }

    pub fn update_hit(&self, x: i32, y: i32) -> bool {
        x >= self.px(UPDATE_HIT.0) && y >= self.px(UPDATE_HIT.1)
    }

    fn in_rect(&self, r: (i32, i32, i32, i32), x: i32, y: i32) -> bool {
        x >= self.px(r.0) && x < self.px(r.0 + r.2) && y >= self.px(r.1) && y < self.px(r.1 + r.3)
    }

    pub fn target_hit(&self, x: i32, y: i32) -> bool {
        self.in_rect(TARGET_HIT, x, y)
    }

    pub fn run_prev_hit(&self, x: i32, y: i32) -> bool {
        self.in_rect(RUN_PREV_HIT, x, y)
    }

    pub fn run_next_hit(&self, x: i32, y: i32) -> bool {
        self.in_rect(RUN_NEXT_HIT, x, y)
    }

    pub fn fight_prev_hit(&self, x: i32, y: i32) -> bool {
        self.in_rect(FIGHT_PREV_HIT, x, y)
    }

    pub fn fight_next_hit(&self, x: i32, y: i32) -> bool {
        self.in_rect(FIGHT_NEXT_HIT, x, y)
    }

    pub fn phase_hit(&self, x: i32, y: i32) -> bool {
        self.in_rect(PHASE_HIT, x, y)
    }

    pub fn rgba(&self, out: &mut Vec<u8>) {
        let n = (self.width * self.height * 4) as usize;
        out.resize(n, 0);
        let src = unsafe { std::slice::from_raw_parts(self.bits, n) };
        let (dst, _) = out.as_chunks_mut::<4>();
        let (pix, _) = src.as_chunks::<4>();
        for (d, s) in dst.iter_mut().zip(pix) {
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

fn fmt_deaths(n: u32) -> String {
    format!("{n} death{}", if n == 1 { "" } else { "s" })
}

fn fmt_dur(secs: u64) -> String {
    if secs >= 60 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

fn group_digits(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_btn_inside_hit() {
        let (hx, hy, hw, hh) = CLOSE_HIT;
        let (bx, by, bw, bh) = CLOSE_BTN;
        assert_eq!(hx + hw, LOGICAL_W);
        assert_eq!(hy, 0);
        assert!(bx >= hx && by >= hy);
        assert!(bx + bw <= hx + hw && by + bh <= hy + hh);
        assert!(by >= 8);
        assert!(LOGICAL_W - (bx + bw) >= 8);
    }

    #[test]
    fn easing() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        let mut prev = 0.0;
        for i in 1..=10 {
            let v = ease_out_cubic(i as f32 / 10.0);
            assert!(v >= prev);
            prev = v;
        }
        assert_eq!(lerp(2.0, 6.0, 0.0), 2.0);
        assert_eq!(lerp(2.0, 6.0, 1.0), 6.0);
    }

    #[test]
    fn target_hit_geometry() {
        let (tx, ty, tw, th) = TARGET_HIT;
        assert!(tx >= 14 && tx + tw <= LOGICAL_W - 14);
        assert!(ty >= 82 && ty + th <= 178);
        assert!(ty > CLOSE_HIT.1 + CLOSE_HIT.3);
        let r = Renderer::new(96);
        assert!(r.target_hit(tx, ty));
        assert!(r.target_hit(tx + tw - 1, ty + th - 1));
        assert!(!r.target_hit(tx - 1, ty));
        assert!(!r.target_hit(tx, ty + th));
        assert!(!r.target_hit(tx + tw, ty));
    }

    #[test]
    fn mix_endpoints() {
        assert_eq!(mix(ACCENT, TEXT, 0.0), ACCENT);
        assert_eq!(mix(ACCENT, TEXT, 1.0), TEXT);
    }

    #[test]
    fn draw_smoke_bars() {
        const P: &str = "2026.09.07 09:12:28 Debug      -  ";
        let mut gs = GameState::default();
        gs.feed(&format!(
            "{P}ECLIPTICA - now in stage: Stage_Test on phase: 0.5 as class: Blade"
        ));
        gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.5"
        ));
        gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: KakarotPhase2(Clone) on phase: 0.5"
        ));
        gs.feed(&format!("{P}Local controller dead, switching off."));
        let mut r = Renderer::new(96);
        let f = Frame {
            now: 0,
            flash_t: 1.0,
            hover_close: true,
            pressed_close: false,
            vr: VrStatus::Off,
            log_ok: true,
            progress_shown: 0.5,
            dps_frac_shown: 0.5,
            update: Badge::None,
            sound_on: true,
            view_run: Some(0),
            view_group: Some(0),
            view_phase: None,
            run_sel: false,
            group_sel: false,
        };
        r.draw(&mut gs, &f);
        let pix = |r: &Renderer, x: i32, y: i32| -> u32 {
            let s =
                unsafe { std::slice::from_raw_parts(r.bits, (r.width * r.height * 4) as usize) };
            let i = ((y * r.width + x) * 4) as usize;
            rgb(s[i + 2] as u32, s[i + 1] as u32, s[i] as u32)
        };
        assert_eq!(pix(&r, 0, 0), BG);
        assert_eq!(pix(&r, 100, 49), GOOD);
        assert_eq!(pix(&r, 300, 49), CARD);
        assert_eq!(pix(&r, 100, 234), ACCENT);
        assert_eq!(pix(&r, 20, 90), CARD);
        assert_eq!(pix(&r, 336, 11), CARD);

        let hist = Frame {
            run_sel: true,
            group_sel: true,
            view_phase: Some(0),
            ..f
        };
        r.draw(&mut gs, &hist);
    }

    #[test]
    fn phase_hit_geometry() {
        let (x, y, w, h) = PHASE_HIT;
        assert!(x >= 14 && x + w <= 346);
        assert!(y >= 82 && y + h <= 178);
        let r = Renderer::new(96);
        assert!(r.phase_hit(x, y));
        assert!(!r.phase_hit(x - 1, y));
        assert!(!r.phase_hit(x, y + h));
    }
}
