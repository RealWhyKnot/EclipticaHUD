use super::layout::*;
use super::Renderer;
use crate::hud::timeline::Filter;

const UPDATE_RECT: (i32, i32, i32, i32) =
    (UPDATE_HIT.0, UPDATE_HIT.1, LOGICAL_W - UPDATE_HIT.0, 32);
const TIP_DELAY_MS: u128 = 450;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hit {
    Pin,
    Settings,
    ScaleDown,
    ScaleUp,
    AlphaDown,
    AlphaUp,
    WindowDown,
    WindowUp,
    Party,
    Discord,
    Usage,
    Log,
    Close,
    Update,
    UpdateNow,
    UpdateLater,
    UpdateSkip,
    Mute,
    Sound,
    RunPrev,
    RunNext,
    FightPrev,
    FightNext,
    Info(Info),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Info {
    Status,
    Progress,
    RunRow,
    Boss,
    Result,
    Dealt(u8),
    DealtGraph,
    LastKill,
    Taken(u8),
    TakenGraph,
    LastHit,
    Deaths,
    TakenMore(u8),
    TopAttacks,
    Target,
    Vr,
    LogDot,
    Discord,
    Version,
}

const B: i32 = BODY_DY;
const T: i32 = TAKEN_CARD_Y + GRAPH_DY + BODY_DY;

pub const INFO_REGIONS: [(Info, (i32, i32, i32, i32)); 25] = [
    (Info::Status, (14, 26 + B, 332, 20)),
    (Info::Progress, (14, 46 + B, 332, 8)),
    (Info::RunRow, (44, 54 + B, 272, 24)),
    (Info::Boss, (74, 84 + B, 160, 26)),
    (Info::Result, (26, 126 + B, 308, 40)),
    (Info::Target, (26, 126 + B, 308, 40)),
    (Info::Dealt(0), (26, 190 + B, 102, 40)),
    (Info::Dealt(1), (128, 190 + B, 102, 40)),
    (Info::Dealt(2), (230, 190 + B, 102, 40)),
    (Info::DealtGraph, (14, 232 + B, 332, 52)),
    (Info::LastKill, (14, 284 + B, 332, 30)),
    (Info::Taken(0), (26, T + 4, 102, 40)),
    (Info::Taken(1), (128, T + 4, 102, 40)),
    (Info::Taken(2), (230, T + 4, 102, 40)),
    (Info::TakenGraph, (14, T + 46, 332, 52)),
    (Info::LastHit, (14, T + 98, 332, 24)),
    (Info::Deaths, (14, T + 98, 332, 24)),
    (Info::TakenMore(0), (26, T + 124, 102, 38)),
    (Info::TakenMore(1), (128, T + 124, 102, 38)),
    (Info::TakenMore(2), (230, T + 124, 102, 38)),
    (Info::TopAttacks, (14, T + 166, 332, 92)),
    (Info::Vr, (14, LOGICAL_H - 30, 72, 24)),
    (Info::LogDot, (86, LOGICAL_H - 30, 50, 24)),
    (Info::Discord, DISCORD_DOT),
    (
        Info::Version,
        (
            UPDATE_HIT.0,
            UPDATE_HIT.1,
            LOGICAL_W - 14 - UPDATE_HIT.0,
            32,
        ),
    ),
];

impl Info {
    pub fn live_only(self) -> bool {
        matches!(self, Info::LastHit | Info::Target | Info::TakenMore(2))
    }

    pub fn history_only(self) -> bool {
        matches!(self, Info::Result | Info::Deaths)
    }

    pub(super) fn rect(self) -> (i32, i32, i32, i32) {
        INFO_REGIONS
            .iter()
            .find(|(i, _)| *i == self)
            .map_or((0, 0, 0, 0), |(_, r)| *r)
    }

    pub(super) fn tip(self, live: bool) -> &'static str {
        match (self, live) {
            (Info::Status, _) => "Stage, run phase and class, plus tokens picked up this stage",
            (Info::Progress, _) => "How far the run has progressed through this stage",
            (Info::RunRow, true) => "Run number and the stage counter of the live run",
            (Info::RunRow, false) => {
                "Run number, start time, stage and deaths; LOST means a boss survived"
            }
            (Info::Boss, true) => "The boss you are fighting, or the stage you are clearing",
            (Info::Boss, false) => "The boss of this fight, all of its phases together",
            (Info::Result, _) => {
                "killed, lost (run ended with it alive) or unfinished, plus fight time"
            }
            (Info::Dealt(0), true) => "Damage per second over the stat window from settings",
            (Info::Dealt(0), false) => "All damage you dealt in this fight, across phases",
            (Info::Dealt(1), true) => "Damage divided by the time in this fight, stage or run",
            (Info::Dealt(1), false) => "Your fight damage divided by the fight time",
            (Info::Dealt(2), true) => "All damage you dealt in this fight, stage or run",
            (Info::Dealt(2), false) => "How long the fight lasted",
            (Info::Dealt(_), _) => "",
            (Info::DealtGraph, _) => {
                "Your damage per second across the fight; lines mark new phases"
            }
            (Info::LastKill, true) => "What the game credited you with on your last boss kill",
            (Info::LastKill, false) => "What the game credited you with when this boss died",
            (Info::Taken(0), true) => {
                "Damage taken per second over the stat window, or across the run"
            }
            (Info::Taken(0), false) => "All damage you took in this fight or run",
            (Info::Taken(1), true) => {
                "Taken per second this fight or stage; between bosses, the run total"
            }
            (Info::Taken(1), false) => "Damage taken divided by the fight or run time",
            (Info::Taken(2), true) => {
                "All damage you took this fight or stage; between bosses, deaths"
            }
            (Info::Taken(2), false) => "How many times you were hit",
            (Info::Taken(_), _) => "",
            (Info::TakenGraph, _) => {
                "Damage you took per second across the fight; lines mark new phases"
            }
            (Info::LastHit, _) => "Newest hit on you, who it came from and how long ago",
            (Info::Deaths, _) => "How many times you died in this fight or run",
            (Info::TakenMore(0), true) => "How many times you were hit",
            (Info::TakenMore(0), false) => "The largest single hit on you",
            (Info::TakenMore(1), true) => "The largest single hit on you",
            (Info::TakenMore(1), false) => "Damage taken divided by the number of hits",
            (Info::TakenMore(2), _) => "Damage taken divided by the number of hits",
            (Info::TakenMore(_), _) => "",
            (Info::TopAttacks, true) => {
                "The attacks that hurt most and how often each hit; run deaths at right"
            }
            (Info::TopAttacks, false) => "The attacks that hurt most and how often each hit",
            (Info::Vr, _) => "SteamVR wrist overlay: green when attached, red when failing",
            (Info::LogDot, _) => "VRChat log: green while VRChat runs and its log is open",
            (Info::Target, _) => "Who the boss is after, and for how long",
            (Info::Discord, _) => "Discord status: green showing, amber waiting, red refused",
            (Info::Version, _) => "Running version; a new release shows here when available",
        }
    }
}

#[derive(Clone, Copy)]
pub struct TipCtx {
    pub live: bool,
    pub topmost: bool,
    pub sound_on: bool,
    pub log_open: bool,
    pub discord_on: bool,
    pub usage_on: bool,
    pub party_open: bool,
}

impl Hit {
    pub fn clickable(self) -> bool {
        !matches!(self, Hit::Info(_))
    }

    pub fn tip(self, c: TipCtx) -> &'static str {
        match self {
            Hit::Pin if c.topmost => "Kept above other windows; click to let them cover it",
            Hit::Pin => "Other windows can cover the HUD; click to keep it on top",
            Hit::Settings => "Size, opacity, stat window, aggro sound, Discord and data sharing",
            Hit::ScaleDown => "Make the HUD smaller",
            Hit::ScaleUp => "Make the HUD bigger",
            Hit::AlphaDown => "Make the HUD more see-through",
            Hit::AlphaUp => "Make the HUD less see-through",
            Hit::WindowDown => "Shorter window for the live DPS and damage taken",
            Hit::WindowUp => "Longer window for the live DPS and damage taken",
            Hit::Party if c.party_open => "Close the shared damage window",
            Hit::Party => "Open the shared damage window",
            Hit::Discord if c.discord_on => {
                "Showing this run as your Discord status; click to stop"
            }
            Hit::Discord => "Show your run as your Discord status, with a join link",
            Hit::Usage if c.usage_on => {
                "Sending boss and damage data, player names removed; click to stop"
            }
            Hit::Usage => "Send boss and damage data to improve the HUD; player names are removed",
            Hit::Log if c.log_open => "Close the event log window",
            Hit::Log => "Open the event log window",
            Hit::Close => "Close the HUD (Esc)",
            Hit::Update => "Show the update",
            Hit::UpdateNow => "Download the new version and restart the HUD",
            Hit::UpdateLater => "Hide this for now; the update label in the corner brings it back",
            Hit::UpdateSkip => "Don't offer this version again",
            Hit::Mute | Hit::Sound if c.sound_on => {
                "Mute the sound that plays when the boss targets you"
            }
            Hit::Mute | Hit::Sound => "Unmute the sound that plays when the boss targets you",
            Hit::RunPrev => "Show the previous run",
            Hit::RunNext => "Show the next run; past the newest returns to live",
            Hit::FightPrev => "Show the previous boss fight of this run",
            Hit::FightNext => "Show the next boss fight of this run",
            Hit::Info(i) => i.tip(c.live),
        }
    }

    pub(super) fn rect(self) -> (i32, i32, i32, i32) {
        match self {
            Hit::Pin => PIN_BTN,
            Hit::Settings => SETTINGS_BTN,
            Hit::ScaleDown => body(SCALE_DOWN_HIT),
            Hit::ScaleUp => body(SCALE_UP_HIT),
            Hit::AlphaDown => body(ALPHA_DOWN_HIT),
            Hit::AlphaUp => body(ALPHA_UP_HIT),
            Hit::WindowDown => body(WINDOW_DOWN_HIT),
            Hit::WindowUp => body(WINDOW_UP_HIT),
            Hit::Party => PARTY_BTN,
            Hit::Discord => body(DISCORD_HIT),
            Hit::Usage => body(USAGE_HIT),
            Hit::Sound => body(SOUND_HIT),
            Hit::Log => LOG_BTN,
            Hit::Close => CLOSE_BTN,
            Hit::Update => UPDATE_RECT,
            Hit::UpdateNow => UPDATE_NOW_HIT,
            Hit::UpdateLater => UPDATE_LATER_HIT,
            Hit::UpdateSkip => UPDATE_SKIP_HIT,
            Hit::Mute => body(MUTE_HIT),
            Hit::RunPrev => body(RUN_PREV_HIT),
            Hit::RunNext => body(RUN_NEXT_HIT),
            Hit::FightPrev => body(FIGHT_PREV_HIT),
            Hit::FightNext => body(FIGHT_NEXT_HIT),
            Hit::Info(i) => i.rect(),
        }
    }
}

fn body((x, y, w, h): (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    (x, y + BODY_DY, w, h)
}

pub fn tip_ready(since: Option<std::time::Instant>) -> bool {
    since.is_some_and(|t| t.elapsed().as_millis() >= TIP_DELAY_MS)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LogHit {
    Close,
    Tab(Filter),
    Thumb,
    Track,
    Title,
    Count,
}

impl LogHit {
    pub fn clickable(self) -> bool {
        !matches!(self, LogHit::Title | LogHit::Count)
    }

    pub fn tip(self) -> &'static str {
        match self {
            LogHit::Title => "Every hit you took and every aggro switch, newest first",
            LogHit::Count => "Events in the current filter; a grey line marks a boss fight start",
            LogHit::Close => "Close the log (Esc)",
            LogHit::Tab(Filter::All) => "Show hits and aggro switches",
            LogHit::Tab(Filter::Damage) => "Show only damage you took",
            LogHit::Tab(Filter::Targets) => "Show only aggro switches",
            LogHit::Thumb => "Drag to scroll, or use the wheel",
            LogHit::Track => "Click to jump a page",
        }
    }

    pub(super) fn rect(self, thumb: Option<(i32, i32)>) -> (i32, i32, i32, i32) {
        match self {
            LogHit::Close => LOG_CLOSE_BTN,
            LogHit::Tab(f) => LOG_TABS
                .iter()
                .find(|t| t.0 == f)
                .map_or(LOG_CLOSE_BTN, |t| (t.2, LOG_TAB_Y, t.3, LOG_TAB_H)),
            LogHit::Thumb => {
                let (x, y, w, _) = LOG_TRACK;
                let (ty, th) = thumb.unwrap_or((0, 24));
                (x, y + ty, w, th)
            }
            LogHit::Track => LOG_TRACK,
            LogHit::Title => LOG_TITLE,
            LogHit::Count => LOG_COUNT,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PartyHit {
    Close,
    Toggle,
    Older,
    Newer,
    Status,
    Rank,
    Board,
}

const PARTY_STATUS: (i32, i32, i32, i32) = (180, 44, 166, 24);
const PARTY_RANK_AREA: (i32, i32, i32, i32) = (14, 68, 332, 48);

impl PartyHit {
    pub fn clickable(self) -> bool {
        matches!(
            self,
            PartyHit::Close | PartyHit::Toggle | PartyHit::Older | PartyHit::Newer
        )
    }

    pub fn tip(self, on: bool) -> &'static str {
        match self {
            PartyHit::Close => "Close this window (Esc)",
            PartyHit::Toggle if on => "Stop sharing your boss damage",
            PartyHit::Toggle => "Share your boss damage with others here who turn this on too",
            PartyHit::Older => "Show an earlier shared fight",
            PartyHit::Newer => "Show a later shared fight",
            PartyHit::Status => "Connection to the shared damage server",
            PartyHit::Rank => "Your place among the players here who share, by damage this fight",
            PartyHit::Board => "Boss damage from everyone here who shares it, highest first",
        }
    }

    pub(super) fn rect(self) -> (i32, i32, i32, i32) {
        match self {
            PartyHit::Close => PARTY_CLOSE_BTN,
            PartyHit::Toggle => PARTY_TOGGLE,
            PartyHit::Older => PARTY_OLDER,
            PartyHit::Newer => PARTY_NEWER,
            PartyHit::Status => PARTY_STATUS,
            PartyHit::Rank => PARTY_RANK_AREA,
            PartyHit::Board => PARTY_CARD2,
        }
    }
}

impl Renderer {
    fn in_rect(&self, r: (i32, i32, i32, i32), x: i32, y: i32) -> bool {
        x >= self.px(r.0) && x < self.px(r.0 + r.2) && y >= self.px(r.1) && y < self.px(r.1 + r.3)
    }

    #[cfg(test)]
    pub fn hit_test(&self, x: i32, y: i32) -> Option<Hit> {
        self.hit_test_where(x, y, |_| true)
    }

    pub fn hit_test_where(&self, x: i32, y: i32, ok: impl Fn(Hit) -> bool) -> Option<Hit> {
        if y < self.px(CLOSE_HIT.1 + CLOSE_HIT.3) {
            if x >= self.px(CLOSE_HIT.0) && ok(Hit::Close) {
                return Some(Hit::Close);
            }
            if x >= self.px(LOG_HIT.0) && x < self.px(CLOSE_HIT.0) && ok(Hit::Log) {
                return Some(Hit::Log);
            }
            if x >= self.px(PIN_HIT.0) && x < self.px(LOG_HIT.0) && ok(Hit::Pin) {
                return Some(Hit::Pin);
            }
            if x >= self.px(SETTINGS_HIT.0) && x < self.px(PIN_HIT.0) && ok(Hit::Settings) {
                return Some(Hit::Settings);
            }
            if x >= self.px(PARTY_HIT.0) && x < self.px(SETTINGS_HIT.0) && ok(Hit::Party) {
                return Some(Hit::Party);
            }
        }
        if x >= self.px(UPDATE_HIT.0) && y >= self.px(UPDATE_HIT.1) && ok(Hit::Update) {
            return Some(Hit::Update);
        }
        let regions = [
            Hit::ScaleDown,
            Hit::ScaleUp,
            Hit::AlphaDown,
            Hit::AlphaUp,
            Hit::WindowDown,
            Hit::WindowUp,
            Hit::Sound,
            Hit::Discord,
            Hit::Usage,
            Hit::Mute,
            Hit::RunPrev,
            Hit::RunNext,
            Hit::FightPrev,
            Hit::FightNext,
        ]
        .map(|h| (h.rect(), h));
        regions
            .into_iter()
            .chain(INFO_REGIONS.iter().map(|(i, r)| (*r, Hit::Info(*i))))
            .chain([
                (UPDATE_NOW_HIT, Hit::UpdateNow),
                (UPDATE_LATER_HIT, Hit::UpdateLater),
                (UPDATE_SKIP_HIT, Hit::UpdateSkip),
            ])
            .find(|(r, h)| ok(*h) && self.in_rect(*r, x, y))
            .map(|(_, h)| h)
    }

    pub fn log_hit_test(&self, x: i32, y: i32, thumb: Option<(i32, i32)>) -> Option<LogHit> {
        if y < self.px(LOG_CLOSE_HIT.1 + LOG_CLOSE_HIT.3) && x >= self.px(LOG_CLOSE_HIT.0) {
            return Some(LogHit::Close);
        }
        for (filter, _, tx, tw) in LOG_TABS {
            if self.in_rect((tx, LOG_TAB_Y, tw, LOG_TAB_H), x, y) {
                return Some(LogHit::Tab(filter));
            }
        }
        for (rect, hit) in [(LOG_TITLE, LogHit::Title), (LOG_COUNT, LogHit::Count)] {
            if self.in_rect(rect, x, y) {
                return Some(hit);
            }
        }
        let (tx, ty, tw, th) = LOG_TRACK;
        if let Some((oy, oh)) = thumb {
            if self.in_rect((tx - 6, ty, tw + 12, th), x, y) {
                if self.in_rect((tx - 6, ty + oy, tw + 12, oh), x, y) {
                    return Some(LogHit::Thumb);
                }
                return Some(LogHit::Track);
            }
        }
        None
    }

    pub fn party_hit_test(&self, x: i32, y: i32, vr: bool, pager: bool) -> Option<PartyHit> {
        let (cx, cy, cw, ch) = PARTY_CLOSE_HIT;
        if !vr && self.in_rect((cx, cy, cw, ch), x, y) {
            return Some(PartyHit::Close);
        }
        [
            PartyHit::Toggle,
            PartyHit::Older,
            PartyHit::Newer,
            PartyHit::Status,
            PartyHit::Rank,
            PartyHit::Board,
        ]
        .into_iter()
        .filter(|h| !vr || !h.clickable())
        .filter(|h| pager || !matches!(h, PartyHit::Older | PartyHit::Newer))
        .find(|h| self.in_rect(h.rect(), x, y))
    }
}
