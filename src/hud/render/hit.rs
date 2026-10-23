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
    TakenTitle,
    Taken(u8),
    TakenMore(u8),
    TakenBar,
    LastHit,
    Attackers,
    TopAttacks,
    HistAttackers,
    HistTopAttacks,
    Target,
    Vr,
    LogDot,
    Discord,
    Version,
}

pub const INFO_REGIONS: [(Info, (i32, i32, i32, i32)); 28] = [
    (Info::Status, (14, 26, 332, 20)),
    (Info::Progress, (14, 46, 332, 8)),
    (Info::RunRow, (44, 54, 272, 24)),
    (Info::Boss, (74, 84, 160, 26)),
    (Info::Result, (26, 126, 308, 40)),
    (Info::Target, (26, 126, 308, 40)),
    (Info::Dealt(0), (26, 190, 102, 40)),
    (Info::Dealt(1), (128, 190, 102, 40)),
    (Info::Dealt(2), (230, 190, 102, 40)),
    (Info::DealtGraph, (14, 232, 332, 52)),
    (Info::LastKill, (14, 284, 332, 30)),
    (Info::TakenTitle, (14, 288 + GRAPH_DY, 332, 20)),
    (Info::Taken(0), (26, 310 + GRAPH_DY, 102, 40)),
    (Info::Taken(1), (128, 310 + GRAPH_DY, 102, 40)),
    (Info::Taken(2), (230, 310 + GRAPH_DY, 102, 40)),
    (Info::TakenMore(0), (26, 354 + GRAPH_DY, 102, 36)),
    (Info::TakenMore(1), (128, 354 + GRAPH_DY, 102, 36)),
    (Info::TakenMore(2), (230, 354 + GRAPH_DY, 102, 36)),
    (Info::TakenBar, (14, 392 + GRAPH_DY, 332, 10)),
    (Info::LastHit, (14, 402 + GRAPH_DY, 332, 24)),
    (Info::Attackers, (14, 428 + GRAPH_DY, 332, 48)),
    (Info::TopAttacks, (14, 478 + GRAPH_DY, 332, 82)),
    (Info::HistAttackers, (14, 398 + GRAPH_DY, 332, 48)),
    (Info::HistTopAttacks, (14, 448 + GRAPH_DY, 332, 82)),
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
        matches!(
            self,
            Info::LastHit
                | Info::Target
                | Info::Attackers
                | Info::TopAttacks
                | Info::TakenBar
                | Info::TakenMore(1)
                | Info::TakenMore(2)
        )
    }

    pub fn history_only(self) -> bool {
        matches!(
            self,
            Info::Result | Info::HistAttackers | Info::HistTopAttacks
        )
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
            (Info::TakenTitle, true) => "Damage you took; the right side counts deaths in this run",
            (Info::TakenTitle, false) => "Damage you took in this fight",
            (Info::Taken(0), true) => "Damage taken per second over the stat window from settings",
            (Info::Taken(0), false) => "All damage you took in this fight",
            (Info::Taken(1), true) => "All damage you took in this fight, stage or run",
            (Info::Taken(1), false) => "How many times you were hit in this fight",
            (Info::Taken(2), true) => "How many times you were hit, or deaths during intermission",
            (Info::Taken(2), false) => "Damage taken divided by the fight time",
            (Info::Taken(_), _) => "",
            (Info::TakenMore(0), true) => "The largest single hit on you so far",
            (Info::TakenMore(0), false) => "Damage taken divided by the number of hits",
            (Info::TakenMore(1), _) => "Damage taken divided by the number of hits",
            (Info::TakenMore(2), _) => "Damage taken divided by the fight time so far",
            (Info::TakenMore(_), _) => "",
            (Info::TakenBar, _) => {
                "Incoming damage right now against the worst 10 seconds this fight"
            }
            (Info::LastHit, _) => "Newest hit on you",
            (Info::Attackers | Info::HistAttackers, _) => {
                "Who hurt you this fight, as a share of all damage taken"
            }
            (Info::TopAttacks | Info::HistTopAttacks, _) => {
                "The attacks that hurt most this fight, with how often they hit"
            }
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
            Hit::Settings => "Size, opacity, stat window, aggro sound and Discord status",
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
            Hit::ScaleDown => SCALE_DOWN_HIT,
            Hit::ScaleUp => SCALE_UP_HIT,
            Hit::AlphaDown => ALPHA_DOWN_HIT,
            Hit::AlphaUp => ALPHA_UP_HIT,
            Hit::WindowDown => WINDOW_DOWN_HIT,
            Hit::WindowUp => WINDOW_UP_HIT,
            Hit::Party => PARTY_BTN,
            Hit::Discord => DISCORD_HIT,
            Hit::Sound => SOUND_HIT,
            Hit::Log => LOG_BTN,
            Hit::Close => CLOSE_BTN,
            Hit::Update => UPDATE_RECT,
            Hit::UpdateNow => UPDATE_NOW_HIT,
            Hit::UpdateLater => UPDATE_LATER_HIT,
            Hit::UpdateSkip => UPDATE_SKIP_HIT,
            Hit::Mute => MUTE_HIT,
            Hit::RunPrev => RUN_PREV_HIT,
            Hit::RunNext => RUN_NEXT_HIT,
            Hit::FightPrev => FIGHT_PREV_HIT,
            Hit::FightNext => FIGHT_NEXT_HIT,
            Hit::Info(i) => i.rect(),
        }
    }
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
    Status,
    Rank,
    Board,
}

const PARTY_STATUS: (i32, i32, i32, i32) = (180, 44, 166, 24);
const PARTY_RANK_AREA: (i32, i32, i32, i32) = (14, 68, 332, 48);

impl PartyHit {
    pub fn clickable(self) -> bool {
        matches!(self, PartyHit::Close | PartyHit::Toggle)
    }

    pub fn tip(self, on: bool) -> &'static str {
        match self {
            PartyHit::Close => "Close this window (Esc)",
            PartyHit::Toggle if on => "Stop sharing your boss damage",
            PartyHit::Toggle => "Share your boss damage with others here who turn this on too",
            PartyHit::Status => "Connection to the shared damage server",
            PartyHit::Rank => "Your place among the players here who share, by damage this fight",
            PartyHit::Board => "Boss damage from everyone here who shares it, highest first",
        }
    }

    pub(super) fn rect(self) -> (i32, i32, i32, i32) {
        match self {
            PartyHit::Close => PARTY_CLOSE_BTN,
            PartyHit::Toggle => PARTY_TOGGLE,
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
            (SCALE_DOWN_HIT, Hit::ScaleDown),
            (SCALE_UP_HIT, Hit::ScaleUp),
            (ALPHA_DOWN_HIT, Hit::AlphaDown),
            (ALPHA_UP_HIT, Hit::AlphaUp),
            (WINDOW_DOWN_HIT, Hit::WindowDown),
            (WINDOW_UP_HIT, Hit::WindowUp),
            (SOUND_HIT, Hit::Sound),
            (DISCORD_HIT, Hit::Discord),
            (MUTE_HIT, Hit::Mute),
            (RUN_PREV_HIT, Hit::RunPrev),
            (RUN_NEXT_HIT, Hit::RunNext),
            (FIGHT_PREV_HIT, Hit::FightPrev),
            (FIGHT_NEXT_HIT, Hit::FightNext),
        ];
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

    pub fn party_hit_test(&self, x: i32, y: i32, vr: bool) -> Option<PartyHit> {
        let (cx, cy, cw, ch) = PARTY_CLOSE_HIT;
        if !vr && self.in_rect((cx, cy, cw, ch), x, y) {
            return Some(PartyHit::Close);
        }
        [
            PartyHit::Toggle,
            PartyHit::Status,
            PartyHit::Rank,
            PartyHit::Board,
        ]
        .into_iter()
        .filter(|h| !vr || !h.clickable())
        .find(|h| self.in_rect(h.rect(), x, y))
    }
}
