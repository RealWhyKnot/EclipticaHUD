use super::*;

pub(super) fn main_states() -> Vec<(&'static str, u8, Frame)> {
    let mut all = Vec::new();
    all.extend(main_states_1());
    all.extend(main_states_2());
    all.extend(main_states_3());
    all.extend(main_states_4());
    all.extend(main_states_5());
    all.extend(main_states_6());
    all
}

fn live() -> Frame {
    let now = tour_gs(false, 60).1;
    Frame {
        now,
        hover: None,
        view_run: Some(0),
        view_group: Some(1),
        group_pages: 2,
        flags: frame().flags.with(Flag::Discord, true),
        discord: Link::Connected,
        vr: crate::vr::VrStatus::On,
        ..frame()
    }
}

fn main_states_1() -> Vec<(&'static str, u8, Frame)> {
    let now_you = tour_gs(true, 60).1;
    vec![
        ("main_fight", 0, live()),
        ("main_dead_revivable", 4, live()),
        ("main_dead_for_good", 5, live()),
        ("main_last_life", 6, live()),
        (
            "main_stage",
            9,
            Frame {
                now: crate::game::event::split_line(&line(300, "x")).unwrap().ts,
                view_group: None,
                group_pages: 4,
                ..live()
            },
        ),
        (
            "main_live_stages_btn",
            9,
            Frame {
                now: crate::game::event::split_line(&line(300, "x")).unwrap().ts,
                view_group: None,
                group_pages: 4,
                flags: live().flags.with(Flag::StagesButton, true),
                ..live()
            },
        ),
        (
            "main_live_stages",
            9,
            Frame {
                now: crate::game::event::split_line(&line(300, "x")).unwrap().ts,
                view_group: None,
                group_pages: 4,
                flags: live()
                    .flags
                    .with(Flag::StagesButton, true)
                    .with(Flag::Stages, true),

                hover: Some(Hit::Stages),
                ..live()
            },
        ),
        (
            "main_intermission",
            3,
            Frame {
                now: inter_gs().1,
                ..frame()
            },
        ),
        (
            "main_fight_you_muted",
            1,
            Frame {
                now: now_you,
                flags: live().flags.with(Flag::Sound, false),
                ..live()
            },
        ),
        (
            "main_settings",
            0,
            Frame {
                flags: live().flags.with(Flag::SettingsOpen, true),
                ..live()
            },
        ),
        (
            "main_hover_settings",
            0,
            Frame {
                hover: Some(Hit::Settings),
                ..live()
            },
        ),
    ]
}

fn main_states_2() -> Vec<(&'static str, u8, Frame)> {
    let now_you = tour_gs(true, 60).1;
    vec![
        (
            "main_party_open",
            0,
            Frame {
                flags: live()
                    .flags
                    .with(Flag::PartyOpen, true)
                    .with(Flag::LogOpen, true),

                ..live()
            },
        ),
        (
            "main_settings_hover_sound",
            1,
            Frame {
                now: now_you,
                flags: live()
                    .flags
                    .with(Flag::SettingsOpen, true)
                    .with(Flag::Sound, false),

                hover: Some(Hit::Sound),
                ..live()
            },
        ),
        (
            "main_settings_hover_usage",
            0,
            Frame {
                flags: live().flags.with(Flag::SettingsOpen, true),
                hover: Some(Hit::Usage),
                tip: Some(Hit::Usage),
                ..live()
            },
        ),
        (
            "main_settings_usage_sending",
            0,
            Frame {
                flags: live()
                    .flags
                    .with(Flag::SettingsOpen, true)
                    .with(Flag::Usage, true),

                hover: Some(Hit::Usage),
                tip: Some(Hit::Usage),
                ..live()
            },
        ),
        (
            "main_settings_usage_failing",
            0,
            Frame {
                flags: live()
                    .flags
                    .with(Flag::SettingsOpen, true)
                    .with(Flag::Usage, true)
                    .with(Flag::UsageFailing, true),

                hover: Some(Hit::Usage),
                tip: Some(Hit::Usage),
                ..live()
            },
        ),
        (
            "main_settings_board_off",
            0,
            Frame {
                flags: live().flags.with(Flag::SettingsOpen, true),
                hover: Some(Hit::Board),
                tip: Some(Hit::Board),
                ..live()
            },
        ),
    ]
}

fn main_states_3() -> Vec<(&'static str, u8, Frame)> {
    vec![
        (
            "main_settings_board_on",
            0,
            Frame {
                flags: live()
                    .flags
                    .with(Flag::SettingsOpen, true)
                    .with(Flag::Board, true),

                hover: Some(Hit::Board),
                tip: Some(Hit::Board),
                ..live()
            },
        ),
        (
            "main_settings_board_failing",
            0,
            Frame {
                flags: live()
                    .flags
                    .with(Flag::SettingsOpen, true)
                    .with(Flag::Board, true),

                board_status: crate::board::FAILING,
                hover: Some(Hit::Board),
                tip: Some(Hit::Board),
                ..live()
            },
        ),
        (
            "main_settings_board_taken",
            0,
            Frame {
                flags: live()
                    .flags
                    .with(Flag::SettingsOpen, true)
                    .with(Flag::Board, true),

                board_status: crate::board::TAKEN,
                hover: Some(Hit::Board),
                tip: Some(Hit::Board),
                ..live()
            },
        ),
        (
            "main_settings_delete",
            0,
            Frame {
                flags: live()
                    .flags
                    .with(Flag::SettingsOpen, true)
                    .with(Flag::Board, true),

                hover: Some(Hit::BoardDelete),
                tip: Some(Hit::BoardDelete),
                ..live()
            },
        ),
        (
            "main_settings_delete_armed",
            0,
            Frame {
                flags: live()
                    .flags
                    .with(Flag::SettingsOpen, true)
                    .with(Flag::Board, true),

                board_delete: DELETE_ARMED,
                hover: Some(Hit::BoardDelete),
                tip: Some(Hit::BoardDelete),
                ..live()
            },
        ),
    ]
}

fn main_states_4() -> Vec<(&'static str, u8, Frame)> {
    vec![
        (
            "main_settings_delete_busy",
            0,
            Frame {
                flags: live().flags.with(Flag::SettingsOpen, true),
                board_delete: DELETE_BUSY,
                ..live()
            },
        ),
        (
            "main_settings_scale_max",
            0,
            Frame {
                flags: live().flags.with(Flag::SettingsOpen, true),
                scale: MAX_SCALE,
                ..live()
            },
        ),
        (
            "main_settings_scale_capped",
            0,
            Frame {
                flags: live().flags.with(Flag::SettingsOpen, true),
                scale: 1.6,
                scale_max: 1.6,
                ..live()
            },
        ),
        (
            "main_history",
            0,
            Frame {
                flags: live()
                    .flags
                    .with(Flag::RunSelected, true)
                    .with(Flag::GroupSelected, true),

                view_group: Some(0),
                pages: 2,
                view_page: 0,
                ..live()
            },
        ),
        (
            "main_history_first",
            7,
            Frame {
                flags: live()
                    .flags
                    .with(Flag::RunSelected, true)
                    .with(Flag::GroupSelected, true),

                view_group: Some(0),
                group_pages: 4,
                pages: 1,
                view_page: 0,
                ..live()
            },
        ),
        (
            "main_history_long",
            8,
            Frame {
                flags: live()
                    .flags
                    .with(Flag::RunSelected, true)
                    .with(Flag::GroupSelected, true),

                view_group: Some(17),
                group_pages: 30,
                pages: 1,
                view_page: 0,
                ..live()
            },
        ),
    ]
}

fn main_states_5() -> Vec<(&'static str, u8, Frame)> {
    vec![
        (
            "main_history_stages_btn",
            8,
            Frame {
                flags: live()
                    .flags
                    .with(Flag::RunSelected, true)
                    .with(Flag::GroupSelected, true)
                    .with(Flag::StagesButton, true),

                view_group: Some(17),
                group_pages: 30,
                pages: 1,
                view_page: 0,

                ..live()
            },
        ),
        (
            "main_stages",
            8,
            Frame {
                flags: live()
                    .flags
                    .with(Flag::RunSelected, true)
                    .with(Flag::GroupSelected, true)
                    .with(Flag::StagesButton, true)
                    .with(Flag::Stages, true),

                view_group: Some(17),
                group_pages: 30,
                pages: 1,
                view_page: 0,

                split_top: 1,
                ..live()
            },
        ),
        (
            "main_stages_short",
            7,
            Frame {
                flags: live()
                    .flags
                    .with(Flag::RunSelected, true)
                    .with(Flag::GroupSelected, true)
                    .with(Flag::StagesButton, true)
                    .with(Flag::Stages, true),

                view_group: Some(1),
                group_pages: 4,
                pages: 1,
                view_page: 0,

                hover: Some(Hit::Stages),
                ..live()
            },
        ),
        (
            "main_history_second",
            7,
            Frame {
                flags: live()
                    .flags
                    .with(Flag::RunSelected, true)
                    .with(Flag::GroupSelected, true),

                view_group: Some(1),
                group_pages: 4,
                pages: 1,
                view_page: 0,
                ..live()
            },
        ),
    ]
}

fn main_states_6() -> Vec<(&'static str, u8, Frame)> {
    vec![
        (
            "main_first_launch",
            2,
            Frame {
                env: Env::InWorld,
                view_run: None,
                view_group: None,
                group_pages: 0,
                flags: live().flags.with(Flag::Discord, false),
                discord: Link::Off,
                ..live()
            },
        ),
        (
            "main_prompt",
            2,
            Frame {
                env: Env::InWorld,
                view_run: None,
                view_group: None,
                group_pages: 0,
                update: Badge::Ready("v2026.10.29.0".to_string()),
                flags: live().flags.with(Flag::PromptOpen, true),
                ..live()
            },
        ),
        (
            "main_prompt_downloading",
            2,
            Frame {
                env: Env::InWorld,
                view_run: None,
                view_group: None,
                group_pages: 0,
                update: Badge::Installing("v2026.10.29.0".to_string()),
                flags: live().flags.with(Flag::PromptOpen, true),
                ..live()
            },
        ),
        (
            "main_downloading_hidden",
            2,
            Frame {
                env: Env::InWorld,
                view_run: None,
                view_group: None,
                group_pages: 0,
                update: Badge::Installing("v2026.10.29.0".to_string()),
                ..live()
            },
        ),
        (
            "main_ask_usage",
            2,
            Frame {
                env: Env::InWorld,
                view_run: None,
                view_group: None,
                group_pages: 0,
                flags: live().flags.with(Flag::UsageAsk, true),
                ..live()
            },
        ),
        (
            "main_ask_usage_hover",
            2,
            Frame {
                env: Env::InWorld,
                view_run: None,
                view_group: None,
                group_pages: 0,
                hover: Some(Hit::UsageYes),
                flags: live().flags.with(Flag::UsageAsk, true),
                ..live()
            },
        ),
    ]
}
