mod breakdown;
mod fmt;
mod frame;
mod hit;
mod layout;
mod log_panel;
mod main_panel;
#[cfg(test)]
mod tests;
mod theme;

pub use frame::{Env, Frame, LogView};
pub use hit::{tip_ready, Hit, Info, LogHit};
pub use layout::*;
pub use theme::ease_out_cubic;

use theme::*;
use windows_sys::Win32::Foundation::RECT;
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

    fn glyph_button(
        &self,
        btn: (i32, i32, i32, i32),
        hover: bool,
        pressed: bool,
        lit: bool,
        glyph: &str,
    ) {
        let (bx, by, bw, bh) = btn;
        let color = if pressed {
            self.rround(bx, by, bw, bh, 6, ACCENT);
            BG
        } else if hover {
            self.rround(bx, by, bw, bh, 6, CARD_HI);
            TEXT
        } else if lit {
            ACCENT
        } else {
            DIM
        };
        self.text_rect(
            bx,
            by,
            bw,
            bh,
            F_GLYPH,
            color,
            DT_CENTER | DT_VCENTER,
            glyph,
        );
    }

    fn arrow(&self, hit: (i32, i32, i32, i32), on: bool, hover: bool, pressed: bool, glyph: &str) {
        let (x, y, w, h) = hit;
        let color = if !on {
            CARD
        } else if pressed {
            self.rround(x + 2, y + 2, w - 4, h - 4, 5, ACCENT);
            BG
        } else if hover {
            self.rround(x + 2, y + 2, w - 4, h - 4, 5, CARD_HI);
            TEXT
        } else {
            TEXT
        };
        self.text_rect(x, y, w, h, F_GLYPH, color, DT_CENTER | DT_VCENTER, glyph);
    }

    fn tooltip(&self, near: (i32, i32, i32, i32), text: &str, win_w: i32, win_h: i32) {
        let w = (text.chars().count() as i32 * 6 + 20).min(win_w - 16);
        let x = near.0.clamp(8, win_w - 8 - w);
        let below = near.1 + near.3 + 6;
        let y = if below + 24 <= win_h - 8 {
            below
        } else {
            near.1 - 30
        };
        self.rround(x, y, w, 24, 6, ACCENT);
        self.rround(x + 1, y + 1, w - 2, 22, 5, CARD_HI);
        self.text_rect(x, y, w, 24, F_TINY, TEXT, DT_CENTER | DT_VCENTER, text);
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
