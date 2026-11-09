#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Flag {
    Sound,
    Topmost,
    Discord,
    Usage,
    UsageFailing,
    UsageAsk,
    Board,
    BoardDeleting,
    Party,
    PartyOpen,
    VrcRunning,
    SettingsOpen,
    Stages,
    StagesButton,
    Primed,
    WasDead,
    PromptOpen,
    Tracking,
    LogOk,
    LogOpen,
    RunSelected,
    GroupSelected,
    Live,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Flags(u32);

impl Flags {
    const fn bit(flag: Flag) -> u32 {
        match flag {
            Flag::Sound => 1,
            Flag::Topmost => 1 << 1,
            Flag::Discord => 1 << 2,
            Flag::Usage => 1 << 3,
            Flag::UsageFailing => 1 << 4,
            Flag::UsageAsk => 1 << 5,
            Flag::Board => 1 << 6,
            Flag::BoardDeleting => 1 << 7,
            Flag::Party => 1 << 8,
            Flag::PartyOpen => 1 << 9,
            Flag::VrcRunning => 1 << 10,
            Flag::SettingsOpen => 1 << 11,
            Flag::Stages => 1 << 12,
            Flag::StagesButton => 1 << 13,
            Flag::Primed => 1 << 14,
            Flag::WasDead => 1 << 15,
            Flag::PromptOpen => 1 << 16,
            Flag::Tracking => 1 << 17,
            Flag::LogOk => 1 << 18,
            Flag::LogOpen => 1 << 19,
            Flag::RunSelected => 1 << 20,
            Flag::GroupSelected => 1 << 21,
            Flag::Live => 1 << 22,
        }
    }

    pub(crate) const fn has(self, flag: Flag) -> bool {
        self.0 & Self::bit(flag) != 0
    }

    #[must_use]
    pub(crate) const fn with(self, flag: Flag, on: bool) -> Flags {
        if on {
            Flags(self.0 | Self::bit(flag))
        } else {
            Flags(self.0 & !Self::bit(flag))
        }
    }

    pub(crate) const fn set(&mut self, flag: Flag, on: bool) {
        *self = self.with(flag, on);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_set_has_nothing_on() {
        assert!(!Flags::default().has(Flag::Sound));
        assert!(!Flags::default().has(Flag::Live));
    }

    #[test]
    fn each_flag_turns_on_and_off_alone() {
        let mut flags = Flags::default().with(Flag::Sound, true);
        flags.set(Flag::Live, true);
        assert!(flags.has(Flag::Sound));
        assert!(flags.has(Flag::Live));
        assert!(!flags.has(Flag::Topmost));
        flags.set(Flag::Sound, false);
        assert!(!flags.has(Flag::Sound));
        assert!(flags.has(Flag::Live));
    }

    #[test]
    fn setting_twice_changes_nothing() {
        let once = Flags::default().with(Flag::Board, true);
        assert_eq!(once.with(Flag::Board, true), once);
        assert_eq!(once.with(Flag::Board, false), Flags::default());
    }
}
