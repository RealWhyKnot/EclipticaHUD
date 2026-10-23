use super::proto::{Member, MAX_MEMBERS};
use std::collections::VecDeque;

pub const DPS_WINDOW_MS: u64 = 10_000;
const MAX_SAMPLES: usize = 64;
const MAX_LABELS: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRow {
    pub name: String,
    pub you: bool,
    pub live: bool,
    pub dmg: u64,
    pub share: Option<u32>,
    pub dps: u64,
}

#[derive(Debug)]
struct Row {
    hash: Member,
    you: bool,
    live: bool,
    dmg: u64,
    frozen_dps: u64,
    frozen_at: u64,
    samples: VecDeque<(u64, u64)>,
}

impl Row {
    fn new(hash: Member, you: bool, now: u64, dmg: u64) -> Self {
        Row {
            hash,
            you,
            live: true,
            dmg,
            frozen_dps: 0,
            frozen_at: 0,
            samples: VecDeque::from([(now, dmg)]),
        }
    }

    fn update(&mut self, now: u64, dmg: u64) {
        if !self.live || dmg < self.dmg {
            self.samples.clear();
            self.samples.push_back((now, dmg));
        } else if dmg != self.dmg {
            self.samples.push_back((now, dmg));
        }
        self.live = true;
        self.dmg = dmg;
        self.prune(now);
    }

    fn prune(&mut self, now: u64) {
        while self.samples.len() > 1 && self.samples[1].0 + DPS_WINDOW_MS <= now {
            self.samples.pop_front();
        }
        while self.samples.len() > MAX_SAMPLES {
            self.samples.pop_front();
        }
    }

    fn freeze(&mut self, now: u64) {
        if self.live {
            let last = self.samples.back().map_or(now, |s| s.0).min(now);
            self.frozen_dps = self.live_dps(last);
            self.frozen_at = now;
            self.live = false;
        }
    }

    fn live_dps(&self, now: u64) -> u64 {
        let Some(&(t0, d0)) = self.samples.front() else {
            return 0;
        };
        let (base, span) = match self
            .samples
            .iter()
            .rev()
            .find(|s| s.0 + DPS_WINDOW_MS <= now)
        {
            Some(&(_, d)) => (d, DPS_WINDOW_MS),
            None => (d0, now.saturating_sub(t0).clamp(1000, DPS_WINDOW_MS)),
        };
        self.dmg.saturating_sub(base) * 1000 / span
    }

    fn dps(&self, now: u64) -> u64 {
        if self.live {
            self.live_dps(now)
        } else {
            self.frozen_dps
        }
    }
}

#[derive(Default, Debug)]
pub struct Party {
    rows: Vec<Row>,
    labels: Vec<(Member, u32)>,
    names: Vec<(Member, String)>,
    gone: Vec<(Member, String)>,
}

impl Party {
    pub fn reset_room(&mut self) {
        self.rows.clear();
        self.labels.clear();
        self.names.clear();
        self.gone.clear();
    }

    pub fn clear(&mut self) {
        self.rows.clear();
        self.gone.clear();
    }

    pub fn set_names(&mut self, names: Vec<(Member, String)>) {
        let old = std::mem::replace(&mut self.names, names);
        let keep = |h: &Member, names: &[(Member, String)], rows: &[Row]| {
            !names.iter().any(|(n, _)| n == h) && rows.iter().any(|r| !r.you && r.hash == *h)
        };
        self.gone.retain(|(h, _)| keep(h, &self.names, &self.rows));
        for (h, name) in old {
            if keep(&h, &self.names, &self.rows) && !self.gone.iter().any(|(g, _)| *g == h) {
                self.gone.push((h, name));
            }
        }
    }

    fn name(&self, hash: Member) -> Option<&String> {
        self.names
            .iter()
            .chain(self.gone.iter())
            .find(|(h, _)| *h == hash)
            .map(|(_, n)| n)
    }

    pub fn others(&self) -> usize {
        self.rows.iter().filter(|r| !r.you).count()
    }

    pub fn apply_snap(&mut self, now: u64, entries: &[(Member, u64)], own: Option<Member>) {
        for row in self.rows.iter_mut().filter(|r| !r.you) {
            if !entries.iter().any(|(h, _)| *h == row.hash) {
                row.freeze(now);
            }
        }
        for &(hash, dmg) in entries {
            if Some(hash) == own {
                continue;
            }
            match self.rows.iter_mut().find(|r| !r.you && r.hash == hash) {
                Some(row) => row.update(now, dmg),
                None => {
                    if self.others() >= MAX_MEMBERS {
                        let Some(i) = self.oldest_frozen() else {
                            continue;
                        };
                        self.rows.remove(i);
                    }
                    self.rows.push(Row::new(hash, false, now, dmg));
                }
            }
        }
    }

    pub fn disconnected(&mut self, now: u64) {
        for row in self.rows.iter_mut().filter(|r| !r.you) {
            row.freeze(now);
        }
    }

    pub fn set_own(&mut self, now: u64, hash: Member, dmg: Option<u64>) {
        let own = self.rows.iter_mut().find(|r| r.you);
        match (own, dmg) {
            (Some(row), Some(d)) if row.hash == hash => row.update(now, d),
            (Some(row), None) => row.freeze(now),
            (_, Some(d)) => {
                self.rows.retain(|r| !r.you);
                self.rows.push(Row::new(hash, true, now, d));
            }
            (None, None) => {}
        }
    }

    fn oldest_frozen(&self) -> Option<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, r)| !r.you && !r.live)
            .min_by_key(|(_, r)| (r.frozen_at, r.samples.back().map_or(0, |s| s.0)))
            .map(|(i, _)| i)
    }

    fn label(&mut self, hash: Member) -> String {
        if let Some((_, n)) = self.labels.iter().find(|(h, _)| *h == hash) {
            return format!("Unknown player {n}");
        }
        if self.labels.len() >= MAX_LABELS {
            return "Unknown player".to_string();
        }
        let n = self.labels.len() as u32 + 1;
        self.labels.push((hash, n));
        format!("Unknown player {n}")
    }

    pub fn view(&mut self, now: u64) -> Vec<PartyRow> {
        let total: u64 = self.rows.iter().map(|r| r.dmg).sum();
        let mut order: Vec<usize> = (0..self.rows.len()).collect();
        order.sort_by(|&a, &b| {
            let (ra, rb) = (&self.rows[a], &self.rows[b]);
            rb.dmg.cmp(&ra.dmg).then(rb.you.cmp(&ra.you))
        });
        let mut out = Vec::with_capacity(order.len());
        for i in order {
            let hash = self.rows[i].hash;
            let you = self.rows[i].you;
            let name = if you {
                "you".to_string()
            } else if let Some(n) = self.name(hash) {
                n.clone()
            } else {
                self.label(hash)
            };
            let r = &self.rows[i];
            out.push(PartyRow {
                name,
                you,
                live: r.live,
                dmg: r.dmg,
                share: (total > 0)
                    .then(|| ((r.dmg as u128 * 100 + total as u128 / 2) / total as u128) as u32),
                dps: r.dps(now),
            });
        }
        if out.len() > MAX_MEMBERS {
            let mut others = MAX_MEMBERS - out.iter().filter(|r| r.you).count().min(1);
            out.retain(|r| {
                r.you || {
                    let keep = others > 0;
                    others = others.saturating_sub(1);
                    keep
                }
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Member = [1; 8];
    const B: Member = [2; 8];
    const C: Member = [3; 8];
    const ME: Member = [9; 8];

    fn row<'a>(v: &'a [PartyRow], name: &str) -> &'a PartyRow {
        v.iter().find(|r| r.name == name).unwrap()
    }

    #[test]
    fn dps_from_snap_deltas() {
        let mut p = Party::default();
        p.apply_snap(0, &[(A, 5000)], None);
        assert_eq!(p.view(0)[0].dps, 0);
        for s in 1..=20u64 {
            p.apply_snap(s * 1000, &[(A, 5000 + s * 300)], None);
        }
        let v = p.view(20_000);
        assert_eq!(v[0].dmg, 11_000);
        assert_eq!(v[0].dps, 300);
        let mut q = Party::default();
        q.apply_snap(0, &[(A, 0)], None);
        q.apply_snap(1000, &[(A, 1000)], None);
        q.apply_snap(2000, &[(A, 2000)], None);
        assert_eq!(q.view(2000)[0].dps, 1000);
        assert_eq!(q.view(4000)[0].dps, 500);
        assert_eq!(q.view(12_000)[0].dps, 0);
    }

    #[test]
    fn dps_window_is_ten_seconds_not_the_hud_window() {
        let mut p = Party::default();
        p.apply_snap(0, &[(A, 0)], None);
        p.apply_snap(1000, &[(A, 10_000)], None);
        assert_eq!(p.view(1000)[0].dps, 10_000);
        assert_eq!(p.view(5000)[0].dps, 2000);
        assert_eq!(p.view(10_999)[0].dps, 1000);
        assert_eq!(p.view(11_000)[0].dps, 0);
    }

    #[test]
    fn damage_reset_restarts_samples() {
        let mut p = Party::default();
        p.apply_snap(0, &[(A, 0)], None);
        p.apply_snap(5000, &[(A, 50_000)], None);
        p.apply_snap(6000, &[(A, 100)], None);
        let v = p.view(6000);
        assert_eq!(v[0].dmg, 100);
        assert_eq!(v[0].dps, 0);
    }

    #[test]
    fn rows_freeze_when_missing_from_snap() {
        let mut p = Party::default();
        p.apply_snap(0, &[(A, 0), (B, 0)], None);
        p.apply_snap(2000, &[(A, 4000), (B, 1000)], None);
        p.apply_snap(3000, &[(A, 6000)], None);
        let v = p.view(3000);
        assert!(row(&v, "Unknown player 1").live);
        let b = row(&v, "Unknown player 2").clone();
        assert!(!b.live);
        assert_eq!((b.dmg, b.dps), (1000, 500));
        let later = p.view(60_000);
        assert_eq!(row(&later, "Unknown player 2").dps, 500);
        assert_eq!(row(&later, "Unknown player 2").dmg, 1000);
        p.apply_snap(61_000, &[(B, 9000)], None);
        let back = p.view(61_000);
        let b = row(&back, "Unknown player 2");
        assert!(b.live);
        assert_eq!((b.dmg, b.dps), (9000, 0));
        assert!(!row(&back, "Unknown player 1").live);
        p.apply_snap(62_000, &[], None);
        assert!(p.view(62_000).iter().all(|r| !r.live));
        assert_eq!(p.others(), 2);
    }

    #[test]
    fn frozen_dps_is_the_rate_at_the_last_sample() {
        let mut p = Party::default();
        let mut q = Party::default();
        for s in 0..=20u64 {
            p.apply_snap(s * 1000, &[(A, s * 1000)], Some(ME));
            p.set_own(s * 1000, ME, Some(s * 1000));
            q.apply_snap(s * 1000, &[(A, s * 1000)], None);
        }
        assert_eq!(row(&p.view(20_000), "Unknown player 1").dps, 1000);
        assert_eq!(row(&p.view(20_000), "you").dps, 1000);
        let dropped = 20_000 + 10_500;
        assert_eq!(row(&p.view(dropped), "Unknown player 1").dps, 0);
        p.apply_snap(dropped, &[], Some(ME));
        p.set_own(dropped, ME, None);
        q.disconnected(dropped);
        for v in [p.view(dropped), p.view(dropped + 60_000)] {
            for name in ["Unknown player 1", "you"] {
                let r = row(&v, name);
                assert!(!r.live, "{name}");
                assert!((990..=1010).contains(&r.dps), "{name} {}", r.dps);
            }
        }
        let r = &q.view(dropped + 60_000)[0];
        assert!(!r.live);
        assert!((990..=1010).contains(&r.dps), "{}", r.dps);
    }

    #[test]
    fn names_stay_on_rows_after_the_player_leaves() {
        let mut p = Party::default();
        p.set_names(vec![(A, "Alice".to_string()), (B, "Bob".to_string())]);
        p.apply_snap(0, &[(A, 10)], None);
        p.set_names(vec![(B, "Bob".to_string())]);
        assert_eq!(p.view(0)[0].name, "Alice");
        p.apply_snap(1000, &[], None);
        p.set_names(Vec::new());
        assert_eq!(p.view(1000)[0].name, "Alice");
        p.apply_snap(2000, &[(B, 5)], None);
        let v = p.view(2000);
        assert_eq!(v[0].name, "Alice");
        assert_eq!(v[1].name, "Unknown player 1");
        p.clear();
        p.apply_snap(3000, &[(A, 7)], None);
        assert_eq!(p.view(3000)[0].name, "Unknown player 2");
        p.set_names(vec![(A, "Alice".to_string())]);
        p.set_names(Vec::new());
        p.reset_room();
        p.apply_snap(4000, &[(A, 1)], None);
        assert_eq!(p.view(4000)[0].name, "Unknown player 1");
    }

    #[test]
    fn a_full_table_evicts_the_row_that_froze_first() {
        let mut p = Party::default();
        let id = |i: u64| -> Member { (i + 100).to_le_bytes() };
        let all: Vec<(Member, u64)> = (0..MAX_MEMBERS as u64)
            .map(|i| (id(i), if i == 0 { 9000 } else { 1000 }))
            .collect();

        p.apply_snap(0, &all, None);
        p.apply_snap(1000, &all[1..], None);
        p.apply_snap(2000, &all[2..], None);
        let late: Vec<(Member, u64)> = all[2..].iter().map(|&(h, _)| (h, 5000)).collect();
        p.apply_snap(3000, &late, None);
        let mut next = late.clone();
        next.push((id(500), 1));
        p.apply_snap(4000, &next, None);
        assert_eq!(p.others(), MAX_MEMBERS);
        assert!(!p.rows.iter().any(|r| r.hash == id(0)));
        assert!(p.rows.iter().any(|r| r.hash == id(1)));
        assert!(p.rows.iter().any(|r| r.hash == id(500)));
    }

    #[test]
    fn disconnect_freezes_everyone_but_you() {
        let mut p = Party::default();
        p.apply_snap(0, &[(A, 10)], None);
        p.set_own(0, ME, Some(50));
        p.disconnected(1000);
        let v = p.view(1000);
        assert!(!row(&v, "Unknown player 1").live);
        assert!(row(&v, "you").live);
    }

    #[test]
    fn clear_and_room_reset() {
        let mut p = Party::default();
        p.apply_snap(0, &[(A, 10), (B, 20)], None);
        assert_eq!(p.view(0)[0].name, "Unknown player 1");
        p.clear();
        assert!(p.view(0).is_empty());
        p.apply_snap(0, &[(C, 5), (A, 7)], None);
        let v = p.view(0);
        assert_eq!(row(&v, "Unknown player 2").dmg, 7);
        assert_eq!(row(&v, "Unknown player 3").dmg, 5);
        p.reset_room();
        assert!(p.view(0).is_empty());
        p.apply_snap(0, &[(C, 5)], None);
        assert_eq!(p.view(0)[0].name, "Unknown player 1");
    }

    #[test]
    fn names_from_roster_and_stable_player_numbers() {
        let mut p = Party::default();
        p.set_names(vec![(B, "Be\u{430}r (Hands)".to_string())]);
        p.apply_snap(0, &[(A, 3), (B, 2), (C, 1)], None);
        let v = p.view(0);
        assert_eq!(
            v.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["Unknown player 1", "Be\u{430}r (Hands)", "Unknown player 2"]
        );
        p.apply_snap(1000, &[(C, 100), (A, 3)], None);
        let v = p.view(1000);
        assert_eq!(v[0].name, "Unknown player 2");
        assert_eq!(row(&v, "Unknown player 1").dmg, 3);
        p.set_names(vec![(C, "Carol".to_string())]);
        let v = p.view(1000);
        assert_eq!(v[0].name, "Carol");
        assert_eq!(row(&v, "Unknown player 1").dmg, 3);
        assert_eq!(row(&v, "Be\u{430}r (Hands)").dmg, 2);
        assert!(!v.iter().any(|r| r.name == "Unknown player 3"));
    }

    #[test]
    fn own_row_from_local_numbers_and_self_filtered_from_snap() {
        let mut p = Party::default();
        p.apply_snap(0, &[(ME, 999_999), (A, 300)], Some(ME));
        assert_eq!(p.others(), 1);
        p.set_own(0, ME, None);
        assert!(p.view(0).iter().all(|r| !r.you));
        p.set_own(0, ME, Some(0));
        p.set_own(1000, ME, Some(700));
        let v = p.view(1000);
        assert_eq!(v[0].name, "you");
        assert!(v[0].you);
        assert_eq!((v[0].dmg, v[0].dps), (700, 700));
        assert_eq!(v[0].share, Some(70));
        assert_eq!(v[1].share, Some(30));
        p.set_own(2000, ME, None);
        let v = p.view(30_000);
        assert_eq!((v[0].dmg, v[0].dps, v[0].live), (700, 700, false));
        p.set_own(2500, ME, None);
        assert_eq!(p.view(40_000)[0].dps, 700);
        p.set_own(3000, [8; 8], Some(5));
        let v = p.view(3000);
        assert_eq!(v.iter().filter(|r| r.you).count(), 1);
        assert_eq!(row(&v, "you").dmg, 5);
    }

    #[test]
    fn shares_round_and_handle_zero_total() {
        let mut p = Party::default();
        p.apply_snap(0, &[(A, 0), (B, 0)], None);
        assert!(p.view(0).iter().all(|r| r.share.is_none()));
        p.apply_snap(0, &[(A, 1), (B, 2)], None);
        let v = p.view(0);
        assert_eq!(v[0].share, Some(67));
        assert_eq!(v[1].share, Some(33));
        p.apply_snap(0, &[(A, 1 << 40), (B, 1 << 40)], None);
        assert_eq!(p.view(0)[0].share, Some(50));
    }

    #[test]
    fn rows_stay_bounded() {
        let mut p = Party::default();
        p.set_own(0, ME, Some(1));
        for round in 0..10u64 {
            let snap: Vec<(Member, u64)> = (0..MAX_MEMBERS as u64)
                .map(|i| {
                    let mut h = [0u8; 8];
                    h[..8].copy_from_slice(&(round * 100 + i).to_le_bytes());
                    (h, i)
                })
                .collect();
            p.apply_snap(round * 1000, &snap, Some(ME));
            assert!(p.others() <= MAX_MEMBERS);
        }
        let v = p.view(10_000);
        assert_eq!(v.len(), MAX_MEMBERS);
        assert!(v.iter().any(|r| r.you));
        for i in 0..2000u64 {
            p.apply_snap(i, &[(i.to_le_bytes(), 1)], None);
            let _ = p.view(i);
        }
        assert!(p.labels.len() <= MAX_LABELS);
        assert!(p.rows.iter().all(|r| r.samples.len() <= MAX_SAMPLES));
    }
}
