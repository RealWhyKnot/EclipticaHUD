use super::proto::{Member, Totals, MAX_MEMBERS};
use std::collections::VecDeque;

pub const DPS_WINDOW_MS: u64 = 10_000;
const MAX_SAMPLES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRow {
    pub name: String,
    pub you: bool,
    pub live: bool,
    pub dmg: u64,
    pub taken: u64,
    pub life: u8,
    pub share: Option<u32>,
    pub dps: u64,
}

#[derive(Debug)]
struct Row {
    hash: Member,
    you: bool,
    live: bool,
    dmg: u64,
    taken: u64,
    life: u8,
    frozen_dps: u64,
    frozen_at: u64,
    samples: VecDeque<(u64, u64)>,
}

impl Row {
    fn new(hash: Member, you: bool, now: u64, t: Totals) -> Self {
        let dmg = t.dealt;
        Row {
            hash,
            you,
            live: true,
            dmg,
            taken: t.taken,
            life: t.life,
            frozen_dps: 0,
            frozen_at: 0,
            samples: VecDeque::from([(now, dmg)]),
        }
    }

    fn update(&mut self, now: u64, t: Totals) {
        let dmg = t.dealt;
        self.taken = t.taken;
        self.life = t.life;
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
    names: Vec<(Member, String)>,
    gone: Vec<(Member, String)>,
}

impl Party {
    pub fn reset_room(&mut self) {
        self.rows.clear();
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

    pub fn apply_snap(&mut self, now: u64, entries: &[(Member, Totals)], own: Option<Member>) {
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

    pub fn set_own(&mut self, now: u64, hash: Member, dmg: Option<Totals>) {
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

    pub fn view(&self, now: u64) -> Vec<PartyRow> {
        let mut shown: Vec<(&Row, String)> = self
            .rows
            .iter()
            .filter_map(|r| {
                if r.you {
                    Some((r, "you".to_string()))
                } else {
                    self.name(r.hash).map(|n| (r, n.clone()))
                }
            })
            .collect();
        shown.sort_by(|(ra, _), (rb, _)| rb.dmg.cmp(&ra.dmg).then(rb.you.cmp(&ra.you)));
        let total: u64 = shown.iter().map(|(r, _)| r.dmg).sum();
        let mut out = Vec::with_capacity(shown.len());
        for (r, name) in shown {
            out.push(PartyRow {
                name,
                you: r.you,
                live: r.live,
                dmg: r.dmg,
                taken: r.taken,
                life: r.life,
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

    fn dealt(n: u64) -> Totals {
        Totals::from((n, 0))
    }

    impl Party {
        fn snap(&mut self, now: u64, entries: &[(Member, u64)], own: Option<Member>) {
            let full: Vec<_> = entries.iter().map(|&(h, n)| (h, dealt(n))).collect();
            self.apply_snap(now, &full, own);
        }

        fn own(&mut self, now: u64, hash: Member, dmg: Option<u64>) {
            self.set_own(now, hash, dmg.map(dealt));
        }
    }

    const A: Member = [1; 8];
    const B: Member = [2; 8];
    const C: Member = [3; 8];
    const ME: Member = [9; 8];

    fn row<'a>(v: &'a [PartyRow], name: &str) -> &'a PartyRow {
        v.iter().find(|r| r.name == name).unwrap()
    }

    fn names(list: &[(Member, &str)]) -> Vec<(Member, String)> {
        list.iter().map(|(h, n)| (*h, n.to_string())).collect()
    }

    fn named() -> Party {
        let mut p = Party::default();
        p.set_names(names(&[(A, "Ann"), (B, "Ben"), (C, "Cat")]));
        p
    }

    #[test]
    fn dps_from_snap_deltas() {
        let mut p = named();
        p.snap(0, &[(A, 5000)], None);
        assert_eq!(p.view(0)[0].dps, 0);
        for s in 1..=20u64 {
            p.snap(s * 1000, &[(A, 5000 + s * 300)], None);
        }
        let v = p.view(20_000);
        assert_eq!(v[0].dmg, 11_000);
        assert_eq!(v[0].dps, 300);
        let mut q = named();
        q.snap(0, &[(A, 0)], None);
        q.snap(1000, &[(A, 1000)], None);
        q.snap(2000, &[(A, 2000)], None);
        assert_eq!(q.view(2000)[0].dps, 1000);
        assert_eq!(q.view(4000)[0].dps, 500);
        assert_eq!(q.view(12_000)[0].dps, 0);
    }

    #[test]
    fn dps_window_is_ten_seconds_not_the_hud_window() {
        let mut p = named();
        p.snap(0, &[(A, 0)], None);
        p.snap(1000, &[(A, 10_000)], None);
        assert_eq!(p.view(1000)[0].dps, 10_000);
        assert_eq!(p.view(5000)[0].dps, 2000);
        assert_eq!(p.view(10_999)[0].dps, 1000);
        assert_eq!(p.view(11_000)[0].dps, 0);
    }

    #[test]
    fn damage_reset_restarts_samples() {
        let mut p = named();
        p.snap(0, &[(A, 0)], None);
        p.snap(5000, &[(A, 50_000)], None);
        p.snap(6000, &[(A, 100)], None);
        let v = p.view(6000);
        assert_eq!(v[0].dmg, 100);
        assert_eq!(v[0].dps, 0);
    }

    #[test]
    fn rows_freeze_when_missing_from_snap() {
        let mut p = named();
        p.snap(0, &[(A, 0), (B, 0)], None);
        p.snap(2000, &[(A, 4000), (B, 1000)], None);
        p.snap(3000, &[(A, 6000)], None);
        let v = p.view(3000);
        assert!(row(&v, "Ann").live);
        let b = row(&v, "Ben").clone();
        assert!(!b.live);
        assert_eq!((b.dmg, b.dps), (1000, 500));
        let later = p.view(60_000);
        assert_eq!(row(&later, "Ben").dps, 500);
        assert_eq!(row(&later, "Ben").dmg, 1000);
        p.snap(61_000, &[(B, 9000)], None);
        let back = p.view(61_000);
        let b = row(&back, "Ben");
        assert!(b.live);
        assert_eq!((b.dmg, b.dps), (9000, 0));
        assert!(!row(&back, "Ann").live);
        p.snap(62_000, &[], None);
        assert!(p.view(62_000).iter().all(|r| !r.live));
        assert_eq!(p.others(), 2);
    }

    #[test]
    fn frozen_dps_is_the_rate_at_the_last_sample() {
        let mut p = named();
        let mut q = named();
        for s in 0..=20u64 {
            p.snap(s * 1000, &[(A, s * 1000)], Some(ME));
            p.own(s * 1000, ME, Some(s * 1000));
            q.snap(s * 1000, &[(A, s * 1000)], None);
        }
        assert_eq!(row(&p.view(20_000), "Ann").dps, 1000);
        assert_eq!(row(&p.view(20_000), "you").dps, 1000);
        let dropped = 20_000 + 10_500;
        assert_eq!(row(&p.view(dropped), "Ann").dps, 0);
        p.snap(dropped, &[], Some(ME));
        p.own(dropped, ME, None);
        q.disconnected(dropped);
        for v in [p.view(dropped), p.view(dropped + 60_000)] {
            for name in ["Ann", "you"] {
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
        p.set_names(names(&[(A, "Alice"), (B, "Bob")]));
        p.snap(0, &[(A, 10)], None);
        p.set_names(names(&[(B, "Bob")]));
        assert_eq!(p.view(0)[0].name, "Alice");
        p.snap(1000, &[], None);
        p.set_names(Vec::new());
        assert_eq!(p.view(1000)[0].name, "Alice");
        p.snap(2000, &[(B, 5)], None);
        let v = p.view(2000);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "Alice");
        p.clear();
        p.snap(3000, &[(A, 7)], None);
        assert!(p.view(3000).is_empty());
        p.set_names(names(&[(A, "Alice")]));
        p.set_names(Vec::new());
        assert_eq!(p.view(3000)[0].name, "Alice");
        p.reset_room();
        p.snap(4000, &[(A, 1)], None);
        assert!(p.view(4000).is_empty());
    }

    #[test]
    fn a_full_table_evicts_the_row_that_froze_first() {
        let mut p = Party::default();
        let id = |i: u64| -> Member { (i + 100).to_le_bytes() };
        let all: Vec<(Member, u64)> = (0..MAX_MEMBERS as u64)
            .map(|i| (id(i), if i == 0 { 9000 } else { 1000 }))
            .collect();

        p.snap(0, &all, None);
        p.snap(1000, &all[1..], None);
        p.snap(2000, &all[2..], None);
        let late: Vec<(Member, u64)> = all[2..].iter().map(|&(h, _)| (h, 5000)).collect();
        p.snap(3000, &late, None);
        let mut next = late.clone();
        next.push((id(500), 1));
        p.snap(4000, &next, None);
        assert_eq!(p.others(), MAX_MEMBERS);
        assert!(!p.rows.iter().any(|r| r.hash == id(0)));
        assert!(p.rows.iter().any(|r| r.hash == id(1)));
        assert!(p.rows.iter().any(|r| r.hash == id(500)));
    }

    #[test]
    fn disconnect_freezes_everyone_but_you() {
        let mut p = named();
        p.snap(0, &[(A, 10)], None);
        p.own(0, ME, Some(50));
        p.disconnected(1000);
        let v = p.view(1000);
        assert!(!row(&v, "Ann").live);
        assert!(row(&v, "you").live);
    }

    #[test]
    fn clear_and_room_reset() {
        let mut p = named();
        p.snap(0, &[(A, 10), (B, 20)], None);
        assert_eq!(p.view(0)[0].name, "Ben");
        p.clear();
        assert!(p.view(0).is_empty());
        p.snap(0, &[(C, 5), (A, 7)], None);
        let v = p.view(0);
        assert_eq!(row(&v, "Ann").dmg, 7);
        assert_eq!(row(&v, "Cat").dmg, 5);
        p.reset_room();
        assert!(p.view(0).is_empty());
        p.snap(0, &[(C, 5)], None);
        assert!(p.view(0).is_empty());
        p.set_names(names(&[(C, "Cat")]));
        assert_eq!(p.view(0)[0].name, "Cat");
    }

    #[test]
    fn only_players_in_the_roster_are_shown() {
        let mut p = Party::default();
        p.set_names(names(&[(B, "Be\u{430}r (Hands)")]));
        p.snap(0, &[(A, 3), (B, 2), (C, 1)], None);
        p.own(0, ME, Some(2));
        let v = p.view(0);
        assert_eq!(
            v.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["you", "Be\u{430}r (Hands)"]
        );
        assert_eq!(v[0].share, Some(50));
        assert_eq!(v[1].share, Some(50));
        p.snap(1000, &[(C, 100), (A, 3)], None);
        p.set_names(names(&[(C, "Carol")]));
        let v = p.view(1000);
        assert_eq!(
            v.iter()
                .map(|r| (r.name.as_str(), r.dmg))
                .collect::<Vec<_>>(),
            [("Carol", 100), ("you", 2), ("Be\u{430}r (Hands)", 2)]
        );
        assert_eq!(v[0].share, Some(96));
        assert!(!v.iter().any(|r| r.dmg == 3));
    }

    #[test]
    fn own_row_from_local_numbers_and_self_filtered_from_snap() {
        let mut p = named();
        p.snap(0, &[(ME, 999_999), (A, 300)], Some(ME));
        assert_eq!(p.others(), 1);
        p.own(0, ME, None);
        assert!(p.view(0).iter().all(|r| !r.you));
        p.own(0, ME, Some(0));
        p.own(1000, ME, Some(700));
        let v = p.view(1000);
        assert_eq!(v[0].name, "you");
        assert!(v[0].you);
        assert_eq!((v[0].dmg, v[0].dps), (700, 700));
        assert_eq!(v[0].share, Some(70));
        assert_eq!(v[1].share, Some(30));
        p.own(2000, ME, None);
        let v = p.view(30_000);
        assert_eq!((v[0].dmg, v[0].dps, v[0].live), (700, 700, false));
        p.own(2500, ME, None);
        assert_eq!(p.view(40_000)[0].dps, 700);
        p.own(3000, [8; 8], Some(5));
        let v = p.view(3000);
        assert_eq!(v.iter().filter(|r| r.you).count(), 1);
        assert_eq!(row(&v, "you").dmg, 5);
    }

    #[test]
    fn shares_round_and_handle_zero_total() {
        let mut p = named();
        p.snap(0, &[(A, 0), (B, 0)], None);
        assert!(p.view(0).iter().all(|r| r.share.is_none()));
        p.snap(0, &[(A, 1), (B, 2)], None);
        let v = p.view(0);
        assert_eq!(v[0].share, Some(67));
        assert_eq!(v[1].share, Some(33));
        p.snap(0, &[(A, 1 << 40), (B, 1 << 40)], None);
        assert_eq!(p.view(0)[0].share, Some(50));
    }

    #[test]
    fn rows_stay_bounded() {
        let mut p = Party::default();
        p.own(0, ME, Some(1));
        let hash = |n: u64| -> Member { n.to_le_bytes() };
        for round in 0..10u64 {
            let snap: Vec<(Member, u64)> = (0..MAX_MEMBERS as u64)
                .map(|i| (hash(round * 100 + i), i))
                .collect();
            p.snap(round * 1000, &snap, Some(ME));
            assert!(p.others() <= MAX_MEMBERS);
        }
        p.set_names(
            (0..MAX_MEMBERS as u64)
                .map(|i| (hash(900 + i), format!("P{i}")))
                .collect(),
        );
        let v = p.view(10_000);
        assert_eq!(v.len(), MAX_MEMBERS);
        assert!(v.iter().any(|r| r.you));
        for i in 0..2000u64 {
            p.snap(i, &[(i.to_le_bytes(), 1)], None);
            let _ = p.view(i);
        }
        assert!(p.others() <= MAX_MEMBERS);
        assert!(p.rows.iter().all(|r| r.samples.len() <= MAX_SAMPLES));
    }

    #[test]
    fn rows_carry_life_and_keep_it_when_frozen() {
        let mut p = named();
        p.apply_snap(
            0,
            &[(
                A,
                Totals {
                    life: 2,
                    ..dealt(10)
                },
            )],
            None,
        );
        p.set_own(
            0,
            ME,
            Some(Totals {
                life: 3,
                ..dealt(5)
            }),
        );
        let v = p.view(0);
        assert_eq!((row(&v, "Ann").life, row(&v, "you").life), (2, 3));
        p.snap(1000, &[], None);
        assert_eq!(row(&p.view(1000), "Ann").life, 2);
        p.snap(2000, &[(A, 12)], None);
        assert_eq!(row(&p.view(2000), "Ann").life, 0);
    }
}
