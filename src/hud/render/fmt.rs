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

pub(super) fn fmt_count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
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

pub(super) const UNITS: [&str; 6] = ["K", "M", "B", "T", "Qa", "Qi"];

pub(super) fn compact(n: u64) -> String {
    if n < 1000 {
        return n.to_string();
    }
    let mut drop = n.ilog10() - 2;
    let p = 10u128.pow(drop);
    let mut m = (n as u128 + p / 2) / p;
    if m >= 1000 {
        m /= 10;
        drop += 1;
    }
    let k = drop.div_ceil(3);
    let digits = m.to_string();
    let (int, frac) = digits.split_at((drop + 3 - 3 * k) as usize);
    let dot = if frac.is_empty() { "" } else { "." };
    format!("{int}{dot}{frac}{}", UNITS[k as usize - 1])
}
