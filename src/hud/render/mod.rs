mod breakdown;
mod enemies;
#[cfg(test)]
mod enemies_tests;
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

pub(crate) use frame::{Env, Frame, LogView, PartyFight, PartyPane, PartyView, RunSync};
pub(crate) use hit::{DELETE_ARMED, DELETE_BUSY, Hit, Info, LogHit, PartyHit, Pointer, tip_ready};
pub(crate) use layout::*;
pub(crate) use theme::ease_out_cubic;

use theme::{
    ACCENT, BG, CARD, CARD_HI, CTRL_HOVER, EDGE, F_GLYPH, F_LABEL, F_TINY, INK_OFF, TEXT, mix,
};
use windows_sys::Win32::Foundation::{RECT, SIZE};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS,
    CreateCompatibleDC, CreateDIBSection, CreateFontW, CreatePen, CreateSolidBrush,
    DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, DT_CENTER, DT_END_ELLIPSIS, DT_NOPREFIX,
    DT_RIGHT, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, FF_DONTCARE, FW_BOLD,
    FW_NORMAL, FW_SEMIBOLD, FillRect, GetStockObject, GetTextExtentPoint32W, GetTextMetricsW,
    HBITMAP, HDC, HFONT, NULL_BRUSH, NULL_PEN, OUT_DEFAULT_PRECIS, PS_INSIDEFRAME, RoundRect,
    SelectObject, SetBkMode, SetTextColor, SetViewportOrgEx, TEXTMETRICW, TRANSPARENT,
};

pub(crate) struct Renderer {
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
    pub(super) const fn rest(hover: bool) -> Ctl {
        if hover { Ctl::Hover } else { Ctl::Rest }
    }

    pub(super) const fn lit(self, on: bool) -> Ctl {
        match (self, on) {
            (Ctl::Rest, true) => Ctl::On,
            (Ctl::Hover, true) => Ctl::OnHover,
            _ => self,
        }
    }

    pub(super) const fn held(self, pressed: bool) -> Ctl {
        if pressed { Ctl::Pressed } else { self }
    }

    pub(super) const fn usable(self, enabled: bool) -> Ctl {
        if enabled { self } else { Ctl::Off }
    }
}

impl Renderer {
    pub(crate) fn new(dpi: u32, logical_w: i32, logical_h: i32) -> Self {
        let scale = crate::num::to_f32(dpi) / 96.0;
        let width = crate::num::to_i32(crate::num::to_f32(logical_w) * scale);
        let height = crate::num::to_i32(crate::num::to_f32(logical_h) * scale);
        // SAFETY: the DC, bitmap and fonts created here are owned by the returned Renderer and freed in Drop.
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = crate::num::to_u32(std::mem::size_of::<BITMAPINFOHEADER>());
            info.bmiHeader.biWidth = width;
            info.bmiHeader.biHeight = -height;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                dc,
                &raw const info,
                DIB_RGB_COLORS,
                &raw mut bits,
                std::ptr::null_mut(),
                0,
            );
            SelectObject(dc, bitmap.cast());
            SetBkMode(dc, crate::num::to_i32(TRANSPARENT));
            let font = |pt: i32, weight: i32, face: &str| {
                let name = wide(face);
                CreateFontW(
                    -crate::num::to_i32(crate::num::to_f32(pt) * scale),
                    0,
                    0,
                    0,
                    weight,
                    0,
                    0,
                    0,
                    u32::from(DEFAULT_CHARSET),
                    u32::from(OUT_DEFAULT_PRECIS),
                    u32::from(CLIP_DEFAULT_PRECIS),
                    u32::from(CLEARTYPE_QUALITY),
                    u32::from(DEFAULT_PITCH | FF_DONTCARE),
                    name.as_ptr(),
                )
            };
            let fonts = [
                font(11, FW_NORMAL.cast_signed(), "Segoe UI\0"),
                font(13, FW_NORMAL.cast_signed(), "Segoe UI\0"),
                font(11, FW_SEMIBOLD.cast_signed(), "Segoe UI\0"),
                font(16, FW_SEMIBOLD.cast_signed(), "Segoe UI\0"),
                font(26, FW_BOLD.cast_signed(), "Segoe UI\0"),
                font(10, FW_NORMAL.cast_signed(), "Segoe MDL2 Assets\0"),
            ];
            Renderer {
                width,
                height,
                dc,
                bitmap,
                bits: bits.cast::<u8>(),
                scale,
                fonts,
            }
        }
    }

    fn px(&self, v: i32) -> i32 {
        crate::num::to_i32(crate::num::to_f32(v) * self.scale)
    }

    fn fill(&self, x: i32, y: i32, width: i32, height: i32, color: u32) {
        // SAFETY: `self.dc` is this renderer's live DC, and the brush made here is deleted before returning.
        unsafe {
            let brush = CreateSolidBrush(color);
            let rect = RECT {
                left: self.px(x),
                top: self.px(y),
                right: self.px(x + width),
                bottom: self.px(y + height),
            };
            FillRect(self.dc, &raw const rect, brush);
            DeleteObject(brush.cast());
        }
    }

    fn rround(&self, x: i32, y: i32, width: i32, height: i32, radius: i32, color: u32) {
        // SAFETY: `self.dc` is this renderer's live DC, and the objects selected here are restored and deleted before returning.
        unsafe {
            let old_pen = SelectObject(self.dc, GetStockObject(NULL_PEN).cast());
            let brush = CreateSolidBrush(color);
            let old_brush = SelectObject(self.dc, brush.cast());
            RoundRect(
                self.dc,
                self.px(x),
                self.px(y),
                self.px(x + width) + 1,
                self.px(y + height) + 1,
                self.px(radius * 2),
                self.px(radius * 2),
            );
            SelectObject(self.dc, old_brush);
            SelectObject(self.dc, old_pen);
            DeleteObject(brush.cast());
        }
    }

    fn shifted(&self, dy: i32, draw: impl FnOnce()) {
        // SAFETY: `self.dc` is this renderer's live DC, and a null old-origin pointer is allowed.
        unsafe { SetViewportOrgEx(self.dc, 0, self.px(dy), std::ptr::null_mut()) };
        draw();
        // SAFETY: `self.dc` is this renderer's live DC, and a null old-origin pointer is allowed.
        unsafe { SetViewportOrgEx(self.dc, 0, 0, std::ptr::null_mut()) };
    }

    fn bar(&self, x: i32, y: i32, w: i32, h: i32, frac: f32, color: u32) {
        self.bar_on((x, y, w, h), frac, color, CARD);
    }

    fn bar_on(&self, (x, y, w, h): (i32, i32, i32, i32), frac: f32, color: u32, track: u32) {
        self.rround(x, y, w, h, h / 2, track);
        let fw = crate::num::to_i32(crate::num::to_f32(w) * frac.clamp(0.0, 1.0));
        if fw >= h {
            self.rround(x, y, fw, h, h / 2, color);
        }
    }

    fn dot(&self, x: i32, y: i32, d: i32, color: u32) {
        self.rround(x, y, d, d, d / 2, color);
    }

    fn text(&self, (x, y, w): (i32, i32, i32), font: usize, color: u32, flags: u32, s: &str) {
        self.text_rect((x, y, w, 40), font, color, flags, s);
    }

    fn text_rect(
        &self,
        (x, y, width, height): (i32, i32, i32, i32),
        font: usize,
        color: u32,
        flags: u32,
        text: &str,
    ) {
        if text.is_empty() {
            return;
        }
        // SAFETY: `self.dc` and the font are owned by this renderer, and the text buffer outlives the call.
        unsafe {
            SelectObject(self.dc, self.fonts[font].cast());
            SetTextColor(self.dc, color);
            let mut rect = RECT {
                left: self.px(x),
                top: self.px(y),
                right: self.px(x + width),
                bottom: self.px(y) + self.px(height),
            };
            let mut buf = wide(text);
            DrawTextW(
                self.dc,
                buf.as_mut_ptr(),
                crate::num::to_i32(buf.len()),
                &raw mut rect,
                flags | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
        }
    }

    pub(super) fn text_w(&self, font: usize, s: &str) -> i32 {
        let buf: Vec<u16> = s.encode_utf16().collect();
        let mut size = SIZE { cx: 0, cy: 0 };
        // SAFETY: `self.dc` and the font are owned by this renderer, and the pointer and length describe `buf`.
        unsafe {
            SelectObject(self.dc, self.fonts[font].cast());
            GetTextExtentPoint32W(
                self.dc,
                buf.as_ptr(),
                crate::num::to_i32(buf.len()),
                &raw mut size,
            );
        }
        size.cx
    }

    fn outline(&self, x: i32, y: i32, width: i32, height: i32, radius: i32, color: u32) {
        // SAFETY: `self.dc` is this renderer's live DC, and the pen made here is deselected and deleted before returning.
        unsafe {
            let pen = CreatePen(PS_INSIDEFRAME, self.px(1).max(1), color);
            let old_pen = SelectObject(self.dc, pen.cast());
            let old_brush = SelectObject(self.dc, GetStockObject(NULL_BRUSH).cast());
            RoundRect(
                self.dc,
                self.px(x),
                self.px(y),
                self.px(x + width),
                self.px(y + height),
                self.px(radius * 2),
                self.px(radius * 2),
            );
            SelectObject(self.dc, old_brush);
            SelectObject(self.dc, old_pen);
            DeleteObject(pen.cast());
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
        self.text_rect((x, y, w, h), F_GLYPH, ink, DT_CENTER | DT_VCENTER, glyph);
    }

    fn text_button(&self, btn: (i32, i32, i32, i32), ctl: Ctl, label: &str) {
        let ink = self.control(btn, ctl);
        let (x, y, w, h) = btn;
        self.text_rect((x, y, w, h), F_LABEL, ink, DT_CENTER | DT_VCENTER, label);
    }

    fn link(
        &self,
        (x, y, width, height): (i32, i32, i32, i32),
        font: usize,
        color: u32,
        flags: u32,
        label: &str,
    ) {
        self.text_rect(
            (x, y, width, height),
            font,
            color,
            flags | DT_VCENTER,
            label,
        );
        let tw = self.unscale(self.text_w(font, label)).min(width);
        let lx = if flags & DT_RIGHT != 0 {
            x + width - tw
        } else if flags & DT_CENTER != 0 {
            x + (width - tw) / 2
        } else {
            x
        };
        let (cell, ascent) = self.font_metrics(font);
        let top = self.px(y) + (self.px(height) - cell) / 2;
        let base = top + ascent + self.px(1).max(1);
        let thick = self.px(1).max(1);
        // SAFETY: `self.dc` is this renderer's live DC, and the brush made here is deleted before returning.
        unsafe {
            let brush = CreateSolidBrush(color);
            let rect = RECT {
                left: self.px(lx),
                top: base,
                right: self.px(lx + tw),
                bottom: base + thick,
            };
            FillRect(self.dc, &raw const rect, brush);
            DeleteObject(brush.cast());
        }
    }

    pub(super) fn font_metrics(&self, font: usize) -> (i32, i32) {
        // SAFETY: `self.dc` and the font are owned by this renderer, and `tm` is a live TEXTMETRICW.
        unsafe {
            SelectObject(self.dc, self.fonts[font].cast());
            let mut tm: TEXTMETRICW = std::mem::zeroed();
            GetTextMetricsW(self.dc, &raw mut tm);
            (tm.tmHeight, tm.tmAscent)
        }
    }

    pub(super) fn tip_lines<'a>(&self, text: &'a str, win_w: i32) -> (i32, Vec<&'a str>) {
        let max = win_w - 16;
        let w = (crate::num::to_i32(text.chars().count()) * 6 + 20).min(max);
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
            8 + TIP_LINE_H * crate::num::to_i32(lines.len())
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
            self.text_rect((x, y, w, 24), F_TINY, TEXT, DT_CENTER | DT_VCENTER, line);
            return;
        }
        for (i, line) in lines.iter().enumerate() {
            let ly = y + 4 + TIP_LINE_H * crate::num::to_i32(i);
            let flags = DT_CENTER | DT_VCENTER;
            self.text_rect((x, ly, w, TIP_LINE_H), F_TINY, TEXT, flags, line);
        }
    }

    pub(crate) fn body_h() -> i32 {
        LOG_BODY.3
    }

    pub(crate) fn unscale(&self, v: i32) -> i32 {
        crate::num::to_i32(crate::num::to_f32(v) / self.scale)
    }

    pub(crate) fn rgba(&self, out: &mut Vec<u8>) {
        let n = crate::num::to_usize(self.width * self.height * 4);
        out.resize(n, 0);
        // SAFETY: `bits` points at the DIB section's width * height * 4 bytes, which live as long as `self`.
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
        // SAFETY: the fonts, bitmap and DC were created in `new` and are freed exactly once here.
        unsafe {
            for f in self.fonts {
                DeleteObject(f.cast());
            }
            DeleteObject(self.bitmap.cast());
            DeleteDC(self.dc);
        }
    }
}
