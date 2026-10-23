use crate::hud::timeline::Filter;

pub const LOGICAL_W: i32 = 360;
pub const LOGICAL_H: i32 = 640;
pub const GRAPH_DY: i32 = 40;
pub const GRAPH_RECT: (i32, i32, i32, i32) = (26, 240, 308, 40);
pub const LOG_W: i32 = 420;
pub const TIP_INSET: i32 = 8;
pub const TIP_PAD: i32 = 16;
pub const TIP_LINE_H: i32 = 16;
pub const LOG_H: i32 = 560;
pub const M: i32 = 14;
pub const W: i32 = LOGICAL_W - 2 * M;

pub const CLOSE_HIT: (i32, i32, i32, i32) = (324, 0, 36, 36);
pub const CLOSE_BTN: (i32, i32, i32, i32) = (328, 8, 24, 24);
pub const LOG_HIT: (i32, i32, i32, i32) = (292, 0, 32, 36);
pub const LOG_BTN: (i32, i32, i32, i32) = (296, 8, 24, 24);
pub const PIN_HIT: (i32, i32, i32, i32) = (260, 0, 32, 36);
pub const PIN_BTN: (i32, i32, i32, i32) = (264, 8, 24, 24);
pub const SETTINGS_HIT: (i32, i32, i32, i32) = (228, 0, 32, 36);
pub const SETTINGS_BTN: (i32, i32, i32, i32) = (232, 8, 24, 24);
pub const PARTY_HIT: (i32, i32, i32, i32) = (196, 0, 32, 36);
pub const PARTY_BTN: (i32, i32, i32, i32) = (200, 8, 24, 24);
pub const SETTINGS_CARD: (i32, i32, i32, i32) = (M, 82, W, 236);
pub const SCALE_DOWN_HIT: (i32, i32, i32, i32) = (194, 96, 28, 24);
pub const SCALE_UP_HIT: (i32, i32, i32, i32) = (306, 96, 28, 24);
pub const ALPHA_DOWN_HIT: (i32, i32, i32, i32) = (194, 142, 28, 24);
pub const ALPHA_UP_HIT: (i32, i32, i32, i32) = (306, 142, 28, 24);
pub const WINDOW_DOWN_HIT: (i32, i32, i32, i32) = (194, 188, 28, 24);
pub const WINDOW_UP_HIT: (i32, i32, i32, i32) = (306, 188, 28, 24);
pub const SOUND_HIT: (i32, i32, i32, i32) = (226, 234, 80, 24);
pub const DISCORD_HIT: (i32, i32, i32, i32) = (226, 280, 80, 24);
pub const SETTING_VALUE: (i32, i32) = (226, 80);
pub const UPDATE_HIT: (i32, i32) = (230, LOGICAL_H - 32);
pub const PROMPT_AREA: (i32, i32) = (54, LOGICAL_H - 88);
pub const PROMPT_T: i32 = 250;
pub const UPDATE_NOW_HIT: (i32, i32, i32, i32) = (50, PROMPT_T + 100, 168, 36);
pub const UPDATE_LATER_HIT: (i32, i32, i32, i32) = (222, PROMPT_T + 100, 88, 36);
pub const UPDATE_SKIP_HIT: (i32, i32, i32, i32) = (120, PROMPT_T + 142, 120, 20);
pub const DISCORD_DOT: (i32, i32, i32, i32) = (136, LOGICAL_H - 30, 72, 24);
pub const MUTE_HIT: (i32, i32, i32, i32) = (306, 108, 28, 22);
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
    (Filter::All, "All", 214, 30),
    (Filter::Damage, "Damage", 248, 62),
    (Filter::Targets, "Targets", 314, 58),
];
pub const LOG_TAB_Y: i32 = 8;
pub const LOG_TAB_H: i32 = 24;
pub const LOG_BODY: (i32, i32, i32, i32) = (14, 44, LOG_W - 40, LOG_H - 54);
pub const LOG_TRACK: (i32, i32, i32, i32) = (LOG_W - 20, 44, 6, LOG_H - 54);

pub const LOG_TITLE: (i32, i32, i32, i32) = (14, LOG_TAB_Y, 74, LOG_TAB_H);
pub const LOG_COUNT: (i32, i32, i32, i32) = (88, LOG_TAB_Y, 120, LOG_TAB_H);

pub const PARTY_W: i32 = 360;
pub const PARTY_H: i32 = 392;
pub const PARTY_CLOSE_HIT: (i32, i32, i32, i32) = (324, 0, 36, 36);
pub const PARTY_CLOSE_BTN: (i32, i32, i32, i32) = (328, 8, 24, 24);
pub const PARTY_TOGGLE: (i32, i32, i32, i32) = (264, 8, 56, 24);
pub const PARTY_CARD1: (i32, i32, i32, i32) = (M, 44, W, 72);
pub const PARTY_CARD2: (i32, i32, i32, i32) = (M, 124, W, 254);
pub const PARTY_ROW_Y: i32 = 154;
pub const PARTY_ROW_H: i32 = 22;
pub const PARTY_ROWS: usize = 10;
pub const PARTY_RANK: (i32, i32) = (26, 22);
pub const PARTY_NAME: (i32, i32) = (54, 132);
pub const PARTY_BAR: (i32, i32) = (194, 64);
pub const PARTY_DMG: (i32, i32) = (266, 68);
