mod breakdown;
mod fmt;
mod frame;
mod graph;
mod hit;
mod layout;
mod log_panel;
mod main_panel;
mod party_panel;
#[cfg(test)]
mod party_tests;
mod prompt;
#[cfg(test)]
mod tests;
mod theme;
#[cfg(test)]
mod tour;

pub use frame::{Env, Frame, LogView, PartyFight, PartyPane, PartyView};
pub use hit::{tip_ready, Hit, Info, LogHit, PartyHit};
pub use layout::*;
pub use theme::ease_out_cubic;

use theme::*;
use windows_sys::Win32::Foundation::{RECT, SIZE};
use windows_sys::Win32::Graphics::Gdi::*;

pub struct Renderer {
    pub width: i32,
    pub height: i32,
    pub dc: HDC,
    bitmap: HBITMAP,
    pub bits: *mut u8,
    scale: f32,
    fonts: [HFONT; 6],
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Ctl {
    Off,
    Rest,
    Hover,
    Pressed,
    On,
    OnHover,
}

impl Ctl {
    pub(super) fn of(enabled: bool, on: bool, hover: bool, pressed: bool) -> Ctl {
        match (enabled, on, hover, pressed) {
            (false, _, _, _) => Ctl::Off,
            (true, _, _, true) => Ctl::Pressed,
            (true, true, true, false) => Ctl::OnHover,
            (true, true, false, false) => Ctl::On,
            (true, false, true, false) => Ctl::Hover,
            (true, false, false, false) => Ctl::Rest,
        }
    }
}

impl Renderer {
    pub fn new(dpi: u32, logical_w: i32, logical_h: i32) -> Self {
        let scale = dpi as f32 / 96.0;
        let width = (logical_w as f32 * scale) as i32;
        let height = (logical_h as f32 * scale) as i32;
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

    fn shifted(&self, dy: i32, draw: impl FnOnce()) {
        unsafe { SetViewportOrgEx(self.dc, 0, self.px(dy), std::ptr::null_mut()) };
        draw();
        unsafe { SetViewportOrgEx(self.dc, 0, 0, std::ptr::null_mut()) };
    }

    fn bar(&self, x: i32, y: i32, w: i32, h: i32, frac: f32, color: u32) {
        self.bar_on(x, y, w, h, frac, color, CARD);
    }

    #[allow(clippy::too_many_arguments)]
    fn bar_on(&self, x: i32, y: i32, w: i32, h: i32, frac: f32, color: u32, track: u32) {
        self.rround(x, y, w, h, h / 2, track);
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
        if s.is_empty() {
            return;
        }
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

    pub(super) fn text_w(&self, font: usize, s: &str) -> i32 {
        let buf: Vec<u16> = s.encode_utf16().collect();
        let mut size = SIZE { cx: 0, cy: 0 };
        unsafe {
            SelectObject(self.dc, self.fonts[font] as _);
            GetTextExtentPoint32W(self.dc, buf.as_ptr(), buf.len() as i32, &mut size);
        }
        size.cx
    }

    fn outline(&self, x: i32, y: i32, w: i32, h: i32, r: i32, color: u32) {
        unsafe {
            let pen = CreatePen(PS_INSIDEFRAME, self.px(1).max(1), color);
            let old_pen = SelectObject(self.dc, pen as _);
            let old_brush = SelectObject(self.dc, GetStockObject(NULL_BRUSH) as _);
            RoundRect(
                self.dc,
                self.px(x),
                self.px(y),
                self.px(x + w),
                self.px(y + h),
                self.px(r * 2),
                self.px(r * 2),
            );
            SelectObject(self.dc, old_brush);
            SelectObject(self.dc, old_pen);
            DeleteObject(pen as _);
        }
    }

    fn control(&self, rect: (i32, i32, i32, i32), ctl: Ctl) -> u32 {
        let (x, y, w, h) = rect;
        match ctl {
            Ctl::Off => INK_OFF,
            Ctl::Rest => {
                self.outline(x, y, w, h, 6, EDGE);
                TEXT
            }
            Ctl::Hover => {
                self.rround(x, y, w, h, 6, CTRL_HOVER);
                self.outline(x, y, w, h, 6, EDGE);
                TEXT
            }
            Ctl::Pressed | Ctl::On => {
                self.rround(x, y, w, h, 6, ACCENT);
                BG
            }
            Ctl::OnHover => {
                self.rround(x, y, w, h, 6, mix(ACCENT, TEXT, 0.2));
                BG
            }
        }
    }

    fn glyph_button(&self, btn: (i32, i32, i32, i32), ctl: Ctl, glyph: &str) {
        let ink = self.control(btn, ctl);
        let (x, y, w, h) = btn;
        self.text_rect(x, y, w, h, F_GLYPH, ink, DT_CENTER | DT_VCENTER, glyph);
    }

    fn text_button(&self, btn: (i32, i32, i32, i32), ctl: Ctl, label: &str) {
        let ink = self.control(btn, ctl);
        let (x, y, w, h) = btn;
        self.text_rect(x, y, w, h, F_LABEL, ink, DT_CENTER | DT_VCENTER, label);
    }

    #[allow(clippy::too_many_arguments)]
    fn link(&self, x: i32, y: i32, w: i32, h: i32, font: usize, color: u32, flags: u32, s: &str) {
        self.text_rect(x, y, w, h, font, color, flags | DT_VCENTER, s);
        let tw = self.unscale(self.text_w(font, s)).min(w);
        let lx = if flags & DT_RIGHT != 0 {
            x + w - tw
        } else if flags & DT_CENTER != 0 {
            x + (w - tw) / 2
        } else {
            x
        };
        let (cell, ascent) = self.font_metrics(font);
        let top = self.px(y) + (self.px(h) - cell) / 2;
        let base = top + ascent + self.px(1).max(1);
        let thick = self.px(1).max(1);
        unsafe {
            let brush = CreateSolidBrush(color);
            let r = RECT {
                left: self.px(lx),
                top: base,
                right: self.px(lx + tw),
                bottom: base + thick,
            };
            FillRect(self.dc, &r, brush);
            DeleteObject(brush as _);
        }
    }

    pub(super) fn font_metrics(&self, font: usize) -> (i32, i32) {
        unsafe {
            SelectObject(self.dc, self.fonts[font] as _);
            let mut tm: TEXTMETRICW = std::mem::zeroed();
            GetTextMetricsW(self.dc, &mut tm);
            (tm.tmHeight, tm.tmAscent)
        }
    }

    pub(super) fn tip_lines<'a>(&self, text: &'a str, win_w: i32) -> (i32, Vec<&'a str>) {
        let max = win_w - 16;
        let w = (text.chars().count() as i32 * 6 + 20).min(max);
        if self.text_w(F_TINY, text) + self.px(TIP_INSET) <= self.px(w) {
            return (w, vec![text]);
        }
        let widest = |a: &str, b: &str| self.text_w(F_TINY, a).max(self.text_w(F_TINY, b));
        let Some((a, b)) = text
            .match_indices(' ')
            .map(|(i, _)| (&text[..i], &text[i + 1..]))
            .min_by_key(|(a, b)| widest(a, b))
        else {
            return (w, vec![text]);
        };
        let w = (self.unscale(widest(a, b)) + 1 + TIP_PAD).min(max);
        (w, vec![a, b])
    }

    fn tooltip(&self, near: (i32, i32, i32, i32), text: &str, win_w: i32, win_h: i32) {
        let (w, lines) = self.tip_lines(text, win_w);
        let h = if lines.len() == 1 {
            24
        } else {
            8 + TIP_LINE_H * lines.len() as i32
        };
        let x = near.0.clamp(8, win_w - 8 - w);
        let below = near.1 + near.3 + 6;
        let y = if below + h <= win_h - 8 {
            below
        } else {
            near.1 - h - 6
        };
        self.rround(x, y, w, h, 6, ACCENT);
        self.rround(x + 1, y + 1, w - 2, h - 2, 5, CARD_HI);
        if let [line] = lines[..] {
            self.text_rect(x, y, w, 24, F_TINY, TEXT, DT_CENTER | DT_VCENTER, line);
            return;
        }
        for (i, line) in lines.iter().enumerate() {
            let ly = y + 4 + TIP_LINE_H * i as i32;
            let flags = DT_CENTER | DT_VCENTER;
            self.text_rect(x, ly, w, TIP_LINE_H, F_TINY, TEXT, flags, line);
        }
    }

    pub fn body_h(&self) -> i32 {
        LOG_BODY.3
    }

    pub fn unscale(&self, v: i32) -> i32 {
        (v as f32 / self.scale) as i32
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
