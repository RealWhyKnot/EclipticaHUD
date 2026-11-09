use super::lines::{float, world};
use super::*;
use crate::history::{PartyRecord, SavedRow};

mod batches;
mod opt_in;
mod text;

fn base() -> u64 {
    crate::game::event::split_line("2026.10.23 10:00:00 Log        -  x")
        .unwrap()
        .ts
}

fn kept(msgs: &[&str], from: u64) -> Vec<(u64, String)> {
    msgs.iter()
        .enumerate()
        .map(|(i, m)| (from + crate::num::to_u64(i), m.to_string()))
        .collect()
}

fn have(db: &History, name: &str) -> u64 {
    db.log_ends()
        .into_iter()
        .find(|(n, _)| n == name)
        .map_or(0, |(_, e)| e)
}

fn add(db: &mut History, name: &str, msgs: &[&str]) {
    let have = have(db, name);
    let lines = kept(msgs, base() + have);
    let end = lines.last().map_or(0, |l| l.0);
    db.archive(name, have, &lines, end);
}

fn add_at(db: &mut History, name: &str, ts: u64, msgs: &[&str]) {
    let have = have(db, name);
    let lines: Vec<(u64, String)> = msgs.iter().map(|m| (ts, m.to_string())).collect();
    db.archive(name, have, &lines, ts);
}

const LOG: &str = "output_log_2026-10-23_10-00-00.txt";
const LATER: &str = "output_log_2026-10-23_12-00-00.txt";
const ENTER: &str = "[Behaviour] Entering Room: Ecliptica";

const SESSION: &[&str] = &[
    "User Authenticated: WhyKnot (usr_11111111-2222-3333-4444-555555555555)",
    ENTER,
    "[Behaviour] Joining wrld_4cf01f21-5c2a-4c7b-8a0a-0123456789ab:98765~private(usr_abc)~region(us)",
    "[Behaviour] OnPlayerJoined WhyKnot (usr_11111111-2222-3333-4444-555555555555)",
    "[Behaviour] OnPlayerJoined Lena (usr_aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee)",
    "[Behaviour] OnPlayerJoined Al (usr_ffffffff-bbbb-cccc-dddd-eeeeeeeeeeee)",
    "ownership of AudioLink transferred to WhyKnot",
    "ownership of BossSync transferred to Lena",
    "ECLIPTICA - now in stage: Stage_GMBigcity on phase: 0.5 as class: Thaumaturge",
    "ECLIPTICA - now fighting boss: Hakli(Clone) (UnityEngine.GameObject)",
    "Dealing 25 STRIKE damage",
    "damage has been taken: 12, from source: (Khepri) attack_Claws2",
    "damage has been taken: 3, from source: Lena's Fireball",
    "damage has been taken: 4, from source: Al",
    "damage has been taken: 2, from source: ",
    "Initializing Enemy POOL ID3 as ENEMY ID 17",
    "[Behaviour] No targets to encode on Kodama(Clone)",
    "Retiring Enemy POOL ID3",
    "spawn token, True, 40",
    "Boss Hakli dead, personal damage dealt: ",
    "STRIKE DMG: 5239",
    "NON-STRIKE DMG: 120",
    "[Behaviour] OnPlayerLeft Lena (usr_aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee)",
    "Local controller dead, switching off.",
    "ECLIPTICA - now in intermission",
    "ECLIPTICA - now in lobby",
    "ECLIPTICA saving SESSION ID 4242",
    "[Behaviour] OnLeftRoom",
];

const WANT: &str = "\
0\tlog\th\t0
1\troom_enter\tEcliptica
1\tworld\twrld_4cf01f21-5c2a-4c7b-8a0a-0123456789ab\t98765
1\tplayer_join\tyou
0\tplayers\t1
1\tplayer_join\tP1
0\tplayers\t2
1\tplayer_join\tP2
0\tplayers\t3
1\townership\tAudioLink\tyou
1\townership\tBossSync\tP1
1\tstage\tGMBigcity\t0.5\tThaumaturge
1\tboss_fight\tHakli\t
1\tdealt\t25\t1
1\tdamage_taken\t12\t(Khepri) attack_Claws2
1\tdamage_taken\t3\tP1's Fireball
1\tdamage_taken\t4\tP2
1\tdamage_taken\t2\t
1\tenemy_spawn\t3\t17
1\tenemy_name\tKodama
1\tenemy_retire\t3
1\ttoken_spawn\t1\t40
1\tboss_dead\tHakli
1\tstrike_total\t5239
1\tnon_strike_total\t120
1\tplayer_left\tP1
0\tplayers\t2
1\tplayer_dead
1\tintermission
1\tstage_end\tGMBigcity\t1\t0\t1\t0\t0\t0\t0\t0
0\tfight_end\tHakli\t1\tkilled\t1\t10\t25\t21\t4\t12\t0\t5239\t120
0\tfight_attack\tHakli\tKhepri\tClaws 2\t12\t1
0\tfight_attack\tHakli\tenemy\tP1's Fireball\t3\t1
0\tfight_attack\tHakli\tenemy\tP2\t4\t1
0\tfight_attack\tHakli\tunknown\tno source\t2\t1
0\trun_end\tlobby\t17\t1\t1\t1\t1\t0\tThaumaturge\tGMBigcity\t25\t21\t4\t12
0\tlobby
1\tsession_save\t4242
1\troom_left
";

fn fresh() -> History {
    let mut db = History::in_memory().unwrap();
    db.usage_start();
    db
}

fn masked(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let mut f: Vec<&str> = line.split('\t').collect();
        if f.get(1) == Some(&"log") {
            assert_eq!(f[2].len(), 16, "{line}");
            assert!(f[2].bytes().all(|b| b.is_ascii_hexdigit()), "{line}");
            f[2] = "h";
        }
        out.push_str(&f.join("\t"));
        out.push('\n');
    }
    out
}

fn body(text: &str) -> String {
    masked(text.split_once('\n').map_or("", |(_, rest)| rest))
}

fn tails(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.split_once('\t').unwrap().1.to_string())
        .collect()
}

const RUN: &[&str] = &[
    ENTER,
    "ECLIPTICA - now in stage: Stage_GMBigcity on phase: 0.5 as class: Thaumaturge",
    "ECLIPTICA - now fighting boss: Hakli(Clone) on phase: 0.25",
    "Dealing 40 STRIKE damage",
];

fn party(at: u64, rows: &[(&str, bool, u64, u64)]) -> PartyRecord {
    PartyRecord {
        at,
        boss: "Hakli".into(),
        progress: 0.3,
        secs: 194,
        down: true,
        here: 7,
        rows: rows
            .iter()
            .map(|&(name, you, dealt, taken)| SavedRow {
                name: name.into(),
                you,
                dealt,
                taken,
            })
            .collect(),
    }
}
