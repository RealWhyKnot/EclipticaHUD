use crate::log::{self, Filter, Row};
use crate::names::{boss_name, phase_name, stage_name};
use crate::state::{
    base_name, describe_source, fmt_clock, generic_attacker, merge_tallies, phase_num, BossFight,
    GameState, Mode, Tally,
};
use crate::update::{Badge, VERSION};
use crate::vr::VrStatus;
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::*;

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
pub const UPDATE_HIT: (i32, i32) = (210, LOGICAL_H - 32);
pub const TARGET_HIT: (i32, i32, i32, i32) = (26, 126, 308, 40);
pub const RUN_PREV_HIT: (i32, i32, i32, i32) = (14, 54, 26, 24);
pub const RUN_NEXT_HIT: (i32, i32, i32, i32) = (320, 54, 26, 24);
pub const FIGHT_PREV_HIT: (i32, i32, i32, i32) = (240, 86, 26, 20);
pub const FIGHT_NEXT_HIT: (i32, i32, i32, i32) = (308, 86, 26, 20);
pub const PHASE_HIT: (i32, i32, i32, i32) = (220, 110, 114, 20);
const UPDATE_RECT: (i32, i32, i32, i32) =
    (UPDATE_HIT.0, UPDATE_HIT.1, LOGICAL_W - UPDATE_HIT.0, 32);
const TIP_DELAY_MS: u128 = 450;

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

const BG: u32 = rgb(0x14, 0x14, 0x1c);
const CARD: u32 = rgb(0x1d, 0x1d, 0x29);
const CARD_HI: u32 = rgb(0x2a, 0x2a, 0x3c);
const TEXT: u32 = rgb(0xe9, 0xe9, 0xf2);
const DIM: u32 = rgb(0x94, 0x94, 0xac);
const ACCENT: u32 = rgb(0x8a, 0x6c, 0xff);
const DANGER: u32 = rgb(0xff, 0x5c, 0x7c);
const AMBER: u32 = rgb(0xff, 0xc8, 0x57);
const GOOD: u32 = rgb(0x57, 0xd9, 0x9a);

const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn ease_out_cubic(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |shift: u32| {
        let ca = ((a >> shift) & 0xff) as f32;
        let cb = ((b >> shift) & 0xff) as f32;
        (lerp(ca, cb, t.clamp(0.0, 1.0)) as u32) << shift
    };
    ch(0) | ch(8) | ch(16)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hit {
    Pin,
    Log,
    Close,
    Update,
    Target,
    RunPrev,
    RunNext,
    FightPrev,
    FightNext,
    Phase,
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
    DealtBar,
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
    Vr,
    LogDot,
    Version,
}

pub const INFO_REGIONS: [(Info, (i32, i32, i32, i32)); 26] = [
    (Info::Status, (14, 26, 332, 20)),
    (Info::Progress, (14, 46, 332, 8)),
    (Info::RunRow, (44, 54, 272, 24)),
    (Info::Boss, (74, 84, 160, 26)),
    (Info::Result, (26, 126, 308, 40)),
    (Info::Dealt(0), (26, 190, 102, 40)),
    (Info::Dealt(1), (128, 190, 102, 40)),
    (Info::Dealt(2), (230, 190, 102, 40)),
    (Info::DealtBar, (14, 230, 332, 10)),
    (Info::LastKill, (14, 240, 332, 30)),
    (Info::TakenTitle, (14, 288, 332, 20)),
    (Info::Taken(0), (26, 310, 102, 40)),
    (Info::Taken(1), (128, 310, 102, 40)),
    (Info::Taken(2), (230, 310, 102, 40)),
    (Info::TakenMore(0), (26, 354, 102, 36)),
    (Info::TakenMore(1), (128, 354, 102, 36)),
    (Info::TakenMore(2), (230, 354, 102, 36)),
    (Info::TakenBar, (14, 392, 332, 10)),
    (Info::LastHit, (14, 402, 332, 24)),
    (Info::Attackers, (14, 428, 332, 48)),
    (Info::TopAttacks, (14, 478, 332, 82)),
    (Info::HistAttackers, (14, 398, 332, 48)),
    (Info::HistTopAttacks, (14, 448, 332, 82)),
    (Info::Vr, (14, LOGICAL_H - 30, 84, 24)),
    (Info::LogDot, (104, LOGICAL_H - 30, 80, 24)),
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
                | Info::Attackers
                | Info::TopAttacks
                | Info::DealtBar
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

    fn rect(self) -> (i32, i32, i32, i32) {
        INFO_REGIONS
            .iter()
            .find(|(i, _)| *i == self)
            .map_or((0, 0, 0, 0), |(_, r)| *r)
    }

    fn tip(self, live: bool) -> &'static str {
        match (self, live) {
            (Info::Status, _) => "Stage, run phase and class, plus tokens picked up this stage",
            (Info::Progress, _) => "How far the run has progressed through this stage",
            (Info::RunRow, true) => "Run number and the stage counter of the live run",
            (Info::RunRow, false) => {
                "Run number, start time, stage and deaths; LOST means a boss survived"
            }
            (Info::Boss, true) => "The boss you are fighting; (P2) means a later phase",
            (Info::Boss, false) => "The boss of this fight; (P2) means a later phase",
            (Info::Result, _) => {
                "killed, lost (run ended with it alive) or unfinished, plus fight time"
            }
            (Info::Dealt(0), true) => "Damage you dealt in the last 10 seconds, per second",
            (Info::Dealt(0), false) => "All damage you dealt in this fight, across phases",
            (Info::Dealt(1), true) => "Your fight damage divided by the fight time so far",
            (Info::Dealt(1), false) => "Your fight damage divided by the fight time",
            (Info::Dealt(2), true) => "All damage you dealt this fight, across phases",
            (Info::Dealt(2), false) => "How long the fight lasted",
            (Info::Dealt(_), _) => "",
            (Info::DealtBar, _) => "Your DPS right now against your best 10 seconds this fight",
            (Info::LastKill, true) => "What the game credited you with on your last boss kill",
            (Info::LastKill, false) => "What the game credited you with when this boss died",
            (Info::TakenTitle, true) => "Damage you took; the right side counts deaths in this run",
            (Info::TakenTitle, false) => "Damage you took in this fight",
            (Info::Taken(0), true) => "Damage you took in the last 10 seconds, per second",
            (Info::Taken(0), false) => "All damage you took in this fight",
            (Info::Taken(1), true) => "All damage you took this fight, across phases",
            (Info::Taken(1), false) => "How many times you were hit in this fight",
            (Info::Taken(2), true) => "How many times you were hit this fight",
            (Info::Taken(2), false) => "Damage taken divided by the fight time",
            (Info::Taken(_), _) => "",
            (Info::TakenMore(0), true) => "The largest single hit on you this fight",
            (Info::TakenMore(0), false) => "Damage taken divided by the number of hits",
            (Info::TakenMore(1), _) => "Damage taken divided by the number of hits",
            (Info::TakenMore(2), _) => "Damage taken divided by the fight time so far",
            (Info::TakenMore(_), _) => "",
            (Info::TakenBar, _) => {
                "Incoming damage right now against the worst 10 seconds this fight"
            }
            (Info::LastHit, _) => "Newest hit on you; sourceless ticks are status effects",
            (Info::Attackers | Info::HistAttackers, _) => {
                "Who hurt you this fight, as a share of all damage taken"
            }
            (Info::TopAttacks | Info::HistTopAttacks, _) => {
                "The attacks that hurt most this fight, with how often they hit"
            }
            (Info::Vr, _) => "SteamVR wrist overlay: green when attached, red when failing",
            (Info::LogDot, _) => "VRChat output log: green when found, amber when missing",
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
}

impl Hit {
    pub fn clickable(self) -> bool {
        !matches!(self, Hit::Info(_))
    }

    pub fn tip(self, c: TipCtx) -> &'static str {
        match self {
            Hit::Pin if c.topmost => "Kept above other windows; click to let them cover it",
            Hit::Pin => "Other windows can cover the HUD; click to keep it on top",
            Hit::Log if c.log_open => "Close the event log window",
            Hit::Log => "Open the event log window",
            Hit::Close => "Close the HUD (Esc)",
            Hit::Update => "Install this update and restart",
            Hit::Target if c.sound_on => "Who the boss is after; click to mute the aggro sound",
            Hit::Target => "Who the boss is after; click to unmute the aggro sound",
            Hit::RunPrev => "Show the previous run",
            Hit::RunNext => "Show the next run; past the newest returns to live",
            Hit::FightPrev => "Show the previous boss fight of this run",
            Hit::FightNext => "Show the next boss fight of this run",
            Hit::Phase => "Switch between the phases of this fight and the total",
            Hit::Info(i) => i.tip(c.live),
        }
    }

    fn rect(self) -> (i32, i32, i32, i32) {
        match self {
            Hit::Pin => PIN_BTN,
            Hit::Log => LOG_BTN,
            Hit::Close => CLOSE_BTN,
            Hit::Update => UPDATE_RECT,
            Hit::Target => TARGET_HIT,
            Hit::RunPrev => RUN_PREV_HIT,
            Hit::RunNext => RUN_NEXT_HIT,
            Hit::FightPrev => FIGHT_PREV_HIT,
            Hit::FightNext => FIGHT_NEXT_HIT,
            Hit::Phase => PHASE_HIT,
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

pub const LOG_TITLE: (i32, i32, i32, i32) = (14, LOG_TAB_Y, 74, LOG_TAB_H);
pub const LOG_COUNT: (i32, i32, i32, i32) = (88, LOG_TAB_Y, 76, LOG_TAB_H);

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

    fn rect(self, thumb: Option<(i32, i32)>) -> (i32, i32, i32, i32) {
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

pub struct Frame {
    pub now: u64,
    pub flash_t: f32,
    pub taken_flash_t: f32,
    pub dead_pulse: f32,
    pub hover: Option<Hit>,
    pub pressed: Option<Hit>,
    pub tip: Option<Hit>,
    pub vr: VrStatus,
    pub log_ok: bool,
    pub log_open: bool,
    pub topmost: bool,
    pub progress_shown: f32,
    pub dps_frac_shown: f32,
    pub taken_frac_shown: f32,
    pub dps_shown: f32,
    pub fight_dps_shown: f32,
    pub taken_shown: f32,
    pub taken_rate_shown: f32,
    pub update: Badge,
    pub sound_on: bool,
    pub view_run: Option<usize>,
    pub view_group: Option<usize>,
    pub view_phase: Option<usize>,
    pub run_sel: bool,
    pub group_sel: bool,
}

impl Frame {
    fn live(&self) -> bool {
        !self.run_sel && !self.group_sel && self.view_phase.is_none()
    }
}

pub struct LogView {
    pub scroll: f32,
    pub filter: Filter,
    pub hover: Option<LogHit>,
    pub dragging: bool,
    pub thumb_t: f32,
    pub slide: f32,
    pub tip: Option<LogHit>,
    pub thumb: Option<(i32, i32)>,
}

struct HistView<'a> {
    fights: &'a [BossFight],
    name: &'a str,
    start: u64,
    dmg: u64,
    taken: u64,
    hits: u32,
    kill: Option<(u64, u64)>,
    last: &'a BossFight,
    n_phases: usize,
}

pub struct Renderer {
    pub width: i32,
    pub height: i32,
    pub dc: HDC,
    bitmap: HBITMAP,
    pub bits: *mut u8,
    scale: f32,
    fonts: [HFONT; 6],
}

const F_TINY: usize = 0;
const F_BODY: usize = 1;
const F_LABEL: usize = 2;
const F_BOSS: usize = 3;
const F_BIG: usize = 4;
const F_GLYPH: usize = 5;

const GLYPH_CLOSE: &str = "\u{E8BB}";
const GLYPH_MUTE: &str = "\u{E74F}";
const GLYPH_PREV: &str = "\u{E76B}";
const GLYPH_NEXT: &str = "\u{E76C}";
const GLYPH_LOG: &str = "\u{E81C}";
const GLYPH_PIN: &str = "\u{E718}";
const GLYPH_UNPIN: &str = "\u{E77A}";

const SEG_COLORS: [u32; 4] = [DANGER, AMBER, ACCENT, DIM];

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

    pub fn draw_main(&mut self, gs: &mut GameState, f: &Frame) {
        const M: i32 = 14;
        const W: i32 = LOGICAL_W - 2 * M;
        let now = f.now;
        let hov = |h: Hit| f.hover == Some(h);
        let prs = |h: Hit| f.pressed == Some(h);
        self.fill(0, 0, LOGICAL_W, LOGICAL_H, BG);

        self.text_rect(
            M,
            8,
            W,
            24,
            F_LABEL,
            ACCENT,
            DT_LEFT | DT_VCENTER,
            "ECLIPTICA HUD",
        );
        self.glyph_button(
            PIN_BTN,
            hov(Hit::Pin),
            prs(Hit::Pin),
            f.topmost,
            if f.topmost { GLYPH_PIN } else { GLYPH_UNPIN },
        );
        self.glyph_button(LOG_BTN, hov(Hit::Log), prs(Hit::Log), f.log_open, GLYPH_LOG);
        self.glyph_button(
            CLOSE_BTN,
            hov(Hit::Close),
            prs(Hit::Close),
            false,
            GLYPH_CLOSE,
        );

        let (status, status_color) = match gs.mode {
            Mode::Idle => ("waiting for a run".to_string(), DIM),
            Mode::Lobby => ("in lobby".to_string(), AMBER),
            Mode::Intermission => ("intermission".to_string(), ACCENT),
            Mode::Stage => (
                format!(
                    "{}   {}   {}",
                    stage_name(&gs.stage),
                    phase_name(gs.progress),
                    gs.class
                ),
                GOOD,
            ),
        };
        self.text(M, 28, W, F_BODY, status_color, DT_LEFT, &status);
        if gs.is_dead(now) {
            let color = mix(mix(BG, DANGER, 0.45), DANGER, f.dead_pulse);
            self.text(M, 28, W, F_BODY, color, DT_RIGHT, "DEAD");
        } else if let Some((got, total)) = gs.tokens_shown() {
            let rune = gs.level_tokens.iter().any(|t| t.0);
            let txt = format!(
                "{got}/{total} token{}{}",
                if total == 1 { "" } else { "s" },
                if rune { " +rune" } else { "" }
            );
            let color = if got == total {
                GOOD
            } else if rune {
                AMBER
            } else {
                DIM
            };
            self.text_rect(M, 28, W, 18, F_TINY, color, DT_RIGHT | DT_VCENTER, &txt);
        }
        if gs.mode == Mode::Stage {
            self.bar(M, 47, W, 4, f.progress_shown, GOOD);
        }

        self.arrow(
            RUN_PREV_HIT,
            f.view_run.is_some_and(|i| i > 0),
            hov(Hit::RunPrev),
            prs(Hit::RunPrev),
            GLYPH_PREV,
        );
        self.arrow(
            RUN_NEXT_HIT,
            f.run_sel,
            hov(Hit::RunNext),
            prs(Hit::RunNext),
            GLYPH_NEXT,
        );
        let (run_label, run_color) = match f.view_run {
            None => ("no runs yet".to_string(), DIM),
            Some(i) if f.live() => {
                let mut s = format!("RUN {}/{}", i + 1, gs.runs.len());
                if let Some(n) = gs.stage_no {
                    s.push_str(&format!("   stage {n}"));
                }
                (s, DIM)
            }
            Some(i) => {
                let r = &gs.runs[i];
                let lost = r.lost_fight().is_some();
                let mut s = format!(
                    "{} {}/{}   {}   {}",
                    if lost { "LOST" } else { "RUN" },
                    i + 1,
                    gs.runs.len(),
                    &fmt_clock(r.start_ts)[..5],
                    stage_name(&r.stage)
                );
                if let Some(n) = r.stage_no {
                    s.push_str(&format!("   stage {n}"));
                }
                if r.deaths > 0 {
                    s.push_str(&format!("   {}", fmt_run_deaths(r.deaths)));
                }
                (s, if lost { DANGER } else { TEXT })
            }
        };
        self.text_rect(
            44,
            54,
            LOGICAL_W - 88,
            24,
            F_LABEL,
            run_color,
            DT_CENTER | DT_VCENTER,
            &run_label,
        );

        self.rround(M, 82, W, 96, 8, CARD);
        let viewed_run = f.view_run.map(|i| &gs.runs[i]);
        let groups = viewed_run.map(|r| r.groups()).unwrap_or_default();
        let viewed_group = viewed_run.and_then(|r| {
            f.view_group
                .and_then(|i| groups.get(i))
                .map(|g| &r.fights[g.clone()])
        });
        let hist = viewed_group.map(|fights| match f.view_phase.and_then(|i| fights.get(i)) {
            Some(ph) => HistView {
                fights: std::slice::from_ref(ph),
                name: ph.name.as_str(),
                start: ph.start_ts,
                dmg: ph.dmg,
                taken: ph.taken,
                hits: ph.hits,
                kill: ph.kill,
                last: ph,
                n_phases: fights.len(),
            },
            None => {
                let last = &fights[fights.len() - 1];
                HistView {
                    fights,
                    name: base_name(&last.name),
                    start: fights[0].start_ts,
                    dmg: fights.iter().map(|p| p.dmg).sum(),
                    taken: fights.iter().map(|p| p.taken).sum(),
                    hits: fights.iter().map(|p| p.hits).sum(),
                    kill: fights
                        .iter()
                        .filter_map(|p| p.kill)
                        .reduce(|a, b| (a.0 + b.0, a.1 + b.1)),
                    last,
                    n_phases: fights.len(),
                }
            }
        });
        self.text(M + 12, 88, W - 24, F_LABEL, DIM, DT_LEFT, "BOSS");
        if !groups.is_empty() {
            self.arrow(
                FIGHT_PREV_HIT,
                f.view_group.is_some_and(|i| i > 0),
                hov(Hit::FightPrev),
                prs(Hit::FightPrev),
                GLYPH_PREV,
            );
            self.arrow(
                FIGHT_NEXT_HIT,
                f.group_sel,
                hov(Hit::FightNext),
                prs(Hit::FightNext),
                GLYPH_NEXT,
            );
            let idx = format!(
                "{}/{}",
                f.view_group.map_or(groups.len(), |i| i + 1),
                groups.len()
            );
            self.text_rect(266, 86, 42, 20, F_TINY, DIM, DT_CENTER | DT_VCENTER, &idx);
        }
        if f.live() {
            match (&gs.boss, &gs.target) {
                (Some(boss), target) => {
                    let pn = phase_num(boss);
                    let shown = if pn > 1 {
                        format!("{} (P{pn})", boss_name(base_name(boss)))
                    } else {
                        boss_name(boss).to_string()
                    };
                    self.text(M + 60, 88, 166, F_BOSS, TEXT, DT_LEFT, &shown);
                    self.text(M + 12, 114, W - 24, F_LABEL, DIM, DT_LEFT, "TARGET");
                    if !f.sound_on {
                        self.text(M + 12, 114, W - 24, F_GLYPH, DIM, DT_RIGHT, GLYPH_MUTE);
                    }
                    let color = mix(ACCENT, TEXT, ease_out_cubic(f.flash_t));
                    match target {
                        Some(t) => {
                            if hov(Hit::Target) || prs(Hit::Target) {
                                let (tx, ty, tw, th) = TARGET_HIT;
                                let tint = if prs(Hit::Target) { ACCENT } else { CARD_HI };
                                self.rround(tx - 6, ty - 2, tw + 6, th, 6, tint);
                            }
                            self.text(M + 12, 128, W - 24, F_BIG, color, DT_LEFT, t);
                            let held = now.saturating_sub(gs.target_since);
                            self.text_rect(
                                M + 12,
                                128,
                                W - 24,
                                34,
                                F_TINY,
                                DIM,
                                DT_RIGHT | DT_VCENTER,
                                &format!("{held}s"),
                            );
                        }
                        None => self.text(M + 12, 128, W - 24, F_BOSS, DIM, DT_LEFT, "-"),
                    }
                }
                (None, _) => {
                    self.text(M + 12, 112, W - 24, F_BOSS, DIM, DT_LEFT, "no boss active");
                    let lost = gs.runs.last().and_then(|r| r.lost_fight().map(|f| (r, f)));
                    if let Some((r, fight)) = lost {
                        let mut line =
                            format!("last run lost to {}", boss_name(base_name(&fight.name)));
                        if r.deaths > 0 {
                            line.push_str(&format!("   {}", fmt_run_deaths(r.deaths)));
                        }
                        self.text(M + 12, 140, W - 24, F_BODY, DANGER, DT_LEFT, &line);
                    }
                }
            }
        } else {
            match &hist {
                Some(h) => {
                    self.text(
                        M + 60,
                        88,
                        166,
                        F_BOSS,
                        TEXT,
                        DT_LEFT,
                        boss_name(base_name(h.name)),
                    );
                    self.text(M + 12, 114, W - 24, F_LABEL, DIM, DT_LEFT, "RESULT");
                    if h.n_phases > 1 {
                        let chip = match f.view_phase {
                            Some(i) => format!("phase {}/{}", i + 1, h.n_phases),
                            None => format!("{} phases", h.n_phases),
                        };
                        let (cx, cy, cw, ch) = PHASE_HIT;
                        if hov(Hit::Phase) || prs(Hit::Phase) {
                            let tint = if prs(Hit::Phase) { ACCENT } else { CARD_HI };
                            self.rround(cx + 40, cy, cw - 40, ch, 5, tint);
                        }
                        let cc = if prs(Hit::Phase) { BG } else { DIM };
                        self.text_rect(
                            cx,
                            cy,
                            cw - 4,
                            ch,
                            F_TINY,
                            cc,
                            DT_RIGHT | DT_VCENTER,
                            &chip,
                        );
                    }
                    let lost = viewed_run
                        .and_then(|r| r.lost_fight())
                        .is_some_and(|f| f.start_ts == h.last.start_ts && f.name == h.last.name);
                    let (res, color) = match (h.last.kill, h.last.end_ts) {
                        (Some(_), _) => ("killed", GOOD),
                        (None, Some(_)) if lost => ("lost", DANGER),
                        (None, Some(_)) => ("unfinished", DIM),
                        (None, None) => ("in progress", AMBER),
                    };
                    self.text(M + 12, 128, W - 24, F_BOSS, color, DT_LEFT, res);
                    let end = h.last.end_ts.unwrap_or(now);
                    let deaths: u32 = h.fights.iter().map(|p| p.deaths).sum();
                    let mut right = fmt_dur(end.saturating_sub(h.start));
                    if deaths > 0 {
                        right = format!("died {deaths}x   {right}");
                    }
                    self.text_rect(
                        M + 12,
                        128,
                        W - 24,
                        22,
                        F_TINY,
                        DIM,
                        DT_RIGHT | DT_VCENTER,
                        &right,
                    );
                }
                None => self.text(M + 12, 112, W - 24, F_BOSS, DIM, DT_LEFT, "no boss fights"),
            }
        }

        self.rround(M, 186, W, 92, 8, CARD);
        let col = (W - 24) / 3;
        let stat = |r: &Renderer, y: i32, i: i32, label: &str, value: &str, color: u32| {
            let x = M + 12 + i * col;
            r.text(x, y, col, F_LABEL, DIM, DT_LEFT, label);
            r.text(x, y + 16, col, F_BOSS, color, DT_LEFT, value);
        };
        if f.live() {
            stat(self, 192, 0, "DPS 10s", &fmt_anim(f.dps_shown), AMBER);
            stat(
                self,
                192,
                1,
                "FIGHT DPS",
                &fmt_anim(f.fight_dps_shown),
                AMBER,
            );
            stat(
                self,
                192,
                2,
                "FIGHT DMG",
                &group_digits(gs.fight_dmg),
                AMBER,
            );
            if gs.boss.is_some() {
                self.bar(M + 12, 233, W - 24, 3, f.dps_frac_shown, ACCENT);
            }
            match &gs.last_kill {
                Some(k) => {
                    let line = format!(
                        "last kill  {}   {} strike + {} other",
                        boss_name(base_name(&k.boss)),
                        group_digits(k.strike),
                        group_digits(k.non_strike)
                    );
                    self.text(M + 12, 242, W - 24, F_BODY, TEXT, DT_LEFT, &line);
                }
                None => self.text(M + 12, 242, W - 24, F_BODY, DIM, DT_LEFT, "no kills yet"),
            }
        } else if let Some(h) = &hist {
            let end = h.last.end_ts.unwrap_or(now);
            let dur = end.saturating_sub(h.start);
            stat(self, 192, 0, "DMG", &group_digits(h.dmg), AMBER);
            stat(
                self,
                192,
                1,
                "DPS",
                &(h.dmg / dur.max(1)).to_string(),
                AMBER,
            );
            stat(self, 192, 2, "TIME", &fmt_dur(dur), AMBER);
            match h.kill {
                Some((s, ns)) => {
                    let line = format!(
                        "kill  {} strike + {} other",
                        group_digits(s),
                        group_digits(ns)
                    );
                    self.text(M + 12, 242, W - 24, F_BODY, TEXT, DT_LEFT, &line);
                }
                None => {
                    self.text(
                        M + 12,
                        242,
                        W - 24,
                        F_BODY,
                        DIM,
                        DT_LEFT,
                        "no kill recorded",
                    );
                }
            }
        } else {
            self.text(M + 12, 208, W - 24, F_BODY, DIM, DT_LEFT, "no data");
        }

        let card = mix(CARD, DANGER, 0.22 * (1.0 - ease_out_cubic(f.taken_flash_t)));
        self.rround(M, 286, W, 276, 8, card);
        self.text(M + 12, 292, W - 24, F_LABEL, DIM, DT_LEFT, "DAMAGE TAKEN");
        let live_deaths = gs
            .runs
            .last()
            .filter(|r| r.end_ts.is_none())
            .map_or(0, |r| r.deaths);
        if f.live() && live_deaths > 0 {
            self.text(
                M + 12,
                292,
                W - 24,
                F_TINY,
                DIM,
                DT_RIGHT,
                &fmt_run_deaths(live_deaths),
            );
        }
        let small = |r: &Renderer, y: i32, i: i32, label: &str, value: &str| {
            let x = M + 12 + i * col;
            r.text(x, y, col, F_LABEL, DIM, DT_LEFT, label);
            r.text(x, y + 15, col, F_BODY, TEXT, DT_LEFT, value);
        };
        let dash = "-".to_string();
        if f.live() {
            let in_fight = gs.boss.is_some();
            let or_dash = |s: String| if in_fight { s } else { dash.clone() };
            stat(self, 312, 0, "TAKEN 10s", &fmt_anim(f.taken_shown), DANGER);
            stat(
                self,
                308,
                1,
                "FIGHT TAKEN",
                &or_dash(group_digits(gs.fight_taken)),
                DANGER,
            );
            stat(
                self,
                308,
                2,
                "HITS",
                &or_dash(gs.fight_hits.to_string()),
                DANGER,
            );
            let avg = if gs.fight_hits > 0 {
                gs.fight_taken / gs.fight_hits as u64
            } else {
                0
            };
            small(
                self,
                350,
                0,
                "BIGGEST HIT",
                &or_dash(gs.fight_max_hit.to_string()),
            );
            small(self, 356, 1, "AVG HIT", &or_dash(avg.to_string()));
            small(
                self,
                350,
                2,
                "TAKEN/S",
                &or_dash(fmt_anim(f.taken_rate_shown)),
            );
            if in_fight {
                self.bar_on(M + 12, 396, W - 24, 3, f.taken_frac_shown, DANGER, BG);
            }
            match gs.taken.back() {
                Some(hit) => {
                    let (who, attack) = describe_source(&hit.source, hit.amount);
                    let line = format!("last hit  {}   {who}   {attack}", hit.amount);
                    self.text_rect(
                        M + 12,
                        402,
                        W - 80,
                        20,
                        F_BODY,
                        TEXT,
                        DT_LEFT | DT_VCENTER,
                        &line,
                    );
                    self.text_rect(
                        M + 12,
                        402,
                        W - 24,
                        20,
                        F_TINY,
                        DIM,
                        DT_RIGHT | DT_VCENTER,
                        &fmt_ago(now, hit.ts),
                    );
                }
                None => self.text(M + 12, 404, W - 24, F_BODY, DIM, DT_LEFT, "no hits yet"),
            }
            if !in_fight {
                self.text_rect(
                    M + 12,
                    430,
                    W - 24,
                    120,
                    F_BODY,
                    DIM,
                    DT_CENTER | DT_VCENTER,
                    "attack breakdown starts with the next boss",
                );
            } else {
                self.breakdown(430, &gs.fight_attacks, gs.fight_taken);
            }
        } else if let Some(h) = &hist {
            let end = h.last.end_ts.unwrap_or(now);
            let dur = end.saturating_sub(h.start);
            stat(self, 312, 0, "TAKEN", &group_digits(h.taken), DANGER);
            stat(self, 312, 1, "HITS", &h.hits.to_string(), DANGER);
            stat(
                self,
                312,
                2,
                "TAKEN/S",
                &(h.taken / dur.max(1)).to_string(),
                DANGER,
            );
            let avg = if h.hits > 0 {
                h.taken / h.hits as u64
            } else {
                0
            };
            let big = if h.taken > 0 {
                avg.to_string()
            } else {
                dash.clone()
            };
            small(self, 356, 0, "AVG HIT", &big);
            let attacks = merge_tallies(h.fights.iter().map(|f| f.attacks.as_slice()));
            self.breakdown(400, &attacks, h.taken);
        } else {
            self.text(M + 12, 324, W - 24, F_BODY, DIM, DT_LEFT, "no data");
        }

        let fy = LOGICAL_H - 26;
        let vr_color = match f.vr {
            VrStatus::On => GOOD,
            VrStatus::Off => DIM,
            VrStatus::Failing => DANGER,
        };
        self.dot(M, fy + 4, 8, vr_color);
        self.text(M + 14, fy, 80, F_TINY, DIM, DT_LEFT, "STEAMVR");
        let log_color = if f.log_ok { GOOD } else { AMBER };
        self.dot(M + 96, fy + 4, 8, log_color);
        self.text(M + 110, fy, 80, F_TINY, DIM, DT_LEFT, "LOG");
        let (utext, ucolor) = match &f.update {
            Badge::None => (VERSION.to_string(), DIM),
            Badge::Ready(tag) => (format!("update {tag}"), ACCENT),
            Badge::Installing => ("updating".to_string(), AMBER),
            Badge::Failed => ("update failed".to_string(), DANGER),
        };
        let ucolor = if hov(Hit::Update) && matches!(f.update, Badge::Ready(_)) {
            TEXT
        } else {
            ucolor
        };
        self.text(M, fy, W, F_TINY, ucolor, DT_RIGHT, &utext);
        if let Some(hit) = f.tip {
            let ctx = TipCtx {
                live: f.live(),
                topmost: f.topmost,
                sound_on: f.sound_on,
                log_open: f.log_open,
            };
            self.tooltip(hit.rect(), hit.tip(ctx), LOGICAL_W, LOGICAL_H);
        }
        unsafe { GdiFlush() };
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

    fn breakdown(&self, y0: i32, tallies: &[Tally], total: u64) {
        const M: i32 = 14;
        const W: i32 = LOGICAL_W - 2 * M;
        if tallies.is_empty() {
            self.text_rect(
                M + 12,
                y0,
                W - 24,
                120,
                F_BODY,
                DIM,
                DT_CENTER | DT_VCENTER,
                "untouched so far",
            );
            return;
        }
        self.text(M + 12, y0, W - 24, F_LABEL, DIM, DT_LEFT, "BY ATTACKER");
        self.text(
            M + 12,
            y0 + 50,
            W - 24,
            F_LABEL,
            DIM,
            DT_LEFT,
            "TOP ATTACKS",
        );
        let mut attackers: Vec<(&str, u64)> = Vec::new();
        for t in tallies {
            match attackers.iter_mut().find(|a| a.0 == t.who) {
                Some(a) => a.1 += t.total,
                None => attackers.push((&t.who, t.total)),
            }
        }
        attackers.sort_by_key(|a| std::cmp::Reverse(a.1));
        let total = total.max(1) as f32;
        let bw = W - 24;
        let by = y0 + 16;
        self.rround(M + 12, by, bw, 8, 4, BG);
        let mut x = 0;
        let mut legend = String::new();
        let mut other = 0;
        for (i, (name, amt)) in attackers.iter().enumerate() {
            if i >= 3 {
                other += amt;
                continue;
            }
            let w = ((bw as f32) * (*amt as f32 / total)) as i32;
            if w >= 4 {
                self.rround(M + 12 + x, by, w, 8, 4, SEG_COLORS[i]);
            }
            x += w;
            if !legend.is_empty() {
                legend.push_str("   ");
            }
            legend.push_str(&format!(
                "{name} {}%",
                (*amt as f32 / total * 100.0).round() as u32
            ));
        }
        if other > 0 {
            let w = ((bw as f32) * (other as f32 / total)) as i32;
            if w >= 4 {
                self.rround(M + 12 + x, by, w, 8, 4, SEG_COLORS[3]);
            }
            legend.push_str(&format!(
                "   other {}%",
                (other as f32 / total * 100.0).round() as u32
            ));
        }
        self.text(M + 12, y0 + 28, W - 24, F_TINY, DIM, DT_LEFT, &legend);
        let mut attacks: Vec<&Tally> = tallies.iter().collect();
        attacks.sort_by_key(|a| std::cmp::Reverse(a.total));
        let top = attacks.first().map_or(1, |a| a.total).max(1) as f32;
        for (i, t) in attacks.iter().take(3).enumerate() {
            let y = y0 + 66 + i as i32 * 20;
            let label = if generic_attacker(&t.who) {
                t.attack.clone()
            } else {
                format!("{}  {}", t.who, t.attack)
            };
            self.text_rect(
                M + 12,
                y,
                W - 110,
                17,
                F_BODY,
                TEXT,
                DT_LEFT | DT_VCENTER,
                &label,
            );
            let right = format!("{}  x{}", group_digits(t.total), t.hits);
            self.text_rect(
                M + 12,
                y,
                W - 24,
                17,
                F_TINY,
                DIM,
                DT_RIGHT | DT_VCENTER,
                &right,
            );
            self.bar_on(
                M + 12,
                y + 17,
                W - 24,
                2,
                t.total as f32 / top,
                mix(CARD, DANGER, 0.6),
                BG,
            );
        }
    }

    pub fn draw_log(&mut self, gs: &GameState, lv: &LogView) {
        self.fill(0, 0, LOG_W, LOG_H, BG);
        let rows = log::timeline(gs, lv.filter);
        self.text_rect(
            14,
            LOG_TAB_Y,
            74,
            LOG_TAB_H,
            F_LABEL,
            ACCENT,
            DT_LEFT | DT_VCENTER,
            "EVENT LOG",
        );
        let count = match rows.iter().filter(|r| !matches!(r, Row::Fight(_))).count() {
            0 => "nothing yet".to_string(),
            1 => "1 event".to_string(),
            n => format!("{n} events"),
        };
        self.text_rect(
            88,
            LOG_TAB_Y,
            76,
            LOG_TAB_H,
            F_TINY,
            DIM,
            DT_LEFT | DT_VCENTER,
            &count,
        );
        for (filter, label, x, w) in LOG_TABS {
            let active = lv.filter == filter;
            let hover = lv.hover == Some(LogHit::Tab(filter));
            let (bg, fg) = if active {
                (ACCENT, BG)
            } else if hover {
                (CARD_HI, TEXT)
            } else {
                (CARD, DIM)
            };
            self.rround(x, LOG_TAB_Y, w, LOG_TAB_H, 6, bg);
            self.text_rect(
                x,
                LOG_TAB_Y,
                w,
                LOG_TAB_H,
                F_LABEL,
                fg,
                DT_CENTER | DT_VCENTER,
                label,
            );
        }
        self.glyph_button(
            LOG_CLOSE_BTN,
            lv.hover == Some(LogHit::Close),
            false,
            false,
            GLYPH_CLOSE,
        );

        let (bx, by, bw, bh) = LOG_BODY;
        if rows.is_empty() {
            self.text_rect(
                bx,
                by,
                bw,
                bh,
                F_BODY,
                DIM,
                DT_CENTER | DT_VCENTER,
                "nothing logged yet",
            );
            unsafe { GdiFlush() };
            return;
        }
        unsafe {
            let saved = SaveDC(self.dc);
            IntersectClipRect(
                self.dc,
                self.px(bx),
                self.px(by),
                self.px(bx + bw),
                self.px(by + bh),
            );
            let first = (lv.scroll / log::ROW_H as f32).floor().max(0.0) as usize;
            let mut y = by + (first as i32 * log::ROW_H) - lv.scroll as i32 + lv.slide as i32;
            for row in rows.iter().skip(first) {
                if y > by + bh {
                    break;
                }
                self.draw_log_row(bx, y, bw, row);
                y += log::ROW_H;
            }
            RestoreDC(self.dc, saved);
        }
        if let Some((ty, th)) = log::thumb(rows.len(), bh, lv.scroll) {
            let (tx, tyy, tw, thh) = LOG_TRACK;
            self.rround(tx, tyy, tw, thh, 3, CARD);
            let lit = lv.dragging || lv.hover == Some(LogHit::Thumb);
            let color = if lit {
                ACCENT
            } else {
                mix(CARD_HI, DIM, lv.thumb_t)
            };
            self.rround(tx, tyy + ty, tw, th, 3, color);
        }
        if let Some(hit) = lv.tip {
            self.tooltip(hit.rect(lv.thumb), hit.tip(), LOG_W, LOG_H);
        }
        unsafe { GdiFlush() };
    }

    fn draw_log_row(&self, x: i32, y: i32, w: i32, row: &Row) {
        match row {
            Row::Fight(f) => {
                self.fill(x, y + 12, w, 1, CARD_HI);
                let pn = phase_num(&f.name);
                let name = if pn > 1 {
                    format!("{} (P{pn})", boss_name(base_name(&f.name)))
                } else {
                    boss_name(&f.name).to_string()
                };
                let label = format!("{name}  {}", &fmt_clock(f.start_ts)[..5]);
                let tw = label.chars().count() as i32 * 7 + 16;
                self.fill(x, y, tw, log::ROW_H, BG);
                self.text_rect(
                    x + 4,
                    y,
                    tw,
                    log::ROW_H,
                    F_LABEL,
                    DIM,
                    DT_LEFT | DT_VCENTER,
                    &label,
                );
            }
            Row::Hit(h) => {
                self.rround(x, y + 5, 3, log::ROW_H - 10, 1, DANGER);
                let vc = DT_LEFT | DT_VCENTER;
                self.text_rect(x + 12, y, 62, log::ROW_H, F_TINY, DIM, vc, &fmt_clock(h.ts));
                self.text_rect(
                    x + 76,
                    y,
                    44,
                    log::ROW_H,
                    F_BODY,
                    DANGER,
                    DT_RIGHT | DT_VCENTER,
                    &h.amount.to_string(),
                );
                let (who, attack) = describe_source(&h.source, h.amount);
                let who_color = if generic_attacker(&who) { DIM } else { TEXT };
                self.text_rect(x + 130, y, 120, log::ROW_H, F_BODY, who_color, vc, &who);
                self.text_rect(x + 254, y, w - 254, log::ROW_H, F_BODY, DIM, vc, &attack);
            }
            Row::Target(t) => {
                self.rround(x, y + 5, 3, log::ROW_H - 10, 1, ACCENT);
                let vc = DT_LEFT | DT_VCENTER;
                self.text_rect(x + 12, y, 62, log::ROW_H, F_TINY, DIM, vc, &fmt_clock(t.ts));
                self.text_rect(
                    x + 76,
                    y,
                    44,
                    log::ROW_H,
                    F_TINY,
                    ACCENT,
                    DT_RIGHT | DT_VCENTER,
                    "aggro",
                );
                self.text_rect(x + 130, y, 120, log::ROW_H, F_BODY, TEXT, vc, &t.player);
                self.text_rect(
                    x + 254,
                    y,
                    w - 254,
                    log::ROW_H,
                    F_BODY,
                    DIM,
                    vc,
                    boss_name(base_name(&t.boss)),
                );
            }
            Row::Death { ts, .. } => {
                self.rround(x, y + 5, 3, log::ROW_H - 10, 1, DANGER);
                let vc = DT_LEFT | DT_VCENTER;
                self.text_rect(x + 12, y, 62, log::ROW_H, F_TINY, DIM, vc, &fmt_clock(*ts));
                self.text_rect(
                    x + 76,
                    y,
                    44,
                    log::ROW_H,
                    F_TINY,
                    DANGER,
                    DT_RIGHT | DT_VCENTER,
                    "death",
                );
                self.text_rect(
                    x + 130,
                    y,
                    w - 130,
                    log::ROW_H,
                    F_BODY,
                    DANGER,
                    vc,
                    "you died",
                );
            }
        }
    }

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
        }
        if x >= self.px(UPDATE_HIT.0) && y >= self.px(UPDATE_HIT.1) && ok(Hit::Update) {
            return Some(Hit::Update);
        }
        let regions = [
            (TARGET_HIT, Hit::Target),
            (RUN_PREV_HIT, Hit::RunPrev),
            (RUN_NEXT_HIT, Hit::RunNext),
            (FIGHT_PREV_HIT, Hit::FightPrev),
            (FIGHT_NEXT_HIT, Hit::FightNext),
            (PHASE_HIT, Hit::Phase),
        ];
        regions
            .into_iter()
            .chain(INFO_REGIONS.iter().map(|(i, r)| (*r, Hit::Info(*i))))
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

fn fmt_anim(v: f32) -> String {
    (v.max(0.0).round() as u64).to_string()
}

fn fmt_ago(now: u64, ts: u64) -> String {
    let d = now.saturating_sub(ts);
    if d < 60 {
        format!("{d}s ago")
    } else {
        fmt_clock(ts)
    }
}

fn fmt_run_deaths(n: u32) -> String {
    format!("{n} run death{}", if n == 1 { "" } else { "s" })
}

fn fmt_dur(secs: u64) -> String {
    if secs >= 60 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

fn group_digits(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Frame {
        Frame {
            now: 0,
            flash_t: 1.0,
            taken_flash_t: 1.0,
            dead_pulse: 0.0,
            hover: Some(Hit::Close),
            pressed: None,
            tip: None,
            vr: VrStatus::Off,
            log_ok: true,
            log_open: false,
            topmost: true,
            progress_shown: 0.5,
            dps_frac_shown: 0.5,
            taken_frac_shown: 0.5,
            dps_shown: 0.0,
            fight_dps_shown: 0.0,
            taken_shown: 0.0,
            taken_rate_shown: 0.0,
            update: Badge::None,
            sound_on: true,
            view_run: Some(0),
            view_group: Some(0),
            view_phase: None,
            run_sel: false,
            group_sel: false,
        }
    }

    fn pix(r: &Renderer, x: i32, y: i32) -> u32 {
        let s = unsafe { std::slice::from_raw_parts(r.bits, (r.width * r.height * 4) as usize) };
        let i = ((y * r.width + x) * 4) as usize;
        rgb(s[i + 2] as u32, s[i + 1] as u32, s[i] as u32)
    }

    #[test]
    fn close_btn_inside_hit() {
        let (hx, hy, hw, hh) = CLOSE_HIT;
        let (bx, by, bw, bh) = CLOSE_BTN;
        assert_eq!(hx + hw, LOGICAL_W);
        assert_eq!(hy, 0);
        assert!(bx >= hx && by >= hy);
        assert!(bx + bw <= hx + hw && by + bh <= hy + hh);
        assert!(by >= 8);
        assert!(LOGICAL_W - (bx + bw) >= 8);
        let (lx, ly, lw, lh) = LOG_HIT;
        let (bx, by, bw, bh) = LOG_BTN;
        assert_eq!(lx + lw, hx);
        assert!(bx >= lx && by >= ly && bx + bw <= lx + lw && by + bh <= ly + lh);
    }

    #[test]
    fn easing() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        let mut prev = 0.0;
        for i in 1..=10 {
            let v = ease_out_cubic(i as f32 / 10.0);
            assert!(v >= prev);
            prev = v;
        }
        assert_eq!(lerp(2.0, 6.0, 0.0), 2.0);
        assert_eq!(lerp(2.0, 6.0, 1.0), 6.0);
    }

    #[test]
    fn hit_test_regions() {
        let r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
        let (tx, ty, tw, th) = TARGET_HIT;
        assert!(tx >= 14 && tx + tw <= LOGICAL_W - 14);
        assert!(ty >= 82 && ty + th <= 178);
        assert_eq!(r.hit_test(tx, ty), Some(Hit::Target));
        assert_eq!(r.hit_test(tx + tw - 1, ty + th - 1), Some(Hit::Target));
        assert_eq!(r.hit_test(tx - 1, ty), None);
        assert_eq!(r.hit_test(tx, ty + th), None);
        assert_eq!(r.hit_test(tx + tw, ty), None);
        assert_eq!(r.hit_test(LOGICAL_W - 1, 0), Some(Hit::Close));
        assert_eq!(r.hit_test(324, 35), Some(Hit::Close));
        assert_eq!(r.hit_test(323, 35), Some(Hit::Log));
        assert_eq!(r.hit_test(292, 0), Some(Hit::Log));
        assert_eq!(r.hit_test(291, 0), Some(Hit::Pin));
        assert_eq!(r.hit_test(260, 35), Some(Hit::Pin));
        assert_eq!(r.hit_test(259, 0), None);
        let (px_, py_, pw_, ph_) = PIN_BTN;
        let (hx_, hy_, hw_, hh_) = PIN_HIT;
        assert!(px_ >= hx_ && py_ >= hy_ && px_ + pw_ <= hx_ + hw_ && py_ + ph_ <= hy_ + hh_);
        assert_eq!(hx_ + hw_, LOG_HIT.0);
        assert_eq!(r.hit_test(323, 36), Some(Hit::Info(Info::Status)));
        assert_eq!(r.hit_test(210, LOGICAL_H - 32), Some(Hit::Update));
        assert_eq!(r.hit_test(209, LOGICAL_H - 1), None);
        for (rect, hit) in [
            (RUN_PREV_HIT, Hit::RunPrev),
            (RUN_NEXT_HIT, Hit::RunNext),
            (FIGHT_PREV_HIT, Hit::FightPrev),
            (FIGHT_NEXT_HIT, Hit::FightNext),
            (PHASE_HIT, Hit::Phase),
        ] {
            let (x, y, w, h) = rect;
            assert_eq!(r.hit_test_where(x, y, |h| h == hit), Some(hit));
            assert_eq!(
                r.hit_test_where(x + w - 1, y + h - 1, |h| h == hit),
                Some(hit)
            );
            assert_ne!(r.hit_test(x + w, y + h), Some(hit));
        }
        let (x, y, w, h) = PHASE_HIT;
        assert!(x >= 14 && x + w <= 346);
        assert!(y >= 82 && y + h <= 178);
        assert_eq!(r.hit_test(x + w - 1, y + h - 1), Some(Hit::Target));
        assert_eq!(
            r.hit_test_where(x + w - 1, y + h - 1, |h| h != Hit::Target),
            Some(Hit::Phase)
        );
    }

    #[test]
    fn log_hit_test_regions() {
        let r = Renderer::new(96, LOG_W, LOG_H);
        assert_eq!(r.log_hit_test(LOG_W - 1, 0, None), Some(LogHit::Close));
        assert_eq!(r.log_hit_test(379, 10, None), None);
        for (filter, _, x, w) in LOG_TABS {
            assert_eq!(
                r.log_hit_test(x, LOG_TAB_Y, None),
                Some(LogHit::Tab(filter))
            );
            assert_eq!(
                r.log_hit_test(x + w - 1, LOG_TAB_Y + LOG_TAB_H - 1, None),
                Some(LogHit::Tab(filter))
            );
        }
        assert_eq!(r.log_hit_test(20, LOG_TAB_Y + 2, None), Some(LogHit::Title));
        assert_eq!(
            r.log_hit_test(100, LOG_TAB_Y + 2, None),
            Some(LogHit::Count)
        );
        assert!(!LogHit::Count.clickable());
        assert!(LogHit::Close.clickable());
        let (tx, ty, _, th) = LOG_TRACK;
        assert_eq!(r.log_hit_test(tx, ty, None), None);
        assert_eq!(
            r.log_hit_test(tx, ty + 10, Some((0, 40))),
            Some(LogHit::Thumb)
        );
        assert_eq!(
            r.log_hit_test(tx, ty + 40, Some((0, 40))),
            Some(LogHit::Track)
        );
        assert_eq!(r.log_hit_test(tx, ty + th, Some((0, 40))), None);
        let (bx, by, bw, bh) = LOG_BODY;
        assert!(bx + bw <= tx);
        assert_eq!(by + bh, LOG_H - 10);
    }

    #[test]
    fn mix_endpoints() {
        assert_eq!(mix(ACCENT, TEXT, 0.0), ACCENT);
        assert_eq!(mix(ACCENT, TEXT, 1.0), TEXT);
    }

    #[test]
    fn draw_smoke_bars() {
        const P: &str = "2026.09.07 09:12:28 Debug      -  ";
        let mut gs = GameState::default();
        gs.feed(&format!(
            "{P}ECLIPTICA - now in stage: Stage_Test on phase: 0.5 as class: Blade"
        ));
        gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.5"
        ));
        gs.feed(&format!(
            "{P}ECLIPTICA - now fighting boss: KakarotPhase2(Clone) on phase: 0.5"
        ));
        gs.feed(&format!(
            "{P}damage has been taken: 12, from source: (Kakarot) attack_Kick"
        ));
        gs.feed(&format!("{P}damage has been taken: 3, from source: "));
        gs.feed(&format!("{P}Local controller dead, switching off."));
        let mut r = Renderer::new(96, LOGICAL_W, LOGICAL_H);
        let f = frame();
        r.draw_main(&mut gs, &f);
        assert_eq!(pix(&r, 0, 0), BG);
        assert_eq!(pix(&r, 100, 49), GOOD);
        assert_eq!(pix(&r, 300, 49), CARD);
        assert_eq!(pix(&r, 100, 234), ACCENT);
        assert_eq!(pix(&r, 20, 90), CARD);
        assert_eq!(pix(&r, 336, 11), CARD_HI);
        assert_eq!(pix(&r, 20, 300), CARD);
        assert_eq!(pix(&r, 100, 397), DANGER);
        assert_eq!(pix(&r, 300, 397), BG);
        assert_eq!(pix(&r, 30, 449), DANGER);

        let flashing = Frame {
            taken_flash_t: 0.0,
            hover: Some(Hit::Log),
            pressed: Some(Hit::RunPrev),
            ..frame()
        };
        r.draw_main(&mut gs, &flashing);
        assert_ne!(pix(&r, 20, 300), CARD);
        assert_eq!(pix(&r, 304, 11), CARD_HI);

        let hist = Frame {
            run_sel: true,
            group_sel: true,
            view_phase: Some(0),
            ..frame()
        };
        r.draw_main(&mut gs, &hist);
        assert_eq!(pix(&r, 300, 397), CARD);
        assert_eq!(pix(&r, 30, 419), CARD);
        let group = Frame {
            run_sel: true,
            group_sel: true,
            view_phase: None,
            ..frame()
        };
        r.draw_main(&mut gs, &group);
        assert_eq!(pix(&r, 30, 419), DANGER);

        let tipped = Frame {
            tip: Some(Hit::Log),
            ..frame()
        };
        r.draw_main(&mut gs, &tipped);
        assert_eq!(pix(&r, 300, 45), CARD_HI);
        assert!(!tip_ready(None));
        assert!(!tip_ready(Some(std::time::Instant::now())));
        for (info, (x, y, w, h)) in INFO_REGIONS {
            let hit = Hit::Info(info);
            assert!(x >= 14 && x + w <= LOGICAL_W - 14, "{info:?}");
            assert!(y + h <= LOGICAL_H, "{info:?}");
            let only = |h: Hit| h == hit;
            assert_eq!(r.hit_test_where(x, y, only), Some(hit), "{info:?}");
            assert_eq!(
                r.hit_test_where(x + w - 1, y + h - 1, only),
                Some(hit),
                "{info:?}"
            );
            assert!(!hit.clickable());
            for live in [true, false] {
                let tip = info.tip(live);
                let unused = (info.live_only() && !live) || (info.history_only() && live);
                assert!(unused || !tip.is_empty(), "{info:?} live={live}");
                assert!(tip.chars().count() <= 70, "{info:?} live={live}");
            }
        }
        for (a, b) in INFO_REGIONS
            .iter()
            .flat_map(|a| INFO_REGIONS.iter().map(move |b| (a, b)))
        {
            if a.0 == b.0 {
                continue;
            }
            let both_live = !a.0.history_only() && !b.0.history_only();
            let both_hist = !a.0.live_only() && !b.0.live_only();
            let (ax, ay, aw, ah) = a.1;
            let (bx, by, bw, bh) = b.1;
            let overlap = ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah;
            assert!(
                !(overlap && (both_live || both_hist)),
                "{:?} overlaps {:?}",
                a.0,
                b.0
            );
        }
        assert!(Hit::Log.clickable());
        assert_eq!(r.hit_test(200, 30), Some(Hit::Info(Info::Status)));
        assert_eq!(r.hit_test(200, 250), Some(Hit::Info(Info::LastKill)));
        assert_eq!(r.hit_test(30, 200), Some(Hit::Info(Info::Dealt(0))));
        assert_eq!(r.hit_test(300, 370), Some(Hit::Info(Info::TakenMore(2))));
        let ctx = TipCtx {
            live: true,
            topmost: false,
            sound_on: false,
            log_open: true,
        };
        assert!(Hit::Pin.tip(ctx).contains("keep it on top"));
        assert!(Hit::Target.tip(ctx).contains("unmute"));
        assert!(Hit::Log.tip(ctx).starts_with("Close"));
        assert_eq!(
            r.hit_test_where(340, LOGICAL_H - 20, |h| h != Hit::Update),
            Some(Hit::Info(Info::Version))
        );
    }

    #[test]
    fn draw_log_smoke() {
        let mut gs = GameState::default();
        let mut r = Renderer::new(96, LOG_W, LOG_H);
        let mut lv = LogView {
            scroll: 0.0,
            filter: Filter::All,
            hover: None,
            dragging: false,
            thumb_t: 0.0,
            slide: 0.0,
            tip: None,
            thumb: None,
        };
        r.draw_log(&gs, &lv);
        assert_eq!(pix(&r, 0, 0), BG);
        let (_, _, ax, aw) = LOG_TABS[0];
        assert_eq!(pix(&r, ax + aw / 2, LOG_TAB_Y + 2), ACCENT);
        let (tx, ty, _, _) = LOG_TRACK;
        assert_eq!(pix(&r, tx + 3, ty + 3), BG);
        for i in 0..40 {
            gs.feed(&format!(
                "2026.09.07 09:12:{:02} Debug      -  damage has been taken: {i}, from source: attack_Spit",
                i % 60
            ));
        }
        gs.feed("2026.09.07 09:13:00 Debug      -  ECLIPTICA - now fighting boss: Yuki(Clone) on phase: 0");
        gs.feed("2026.09.07 09:13:01 Debug      -  ownership of Yuki transferred to Alice");
        lv.filter = Filter::Targets;
        r.draw_log(&gs, &lv);
        assert_eq!(pix(&r, tx + 3, ty + 3), BG);
        let (bx, by, _, _) = LOG_BODY;
        assert_eq!(pix(&r, bx + 1, by + 12), ACCENT);
        lv.filter = Filter::All;
        lv.scroll = log::max_scroll(42, r.body_h());
        r.draw_log(&gs, &lv);
        assert_eq!(pix(&r, tx + 3, ty + 3), CARD);
        assert_eq!(pix(&r, tx + 3, LOG_H - 12), CARD_HI);
        assert_eq!(pix(&r, bx + 1, by + 12), DANGER);
    }
}
