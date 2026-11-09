use super::super::Renderer;
use super::super::frame::Frame;
use super::super::hit::{DELETE_ARMED, DELETE_BUSY, Hit};
use super::super::layout::{
    ALPHA_DOWN_HIT, ALPHA_UP_HIT, BOARD_DELETE_HIT, BOARD_HIT, DISCORD_HIT, M, MAX_ALPHA,
    MAX_WINDOW, MIN_ALPHA, MIN_SCALE, MIN_WINDOW, SCALE_DOWN_HIT, SCALE_UP_HIT, SETTING_VALUE,
    SETTINGS_CARD, SOUND_HIT, USAGE_HIT, WINDOW_DOWN_HIT, WINDOW_UP_HIT,
};
use super::super::theme::{
    AMBER, BG, CARD, CARD_HI, DANGER, DIM, EDGE, F_BOSS, F_LABEL, GLYPH_MINUS, GLYPH_PLUS, INK_OFF,
    TEXT, mix,
};
use crate::hud::flags::Flag;
use windows_sys::Win32::Graphics::Gdi::{DT_CENTER, DT_LEFT, DT_VCENTER};

impl Renderer {
    pub(super) fn draw_settings(&self, f: &Frame) {
        let (cx, cy, cw, ch) = SETTINGS_CARD;
        self.rround(cx, cy, cw, ch, 8, CARD_HI);
        let steppers = [
            (
                "SIZE",
                format!("{}%", crate::num::to_i32((f.scale * 100.0).round())),
                (Hit::ScaleDown, SCALE_DOWN_HIT, f.scale > MIN_SCALE + 0.001),
                (Hit::ScaleUp, SCALE_UP_HIT, f.scale < f.scale_max - 0.001),
            ),
            (
                "OPACITY",
                format!("{}%", f.alpha),
                (Hit::AlphaDown, ALPHA_DOWN_HIT, f.alpha > MIN_ALPHA),
                (Hit::AlphaUp, ALPHA_UP_HIT, f.alpha < MAX_ALPHA),
            ),
            (
                "STAT WINDOW",
                format!("{} s", f.window),
                (Hit::WindowDown, WINDOW_DOWN_HIT, f.window > MIN_WINDOW),
                (Hit::WindowUp, WINDOW_UP_HIT, f.window < MAX_WINDOW),
            ),
        ];
        let (vx, vw) = SETTING_VALUE;
        for (label, value, down, up) in steppers {
            let y = down.1.1;
            self.setting_label(y, label);
            self.text_rect(
                (vx, y, vw, 24),
                F_BOSS,
                TEXT,
                DT_CENTER | DT_VCENTER,
                &value,
            );
            for (hit, rect, on) in [down, up] {
                let glyph = if hit == down.0 {
                    GLYPH_MINUS
                } else {
                    GLYPH_PLUS
                };
                self.glyph_button(
                    rect,
                    f.ctl(hit).usable(on && !f.flags.has(Flag::PromptOpen)),
                    glyph,
                );
            }
        }
        for (label, hit, rect, on) in [
            (
                "AGGRO SOUND",
                Hit::Sound,
                SOUND_HIT,
                f.flags.has(Flag::Sound),
            ),
            (
                "DISCORD STATUS",
                Hit::Discord,
                DISCORD_HIT,
                f.flags.has(Flag::Discord),
            ),
            (
                "SHARE FIGHT DATA",
                Hit::Usage,
                USAGE_HIT,
                f.flags.has(Flag::Usage),
            ),
            (
                "LEADERBOARD",
                Hit::Board,
                BOARD_HIT,
                f.flags.has(Flag::Board),
            ),
        ] {
            self.setting_label(rect.1, label);
            let word = if on { "On" } else { "Off" };
            self.text_button(
                rect,
                f.ctl(hit).lit(on).usable(!f.flags.has(Flag::PromptOpen)),
                word,
            );
        }
        if f.flags.has(Flag::UsageFailing) {
            self.dot(USAGE_HIT.0 - 16, USAGE_HIT.1 + 8, 8, AMBER);
        }
        if f.flags.has(Flag::Board) && f.board_status != crate::board::OK {
            let color = if f.board_status == crate::board::TAKEN {
                DANGER
            } else {
                AMBER
            };
            self.dot(BOARD_HIT.0 - 16, BOARD_HIT.1 + 8, 8, color);
        }
        self.setting_label(BOARD_DELETE_HIT.1, "BOARD KILLS");
        self.delete_button(f);
    }

    fn delete_button(&self, frame: &Frame) {
        let (x, y, width, height) = BOARD_DELETE_HIT;
        let hover = frame.hover == Some(Hit::BoardDelete) && !frame.flags.has(Flag::PromptOpen);
        let (word, ink) = match frame.board_delete {
            DELETE_BUSY => ("Deleting", INK_OFF),
            DELETE_ARMED => {
                let fill = if hover {
                    mix(DANGER, TEXT, 0.2)
                } else {
                    DANGER
                };
                self.rround(x, y, width, height, 6, fill);
                ("Sure?", BG)
            }
            _ if frame.flags.has(Flag::PromptOpen) => ("Delete", INK_OFF),
            _ => {
                if hover {
                    self.rround(x, y, width, height, 6, mix(CARD, DANGER, 0.3));
                }
                self.outline(x, y, width, height, 6, mix(EDGE, DANGER, 0.6));
                ("Delete", DANGER)
            }
        };
        self.text_rect(
            (x, y, width, height),
            F_LABEL,
            ink,
            DT_CENTER | DT_VCENTER,
            word,
        );
    }

    fn setting_label(&self, y: i32, label: &str) {
        self.text_rect(
            (M + 12, y, SCALE_DOWN_HIT.0 - M - 20, 24),
            F_LABEL,
            DIM,
            DT_LEFT | DT_VCENTER,
            label,
        );
    }
}
