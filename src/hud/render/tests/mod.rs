use super::hit::{INFO_REGIONS, TipCtx};
use super::layout::*;
use super::theme::*;
use super::*;
use crate::discord::Link;
use crate::game::state::GameState;
use crate::hud::flags::{Flag, Flags};
use crate::hud::timeline::{self, Filter};
use crate::update::Badge;
use crate::vr::VrStatus;
use windows_sys::Win32::Graphics::Gdi::*;

pub(super) fn frame() -> Frame {
    Frame {
        now: 0,
        env: Env::InWorld,
        pages: 1,
        view_page: 0,
        flags: Flags::default()
            .with(Flag::SettingsOpen, false)
            .with(Flag::LogOk, true)
            .with(Flag::LogOpen, false)
            .with(Flag::Topmost, true)
            .with(Flag::PromptOpen, false)
            .with(Flag::Sound, true)
            .with(Flag::Discord, false)
            .with(Flag::Usage, false)
            .with(Flag::UsageFailing, false)
            .with(Flag::Board, false)
            .with(Flag::UsageAsk, false)
            .with(Flag::PartyOpen, false)
            .with(Flag::RunSelected, false)
            .with(Flag::GroupSelected, false)
            .with(Flag::StagesButton, false)
            .with(Flag::Stages, false),
        scale: 1.0,
        scale_max: MAX_SCALE,
        alpha: 100,
        window: 10,
        flash_t: 1.0,
        taken_flash_t: 1.0,
        warn_t: 1.0,
        dead_pulse: 0.0,
        hover: Some(Hit::Close),
        pressed: None,
        tip: None,
        vr: VrStatus::Off,

        progress_shown: 0.5,
        dps_shown: 0.0,
        fight_dps_shown: 0.0,
        taken_shown: 0.0,
        taken_rate_shown: 0.0,
        update: Badge::None,

        discord: Link::Off,

        board_status: 0,
        board_delete: 0,

        view_run: Some(0),
        view_group: Some(0),
        group_pages: 1,

        split_top: 0,
        sync: RunSync::Idle,
    }
}

pub(super) fn pix(renderer: &Renderer, x: i32, y: i32) -> u32 {
    let bytes =
        // SAFETY: `bits` points at the DIB section's width * height * 4 bytes, which live as long as the renderer.
        unsafe { std::slice::from_raw_parts(renderer.bits, crate::num::to_usize(renderer.width * renderer.height * 4)) };
    let index = crate::num::to_usize((y * renderer.width + x) * 4);
    rgb(
        u32::from(bytes[index + 2]),
        u32::from(bytes[index + 1]),
        u32::from(bytes[index]),
    )
}

fn text_w(r: &Renderer, font: usize, s: &str) -> i32 {
    let buf: Vec<u16> = s.encode_utf16().collect();
    let mut size = windows_sys::Win32::Foundation::SIZE { cx: 0, cy: 0 };
    // SAFETY: the DC and font belong to the live renderer, and the pointer and length describe `buf`.
    unsafe {
        SelectObject(r.dc, r.fonts[font].cast());
        GetTextExtentPoint32W(
            r.dc,
            buf.as_ptr(),
            crate::num::to_i32(buf.len()),
            &raw mut size,
        );
    }
    size.cx
}

fn settings_hit(h: Hit) -> bool {
    matches!(
        h,
        Hit::ScaleDown
            | Hit::ScaleUp
            | Hit::AlphaDown
            | Hit::AlphaUp
            | Hit::WindowDown
            | Hit::WindowUp
            | Hit::Sound
            | Hit::Discord
            | Hit::Usage
    )
}

fn smoke_gs() -> GameState {
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
    gs
}

fn tip_ctx() -> TipCtx {
    TipCtx {
        flags: Flags::default()
            .with(Flag::Live, true)
            .with(Flag::Topmost, false)
            .with(Flag::Sound, false)
            .with(Flag::LogOpen, true)
            .with(Flag::Discord, false)
            .with(Flag::Usage, false)
            .with(Flag::UsageFailing, false)
            .with(Flag::Board, false)
            .with(Flag::PartyOpen, false)
            .with(Flag::Stages, false),

        board_status: 0,
        board_delete: 0,
    }
}

fn fight_log(secs: u64, me_targeted: bool) -> (GameState, u64) {
    let line = |t: u64, m: &str| {
        format!(
            "2026.09.07 {:02}:{:02}:{:02} Debug      -  {m}",
            9 + t / 3600,
            t / 60 % 60,
            t % 60
        )
    };
    let mut gs = GameState::default();
    gs.feed(&line(0, "User Authenticated: Me (Alt) (usr_1)"));
    gs.feed(&line(
        0,
        "ECLIPTICA - now in stage: Stage_Test on phase: 0.5 as class: Blade",
    ));
    gs.feed(&line(
        1,
        "ECLIPTICA - now fighting boss: Kakarot(Clone) on phase: 0.5",
    ));
    for t in 2..secs {
        gs.feed(&line(t, &format!("Dealing {} STRIKE damage", t * 10)));
    }
    let who = if me_targeted { "Me (Alt)" } else { "Someone" };
    gs.feed(&line(
        secs,
        &format!("ownership of Kakarot transferred to {who}"),
    ));
    let now = crate::game::event::split_line(&line(secs, "x")).unwrap().ts;
    (gs, now)
}

fn prompt_frame(update: Badge) -> Frame {
    Frame {
        update,
        flags: frame().flags.with(Flag::PromptOpen, true),
        hover: None,
        ..frame()
    }
}

fn main_frame() -> Frame {
    Frame {
        view_run: None,
        view_group: None,
        ..frame()
    }
}

pub(super) fn party_row(
    name: &str,
    you: bool,
    dmg: u64,
    taken: u64,
    share: Option<u32>,
    dps: u64,
) -> PartyRow {
    PartyRow {
        name: name.to_string(),
        you,
        live: true,
        dmg,
        taken,
        life: 0,
        share,
        dps,
    }
}

pub(super) fn log_view(filter: Filter) -> LogView {
    LogView {
        scroll: 0.0,
        filter,
        hover: None,
        dragging: false,
        thumb_t: 0.0,
        slide: 0.0,
        tip: None,
        thumb: None,
    }
}

pub(super) fn pane(rows: Vec<PartyRow>) -> PartyPane {
    PartyPane {
        on: true,
        ready: true,
        link: crate::relay::Link::Connected,
        here: rows.len() + 2,
        fight: Some(PartyFight {
            name: "Ha'kli".to_string(),
            secs: 83,
            live: true,
            down: false,
        }),
        rows,
        ..PartyPane::default()
    }
}

pub(super) fn sample_party() -> Vec<PartyRow> {
    vec![
        party_row(
            "Be\u{430}r (Hands)",
            false,
            1_284_310,
            4_120,
            Some(41),
            18_420,
        ),
        party_row("Me (Alt)", true, 902_115, 18_904, Some(29), 12_007),
        party_row("Mira", false, 610_002, 655, Some(19), 9_880),
        PartyRow {
            live: false,
            ..party_row(
                "\u{6c17}\u{307e}\u{3050}\u{308c} Sora",
                false,
                230_440,
                31_377,
                Some(7),
                3_120,
            )
        },
        party_row("Tobias_77", false, 118_900, 0, Some(4), 2_500),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Stress {
    Short,
    Names,
    Huge,
}

pub(super) fn stress_party(stress: Stress) -> Vec<PartyRow> {
    let scripts = ["W", "\u{6c17}", "\u{430}", "\u{1d04}", "m", "\u{5d0}"];
    (0..MAX_MEMBERS)
        .map(|i| {
            let name = if stress == Stress::Short {
                format!("P{i}")
            } else {
                scripts[i % scripts.len()].repeat(400)
            };
            let (dmg, dps) = if stress == Stress::Huge {
                (
                    MAX_DMG - crate::num::to_u64(i),
                    u64::MAX - crate::num::to_u64(i),
                )
            } else {
                (5_000 - crate::num::to_u64(i), 120)
            };
            PartyRow {
                live: i % 5 != 4,
                ..party_row(&name, i == 3, dmg, dmg, Some(100), dps)
            }
        })
        .collect()
}

use crate::relay::party::PartyRow;
use crate::relay::proto::{MAX_DMG, MAX_MEMBERS};

mod details;
mod pages;
mod smoke;

fn tip_fits(r: &Renderer, tip: &str, win_w: i32) -> (i32, usize) {
    let (w, lines) = r.tip_lines(tip, win_w);
    assert!(w <= win_w - 16, "{tip}: {w}");
    assert!(lines.len() <= 2, "{tip}");
    assert_eq!(lines.join(" "), tip);
    for line in &lines {
        let lw = text_w(r, F_TINY, line);
        assert!(
            lw + r.px(TIP_INSET) <= r.px(w),
            "{line:?} is {lw} px in a {} px tip",
            r.px(w)
        );
    }
    (w, lines.len())
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

pub(super) fn png(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    let mut raw =
        Vec::with_capacity(crate::num::to_usize(width * 3 + 1) * crate::num::to_usize(height));
    for row in rgb.chunks(crate::num::to_usize(width) * 3) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut stream = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (index, b) in blocks.iter().enumerate() {
        stream.push(u8::from(index + 1 == blocks.len()));
        stream.extend_from_slice(&crate::num::to_u16(b.len()).to_le_bytes());
        stream.extend_from_slice(&(!crate::num::to_u16(b.len())).to_le_bytes());
        stream.extend_from_slice(b);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in &raw {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    stream.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8], data: &[u8]| {
        out.extend_from_slice(&crate::num::to_u32(data.len()).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &stream);
    chunk(b"IEND", &[]);
    out
}

struct Sheet {
    w: u32,
    h: u32,
    rgb: Vec<u8>,
}

impl Sheet {
    fn new(w: u32, h: u32) -> Self {
        let mut rgb = Vec::with_capacity(crate::num::to_usize(w * h * 3));
        for _ in 0..w * h {
            rgb.extend_from_slice(&[0x0b, 0x0b, 0x10]);
        }
        Sheet { w, h, rgb }
    }

    fn put(&mut self, r: &Renderer, x0: u32, y0: u32, clip: (i32, i32, i32, i32)) {
        let (cx, cy, cw, ch) = (r.px(clip.0), r.px(clip.1), r.px(clip.2), r.px(clip.3));
        for y in 0..ch {
            for x in 0..cw {
                let c = pix(r, cx + x, cy + y);
                let i = crate::num::to_usize(
                    ((y0 + y.cast_unsigned()) * self.w + x0 + x.cast_unsigned()) * 3,
                );
                self.rgb[i] = crate::num::to_u8(c & 0xff);
                self.rgb[i + 1] = crate::num::to_u8((c >> 8) & 0xff);
                self.rgb[i + 2] = crate::num::to_u8((c >> 16) & 0xff);
            }
        }
    }

    fn save(&self, name: &str) {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".devflow")
            .join("pick");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), png(self.w, self.h, &self.rgb)).unwrap();
    }
}

fn boss_fight(boss: &str) -> (GameState, u64) {
    let line = |t: u64, m: &str| format!("2026.09.07 09:00:{t:02} Debug      -  {m}");
    let mut gs = GameState::default();
    gs.feed(&line(0, "User Authenticated: Me (Alt) (usr_1)"));
    gs.feed(&line(
        0,
        "ECLIPTICA - now in stage: Stage_Test on phase: 0.5 as class: Blade",
    ));
    gs.feed(&line(
        1,
        &format!("ECLIPTICA - now fighting boss: {boss}(Clone) on phase: 0.5"),
    ));
    for t in 2..20 {
        gs.feed(&line(t, "Dealing 50 STRIKE damage"));
    }
    gs.feed(&line(
        20,
        &format!("ownership of {boss} transferred to Me (Alt)"),
    ));
    let now = crate::game::event::split_line(&line(20, "x")).unwrap().ts;
    (gs, now)
}

fn pixels(r: &Renderer) -> Vec<u32> {
    (0..r.height)
        .flat_map(|y| (0..r.width).map(move |x| (x, y)))
        .map(|(x, y)| pix(r, x, y))
        .collect()
}

const LONG_BOSSES: [&str; 4] = [
    "OnyxPilotOfTheVeryLongAndUnreasonablyNamedEclipseVanguard",
    "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW",
    "Mk2GolemMk2GolemMk2GolemMk2GolemMk2GolemMk2GolemMk2GolemMk2Golem",
    "東京BossÉclairBêteΩmegaLord東京BossÉclairBêteΩmegaLord東京Boss",
];

fn caught_up() -> GameState {
    let (peer, mut gs) = late_party();
    assert_eq!(gs.catch_up(&peer.summary().unwrap()), Some(true));
    gs
}

fn late_party() -> (GameState, GameState) {
    let line = |gs: &mut GameState, t: &str, m: &str| {
        gs.feed(&format!("2026.10.26 {t} Debug      -  {m}"));
    };
    let mut peer = GameState::default();
    line(
        &mut peer,
        "15:40:00",
        "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer",
    );
    line(&mut peer, "15:41:00", "Dealing 4000 STRIKE damage");
    line(
        &mut peer,
        "15:44:00",
        "ECLIPTICA - now fighting boss: Nan(Clone) on phase: 0",
    );
    line(&mut peer, "15:45:00", "Dealing 9000 STRIKE damage");
    line(
        &mut peer,
        "15:47:30",
        "Boss Nan dead, personal damage dealt: ",
    );
    line(&mut peer, "15:47:30", "STRIKE DMG: 9000");
    line(&mut peer, "15:47:30", "NON-STRIKE DMG: 0");
    line(&mut peer, "15:47:40", "ECLIPTICA - now in intermission");
    line(
        &mut peer,
        "15:52:00",
        "ECLIPTICA - now in stage: Stage_Hallow on phase: 0.1 as class: Spellhammer",
    );
    line(
        &mut peer,
        "16:01:00",
        "ECLIPTICA - now fighting boss: HaKli(Clone) on phase: 0",
    );
    let mut gs = GameState::default();
    line(
        &mut gs,
        "16:01:04",
        "ECLIPTICA - now fighting boss: HaKli(Clone) on phase: 0",
    );
    for s in 5..60 {
        line(
            &mut gs,
            &format!("16:01:{s:02}"),
            "Dealing 700 STRIKE damage",
        );
    }
    line(
        &mut gs,
        "16:02:00",
        "Dealing 300 damage to you from HaKli Slam",
    );
    (peer, gs)
}

fn missed_frames(gs: &GameState) -> [Frame; 2] {
    let now = crate::game::event::split_line("2026.10.26 16:02:00 Debug      -  x")
        .unwrap()
        .ts;
    let pages = gs.runs[0].pages().len();
    [
        Frame {
            now,
            flags: frame()
                .flags
                .with(Flag::RunSelected, true)
                .with(Flag::GroupSelected, true),

            view_run: Some(0),
            view_group: Some(0),
            group_pages: pages,
            pages: 1,
            ..frame()
        },
        Frame {
            now,
            flags: frame()
                .flags
                .with(Flag::StagesButton, true)
                .with(Flag::Stages, true),

            ..frame()
        },
    ]
}

fn sync_frames() -> [(&'static str, GameState, RunSync); 3] {
    let (_, waiting) = late_party();
    [
        ("joined late, syncing", waiting, RunSync::Waiting),
        ("synced", caught_up(), RunSync::Done),
        ("synced, later", caught_up(), RunSync::Idle),
    ]
}

fn sync_now() -> u64 {
    crate::game::event::split_line("2026.10.26 16:02:00 Debug      -  x")
        .unwrap()
        .ts
}
