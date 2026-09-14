use crate::hud::timeline::Filter;

pub const LOGICAL_W: i32 = 360;
pub const LOGICAL_H: i32 = 600;
pub const LOG_W: i32 = 420;
pub const LOG_H: i32 = 560;

pub const CLOSE_HIT: (i32, i32, i32, i32) = (324, 0, 36, 36);
pub const CLOSE_BTN: (i32, i32, i32, i32) = (328, 8, 24, 24);
pub const LOG_HIT: (i32, i32, i32, i32) = (292, 0, 32, 36);
pub const LOG_BTN: (i32, i32, i32, i32) = (296, 8, 24, 24);
pub const PIN_HIT: (i32, i32, i32, i32) = (260, 0, 32, 36);
pub const PIN_BTN: (i32, i32, i32, i32) = (264, 8, 24, 24);
pub const SETTINGS_HIT: (i32, i32, i32, i32) = (228, 0, 32, 36);
pub const SETTINGS_BTN: (i32, i32, i32, i32) = (235, 8, 24, 24);
pub const SCALE_DOWN_HIT: (i32, i32, i32, i32) = (200, 90, 26, 24);
pub const SCALE_UP_HIT: (i32, i32, i32, i32) = (306, 90, 26, 24);
pub const ALPHA_DOWN_HIT: (i32, i32, i32, i32) = (200, 134, 26, 24);
pub const ALPHA_UP_HIT: (i32, i32, i32, i32) = (306, 134, 26, 24);
pub const WINDOW_DOWN_HIT: (i32, i32, i32, i32) = (200, 178, 26, 24);
pub const WINDOW_UP_HIT: (i32, i32, i32, i32) = (306, 178, 26, 24);
pub const UPDATE_HIT: (i32, i32) = (230, LOGICAL_H - 32);
pub const DISCORD_HIT: (i32, i32, i32, i32) = (136, LOGICAL_H - 30, 72, 24);
pub const TARGET_HIT: (i32, i32, i32, i32) = (26, 126, 308, 40);
pub const RUN_PREV_HIT: (i32, i32, i32, i32) = (14, 54, 26, 24);
pub const RUN_NEXT_HIT: (i32, i32, i32, i32) = (320, 54, 26, 24);
pub const FIGHT_PREV_HIT: (i32, i32, i32, i32) = (240, 86, 26, 20);
pub const FIGHT_NEXT_HIT: (i32, i32, i32, i32) = (308, 86, 26, 20);
pub const MIN_SCALE: f32 = 0.5;
pub const MAX_SCALE: f32 = 2.0;
pub const MIN_ALPHA: u8 = 30;
pub const MAX_ALPHA: u8 = 100;
pub const MIN_WINDOW: u64 = 3;
pub const MAX_WINDOW: u64 = 30;

pub const LOG_CLOSE_HIT: (i32, i32, i32, i32) = (380, 0, 40, 36);
pub const LOG_CLOSE_BTN: (i32, i32, i32, i32) = (388, 8, 24, 24);
pub const LOG_TABS: [(Filter, &str, i32, i32); 3] = [
    (Filter::All, "All", 166, 40),
    (Filter::Damage, "Damage", 210, 66),
    (Filter::Targets, "Targets", 280, 66),
];
pub const LOG_TAB_Y: i32 = 8;
pub const LOG_TAB_H: i32 = 24;
pub const LOG_BODY: (i32, i32, i32, i32) = (14, 44, LOG_W - 40, LOG_H - 54);
pub const LOG_TRACK: (i32, i32, i32, i32) = (LOG_W - 20, 44, 6, LOG_H - 54);

pub const LOG_TITLE: (i32, i32, i32, i32) = (14, LOG_TAB_Y, 74, LOG_TAB_H);
pub const LOG_COUNT: (i32, i32, i32, i32) = (88, LOG_TAB_Y, 76, LOG_TAB_H);
