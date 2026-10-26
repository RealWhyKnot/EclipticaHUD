use super::frame::Frame;
use super::hit::Hit;
use super::layout::*;
use super::theme::*;
use super::Renderer;
use crate::update::{Badge, VERSION};
use windows_sys::Win32::Graphics::Gdi::{DT_CENTER, DT_VCENTER};

impl Renderer {
    pub(super) fn draw_update_prompt(&self, f: &Frame) {
        let tag = match &f.update {
            Badge::Ready(t) | Badge::Installing(t) | Badge::Failed(t) => t,
            Badge::None => return,
        };
        self.fill(0, PROMPT_AREA.0, LOGICAL_W, PROMPT_AREA.1, BG);
        let c = DT_CENTER | DT_VCENTER;
        let (bx, bw) = (M + 12, W - 24);
        self.text_rect(bx, PROMPT_T, bw, 32, F_BIG, TEXT, c, "Update available");
        let current = format!("Current version: {VERSION}");
        self.text_rect(bx, PROMPT_T + 44, bw, 20, F_BODY, DANGER, c, &current);
        let next = format!("New version: {}", tag.trim_start_matches(['v', 'V']));
        self.text_rect(bx, PROMPT_T + 64, bw, 20, F_BODY, GOOD, c, &next);
        let (ny, nh) = (UPDATE_NOW_HIT.1, UPDATE_NOW_HIT.3);
        let later = f.ctl(Hit::UpdateLater, true, false);
        let ink = self.control(UPDATE_LATER_HIT, later);
        let (lx, ly, lw, lh) = UPDATE_LATER_HIT;
        self.text_rect(lx, ly, lw, lh, F_BODY, ink, c, "Later");
        match f.update {
            Badge::Installing(_) => {
                let (nx, _, nw, _) = UPDATE_NOW_HIT;
                self.text_rect(nx, ny, nw, nh, F_BODY, DIM, c, "Downloading...");
            }
            _ => {
                self.choice(f, Hit::UpdateNow, UPDATE_NOW_HIT, GOOD, "Update");
                let (_, sy, _, sh) = UPDATE_SKIP_HIT;
                if matches!(f.update, Badge::Failed(_)) {
                    self.text_rect(bx, sy, bw, sh, F_TINY, DANGER, c, "Update failed");
                } else {
                    let (sx, _, sw, _) = UPDATE_SKIP_HIT;
                    let ink = if f.hov(Hit::UpdateSkip) { TEXT } else { DIM };
                    self.link(sx, sy, sw, sh, F_TINY, ink, DT_CENTER, "Skip this version");
                }
            }
        }
    }

    pub(super) fn draw_usage_prompt(&self, f: &Frame) {
        self.fill(0, PROMPT_AREA.0, LOGICAL_W, PROMPT_AREA.1, BG);
        let c = DT_CENTER | DT_VCENTER;
        let (bx, bw) = (M + 12, W - 24);
        self.text_rect(bx, ASK_T, bw, 32, F_BIG, TEXT, c, "Share fight data?");
        for (i, line) in ASK_LINES.iter().enumerate() {
            let y = ASK_T + 44 + 20 * i as i32;
            self.text_rect(bx, y, bw, 20, F_BODY, DIM, c, line);
        }
        self.choice(f, Hit::UsageYes, USAGE_YES_HIT, GOOD, "Share");
        let no = f.ctl(Hit::UsageNo, true, false);
        let ink = self.control(USAGE_NO_HIT, no);
        let (nx, ny, nw, nh) = USAGE_NO_HIT;
        self.text_rect(nx, ny, nw, nh, F_BODY, ink, c, "No thanks");
        let note_y = USAGE_YES_HIT.1 + USAGE_YES_HIT.3 + 10;
        let note = "You can change this in settings";
        self.text_rect(bx, note_y, bw, 20, F_TINY, DIM, c, note);
    }

    fn choice(&self, f: &Frame, hit: Hit, r: (i32, i32, i32, i32), tint: u32, s: &str) {
        let t = if f.prs(hit) {
            0.55
        } else if f.hov(hit) {
            0.4
        } else {
            0.25
        };
        self.rround(r.0, r.1, r.2, r.3, 8, mix(BG, tint, t));
        self.text_rect(r.0, r.1, r.2, r.3, F_BODY, TEXT, DT_CENTER | DT_VCENTER, s);
    }
}
