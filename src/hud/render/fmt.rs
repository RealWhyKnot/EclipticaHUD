use crate::game::event::fmt_clock;

pub(super) fn fmt_anim(v: f32) -> String {
    (v.max(0.0).round() as u64).to_string()
}

pub(super) fn fmt_ago(now: u64, ts: u64) -> String {
    let d = now.saturating_sub(ts);
    if d < 60 {
        format!("{d}s ago")
    } else {
        fmt_clock(ts)
    }
}

pub(super) fn fmt_run_deaths(n: u32) -> String {
    format!("{n} run death{}", if n == 1 { "" } else { "s" })
}

pub(super) fn fmt_dur(secs: u64) -> String {
    if secs >= 60 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

pub(super) fn group_digits(n: u64) -> String {
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
