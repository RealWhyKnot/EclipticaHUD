use crate::hud::timeline::Filter;

pub(crate) const LOGICAL_W: i32 = 360;
pub(crate) const LOGICAL_H: i32 = 640;
pub(crate) const GRAPH_DY: i32 = 40;
pub(crate) const BODY_DY: i32 = 6;
pub(crate) const TAKEN_CARD_Y: i32 = 286;
pub(crate) const TAKEN_CARD_H: i32 = 270;
pub(crate) const GRAPH_RECT: (i32, i32, i32, i32) = (26, 240, 308, 40);
pub(crate) const LOG_W: i32 = 420;
pub(crate) const TIP_INSET: i32 = 8;
pub(crate) const TIP_PAD: i32 = 16;
pub(crate) const TIP_LINE_H: i32 = 16;
pub(crate) const LOG_H: i32 = 560;
pub(crate) const M: i32 = 14;
pub(crate) const W: i32 = LOGICAL_W - 2 * M;

pub(crate) const CLOSE_HIT: (i32, i32, i32, i32) = (324, 0, 36, 36);
pub(crate) const CLOSE_BTN: (i32, i32, i32, i32) = (328, 8, 24, 24);
pub(crate) const LOG_HIT: (i32, i32, i32, i32) = (292, 0, 32, 36);
pub(crate) const LOG_BTN: (i32, i32, i32, i32) = (296, 8, 24, 24);
pub(crate) const PIN_HIT: (i32, i32, i32, i32) = (260, 0, 32, 36);
pub(crate) const PIN_BTN: (i32, i32, i32, i32) = (264, 8, 24, 24);
pub(crate) const SETTINGS_HIT: (i32, i32, i32, i32) = (228, 0, 32, 36);
pub(crate) const SETTINGS_BTN: (i32, i32, i32, i32) = (232, 8, 24, 24);
pub(crate) const PARTY_HIT: (i32, i32, i32, i32) = (196, 0, 32, 36);
pub(crate) const PARTY_BTN: (i32, i32, i32, i32) = (200, 8, 24, 24);
pub(crate) const SETTINGS_CARD: (i32, i32, i32, i32) = (M, 82, W, 308);
pub(crate) const SCALE_DOWN_HIT: (i32, i32, i32, i32) = (194, 98, 28, 24);
pub(crate) const SCALE_UP_HIT: (i32, i32, i32, i32) = (306, 98, 28, 24);
pub(crate) const ALPHA_DOWN_HIT: (i32, i32, i32, i32) = (194, 134, 28, 24);
pub(crate) const ALPHA_UP_HIT: (i32, i32, i32, i32) = (306, 134, 28, 24);
pub(crate) const WINDOW_DOWN_HIT: (i32, i32, i32, i32) = (194, 170, 28, 24);
pub(crate) const WINDOW_UP_HIT: (i32, i32, i32, i32) = (306, 170, 28, 24);
pub(crate) const SOUND_HIT: (i32, i32, i32, i32) = (226, 206, 80, 24);
pub(crate) const DISCORD_HIT: (i32, i32, i32, i32) = (226, 242, 80, 24);
pub(crate) const USAGE_HIT: (i32, i32, i32, i32) = (226, 278, 80, 24);
pub(crate) const BOARD_HIT: (i32, i32, i32, i32) = (226, 314, 80, 24);
pub(crate) const BOARD_DELETE_HIT: (i32, i32, i32, i32) = (226, 350, 80, 24);
pub(crate) const SETTING_VALUE: (i32, i32) = (226, 80);
pub(crate) const UPDATE_HIT: (i32, i32) = (230, LOGICAL_H - 32);
pub(crate) const PROMPT_AREA: (i32, i32) = (54 + BODY_DY, LOGICAL_H - 88 - BODY_DY);
pub(crate) const PROMPT_T: i32 = 250;
pub(crate) const UPDATE_NOW_HIT: (i32, i32, i32, i32) = (50, PROMPT_T + 100, 168, 36);
pub(crate) const UPDATE_LATER_HIT: (i32, i32, i32, i32) = (222, PROMPT_T + 100, 88, 36);
pub(crate) const UPDATE_SKIP_HIT: (i32, i32, i32, i32) = (120, PROMPT_T + 142, 120, 20);
pub(crate) const ASK_T: i32 = 240;
pub(crate) const USAGE_YES_HIT: (i32, i32, i32, i32) = (50, ASK_T + 120, 168, 36);
pub(crate) const USAGE_NO_HIT: (i32, i32, i32, i32) = (222, ASK_T + 120, 88, 36);
pub(crate) const ASK_LINES: [&str; 3] = [
    "Help improve the HUD by sending boss",
    "and damage data from your log. Player",
    "names and IDs are taken out first.",
];
pub(crate) const DISCORD_DOT: (i32, i32, i32, i32) = (136, LOGICAL_H - 30, 72, 24);
pub(crate) const MUTE_HIT: (i32, i32, i32, i32) = (306, 108, 28, 22);
pub(crate) const RUN_PREV_HIT: (i32, i32, i32, i32) = (14, 54, 26, 24);
pub(crate) const RUN_NEXT_HIT: (i32, i32, i32, i32) = (320, 54, 26, 24);
pub(crate) const FIGHT_PREV_HIT: (i32, i32, i32, i32) = (240, 86, 26, 20);
pub(crate) const FIGHT_NEXT_HIT: (i32, i32, i32, i32) = (308, 86, 26, 20);
pub(crate) const STAGES_BTN: (i32, i32, i32, i32) = (308, 191, 26, 18);
pub(crate) const SPLITS_CARD: (i32, i32, i32, i32) =
    (M, 186, W, TAKEN_CARD_Y + GRAPH_DY + TAKEN_CARD_H - 186);
pub(crate) const SPLIT_ROWS: usize = 14;
pub(crate) const SPLIT_ROW_H: i32 = 22;
pub(crate) const MIN_SCALE: f32 = 0.5;
pub(crate) const MAX_SCALE: f32 = 2.0;
pub(crate) const MIN_ALPHA: u8 = 30;
pub(crate) const MAX_ALPHA: u8 = 100;
pub(crate) const MIN_WINDOW: u64 = 3;
pub(crate) const MAX_WINDOW: u64 = 30;

pub(crate) const LOG_CLOSE_HIT: (i32, i32, i32, i32) = (380, 0, 40, 36);
pub(crate) const LOG_CLOSE_BTN: (i32, i32, i32, i32) = (388, 8, 24, 24);
pub(crate) const LOG_TABS: [(Filter, &str, i32, i32); 3] = [
    (Filter::All, "All", 214, 30),
    (Filter::Damage, "Damage", 248, 62),
    (Filter::Targets, "Targets", 314, 58),
];
pub(crate) const LOG_TAB_Y: i32 = 8;
pub(crate) const LOG_TAB_H: i32 = 24;
pub(crate) const LOG_BODY: (i32, i32, i32, i32) = (14, 44, LOG_W - 40, LOG_H - 54);
pub(crate) const LOG_TRACK: (i32, i32, i32, i32) = (LOG_W - 20, 44, 6, LOG_H - 54);

pub(crate) const LOG_TITLE: (i32, i32, i32, i32) = (14, LOG_TAB_Y, 74, LOG_TAB_H);
pub(crate) const LOG_COUNT: (i32, i32, i32, i32) = (88, LOG_TAB_Y, 120, LOG_TAB_H);

pub(crate) const PARTY_W: i32 = 360;
pub(crate) const PARTY_H: i32 = 420;
pub(crate) const PARTY_CLOSE_HIT: (i32, i32, i32, i32) = (324, 0, 36, 36);
pub(crate) const PARTY_CLOSE_BTN: (i32, i32, i32, i32) = (328, 8, 24, 24);
pub(crate) const PARTY_TOGGLE: (i32, i32, i32, i32) = (264, 8, 56, 24);
pub(crate) const PARTY_CARD1: (i32, i32, i32, i32) = (M, 44, W, 72);
pub(crate) const PARTY_CARD2: (i32, i32, i32, i32) = (M, 124, W, 254);
pub(crate) const PARTY_ROW_Y: i32 = 154;
pub(crate) const PARTY_ROW_H: i32 = 22;
pub(crate) const PARTY_ROWS: usize = 10;
pub(crate) const PARTY_RANK: (i32, i32) = (26, 22);
pub(crate) const PARTY_NAME: (i32, i32) = (54, 108);
pub(crate) const PARTY_BAR: (i32, i32) = (168, 38);
pub(crate) const PARTY_DMG: (i32, i32) = (212, 64);
pub(crate) const PARTY_TAKEN: (i32, i32) = (282, 52);
const PAGE_W: i32 = 72;
pub(crate) const PARTY_OLDER: (i32, i32, i32, i32) = ((PARTY_W - 52 - PAGE_W) / 2, 386, 26, 20);
pub(crate) const PARTY_PAGE: (i32, i32, i32, i32) = (PARTY_OLDER.0 + 26, PARTY_OLDER.1, PAGE_W, 20);
pub(crate) const PARTY_NEWER: (i32, i32, i32, i32) =
    (PARTY_OLDER.0 + 26 + PAGE_W, PARTY_OLDER.1, 26, 20);
pub(crate) const ATTACK_BAR: (i32, i32) = (194, 64);
pub(crate) const ATTACK_DMG: (i32, i32) = (266, 68);
