use super::sha256::sha256;

pub type Room = [u8; 16];
pub type Member = [u8; 8];
pub type Claim = [u8; 16];

pub const JOIN: u8 = 0x01;
pub const DMG: u8 = 0x02;
pub const SNAP: u8 = 0x03;
pub const END: u8 = 0x04;
pub const DMG2: u8 = 0x05;
pub const SNAP2: u8 = 0x06;
pub const JOIN_LEN: usize = 41;
pub const DMG_LEN: usize = 17;
pub const DMG2_LEN: usize = 18;
pub const SNAP_ENTRY: usize = 24;
pub const SNAP2_ENTRY: usize = 25;
pub const MAX_MEMBERS: usize = 82;
pub const MAX_SNAP_LEN: usize = 1 + SNAP2_ENTRY * MAX_MEMBERS;
pub const MAX_DMG: u64 = 1 << 40;
pub const MAX_LIFE: u8 = 3;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Totals {
    pub dealt: u64,
    pub taken: u64,
    pub life: u8,
}

impl From<(u64, u64)> for Totals {
    fn from((dealt, taken): (u64, u64)) -> Totals {
        Totals {
            dealt,
            taken,
            life: 0,
        }
    }
}

impl Totals {
    pub fn capped(self) -> Totals {
        Totals {
            dealt: self.dealt.min(MAX_DMG),
            taken: self.taken.min(MAX_DMG),
            life: self.life.min(MAX_LIFE),
        }
    }

    fn write(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.dealt.to_le_bytes());
        out.extend_from_slice(&self.taken.to_le_bytes());
    }

    fn read(b: &[u8]) -> Option<Totals> {
        let t = Totals {
            dealt: u64::from_le_bytes(b.get(..8)?.try_into().ok()?),
            taken: u64::from_le_bytes(b.get(8..16)?.try_into().ok()?),
            life: 0,
        };
        (t.dealt <= MAX_DMG && t.taken <= MAX_DMG).then_some(t)
    }
}

pub fn room_hash(location: &str) -> Room {
    let d = sha256(&[b"ehud-room-v1\0", location.as_bytes()]);
    let mut out = [0u8; 16];
    out.copy_from_slice(&d[..16]);
    out
}

pub fn member_hash(location: &str, usr: &str) -> Member {
    let d = sha256(&[
        b"ehud-member-v1\0",
        location.as_bytes(),
        b"\0",
        usr.as_bytes(),
    ]);
    let mut out = [0u8; 8];
    out.copy_from_slice(&d[..8]);
    out
}

pub fn claim_hash(key: &[u8; 32], location: &str) -> Claim {
    let d = sha256(&[b"ehud-claim-v1\0", key, location.as_bytes()]);
    let mut out = [0u8; 16];
    out.copy_from_slice(&d[..16]);
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    Join {
        room: Room,
        member: Member,
        claim: Claim,
    },
    Dmg(Totals),
    End,
    Snap(Vec<(Member, Totals)>),
}

impl Msg {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Msg::Join {
                room,
                member,
                claim,
            } => {
                let mut out = Vec::with_capacity(JOIN_LEN);
                out.push(JOIN);
                out.extend_from_slice(room);
                out.extend_from_slice(member);
                out.extend_from_slice(claim);
                out
            }
            Msg::Dmg(t) => {
                let mut out = Vec::with_capacity(DMG2_LEN);
                out.push(DMG2);
                t.write(&mut out);
                out.push(t.life);
                out
            }
            Msg::End => vec![END],
            Msg::Snap(entries) => {
                let life = entries.iter().any(|(_, t)| t.life != 0);
                let mut out = Vec::with_capacity(1 + SNAP2_ENTRY * entries.len());
                out.push(if life { SNAP2 } else { SNAP });
                for (m, t) in entries {
                    out.extend_from_slice(m);
                    t.write(&mut out);
                    if life {
                        out.push(t.life);
                    }
                }
                out
            }
        }
    }

    pub fn decode(b: &[u8]) -> Option<Msg> {
        let (&kind, rest) = b.split_first()?;
        match kind {
            JOIN if b.len() == JOIN_LEN => Some(Msg::Join {
                room: rest[..16].try_into().ok()?,
                member: rest[16..24].try_into().ok()?,
                claim: rest[24..].try_into().ok()?,
            }),
            END if rest.is_empty() => Some(Msg::End),
            DMG if b.len() == DMG_LEN => Totals::read(rest).map(Msg::Dmg),
            DMG2 if b.len() == DMG2_LEN => {
                let life = rest[16];
                let t = Totals::read(rest)?;
                (life <= MAX_LIFE).then_some(Msg::Dmg(Totals { life, ..t }))
            }
            SNAP if rest.len() % SNAP_ENTRY == 0 && b.len() <= 1 + SNAP_ENTRY * MAX_MEMBERS => {
                let mut entries = Vec::with_capacity(rest.len() / SNAP_ENTRY);
                for c in rest.as_chunks::<SNAP_ENTRY>().0 {
                    entries.push((c[..8].try_into().ok()?, Totals::read(&c[8..])?));
                }
                Some(Msg::Snap(entries))
            }
            SNAP2 if rest.len() % SNAP2_ENTRY == 0 && b.len() <= MAX_SNAP_LEN => {
                let mut entries = Vec::with_capacity(rest.len() / SNAP2_ENTRY);
                for c in rest.as_chunks::<SNAP2_ENTRY>().0 {
                    let life = c[24].min(MAX_LIFE);
                    let t = Totals::read(&c[8..24])?;
                    entries.push((c[..8].try_into().ok()?, Totals { life, ..t }));
                }
                Some(Msg::Snap(entries))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::relay::sha256::hex;

    pub(crate) const LOC: &str = "wrld_00000000-0000-0000-0000-000000000000:12345~region(us)";
    pub(crate) const USR: &str = "usr_00000000-0000-0000-0000-000000000000";
    const ROOM_HEX: &str = "6e35fa9ce4780b2a38a74e3b8d9ce66b";
    const MEMBER_HEX: &str = "6a59070463d1200d";
    const KEY: [u8; 32] = [0x42; 32];

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn cross_check_hashes() {
        assert_eq!(hex(&room_hash(LOC)), ROOM_HEX);
        assert_eq!(hex(&member_hash(LOC, USR)), MEMBER_HEX);
        assert_ne!(room_hash(LOC), room_hash(&format!("{LOC} ")));
        assert_ne!(member_hash(LOC, USR), member_hash(LOC, "usr_1"));
        assert_ne!(
            member_hash(LOC, USR)[..],
            room_hash(&format!("{LOC}\0{USR}"))[..8]
        );
    }

    #[test]
    fn claim_depends_on_key_and_location() {
        let mut input = b"ehud-claim-v1\0".to_vec();
        input.extend_from_slice(&KEY);
        input.extend_from_slice(LOC.as_bytes());
        assert_eq!(
            claim_hash(&KEY, LOC)[..],
            crate::relay::sha256::sha256(&[&input])[..16]
        );
        assert_ne!(claim_hash(&KEY, LOC), claim_hash(&[0x43; 32], LOC));
        assert_ne!(claim_hash(&KEY, LOC), claim_hash(&KEY, &format!("{LOC} ")));
        assert_ne!(claim_hash(&KEY, LOC), room_hash(LOC));
    }

    #[test]
    fn join_fixture() {
        let msg = Msg::Join {
            room: room_hash(LOC),
            member: member_hash(LOC, USR),
            claim: [0xC3; 16],
        };
        let bytes = msg.encode();
        assert_eq!(
            hex(&bytes),
            "016e35fa9ce4780b2a38a74e3b8d9ce66b6a59070463d1200dc3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3"
        );
        assert_eq!(bytes.len(), JOIN_LEN);
        assert_eq!(Msg::decode(&bytes), Some(msg));
    }

    fn t(dealt: u64, taken: u64) -> Totals {
        Totals::from((dealt, taken))
    }

    fn tl(dealt: u64, taken: u64, life: u8) -> Totals {
        Totals {
            life,
            ..t(dealt, taken)
        }
    }

    #[test]
    fn dmg_fixture() {
        let fx = "0215cd5b07000000002a00000000000000";
        assert_eq!(
            hex(&Msg::Dmg(tl(123456789, 42, 2)).encode()),
            "0515cd5b07000000002a0000000000000002"
        );
        assert_eq!(
            hex(&Msg::Dmg(t(0, 0)).encode()),
            "050000000000000000".to_string() + "000000000000000000"
        );
        assert_eq!(
            hex(&Msg::Dmg(t(MAX_DMG, MAX_DMG)).encode()),
            "050000000000010000".to_string() + "000000000001000000"
        );
        assert_eq!(Msg::decode(&unhex(fx)), Some(Msg::Dmg(t(123456789, 42))));
        let dmg2 = Msg::Dmg(tl(9, 8, 3));
        assert_eq!(Msg::decode(&dmg2.encode()), Some(dmg2));
        let mut bad = Msg::Dmg(tl(9, 8, 3)).encode();
        bad[17] = MAX_LIFE + 1;
        assert_eq!(Msg::decode(&bad), None);
        assert_eq!(tl(1, 1, 200).capped().life, MAX_LIFE);
        let over = "020100000000010000".to_string() + "0000000000000000";
        assert_eq!(Msg::decode(&unhex(&over)), None);
        let over = "020000000000000000".to_string() + "0100000000010000";
        assert_eq!(Msg::decode(&unhex(&over)), None);
        assert_eq!(Msg::decode(&unhex("0215cd5b0700000000")), None);
    }

    #[test]
    fn capped_clamps_both_totals() {
        assert_eq!(t(u64::MAX, 5).capped(), t(MAX_DMG, 5));
        assert_eq!(t(5, u64::MAX).capped(), t(5, MAX_DMG));
    }

    #[test]
    fn end_fixture() {
        assert_eq!(hex(&Msg::End.encode()), "04");
        assert_eq!(Msg::decode(&[END]), Some(Msg::End));
        assert_eq!(Msg::decode(&[END, 0]), None);
    }

    #[test]
    fn snap_fixture() {
        let me = member_hash(LOC, USR);
        let other: Member = [0, 1, 2, 3, 4, 5, 6, 7];
        let msg = Msg::Snap(vec![(me, t(4242, 7)), (other, t(MAX_DMG, 0))]);
        let want = [
            "03",
            "6a59070463d1200d",
            "9210000000000000",
            "0700000000000000",
            "0001020304050607",
            "0000000000010000",
            "0000000000000000",
        ]
        .concat();
        assert_eq!(hex(&msg.encode()), want);
        assert_eq!(Msg::decode(&unhex(&want)), Some(msg));
        assert_eq!(Msg::decode(&[SNAP]), Some(Msg::Snap(Vec::new())));
        assert_eq!(hex(&Msg::Snap(Vec::new()).encode()), "03");
        let full = Msg::Snap(
            (0..MAX_MEMBERS as u8)
                .map(|i| ([i; 8], t(i as u64, 1)))
                .collect(),
        );
        let bytes = full.encode();
        assert_eq!(bytes.len(), 1969);
        assert_eq!(Msg::decode(&bytes), Some(full));
        let mut over = bytes.clone();
        over.extend_from_slice(&[9; SNAP_ENTRY]);
        assert_eq!(Msg::decode(&over), None);
        let mut big = unhex(&want);
        big[14] = 0x01;
        assert_eq!(Msg::decode(&big), None);
        let mut big = unhex(&want);
        big[22] = 0x01;
        assert_eq!(Msg::decode(&big), None);
    }

    #[test]
    fn snap2_carries_life() {
        let me = member_hash(LOC, USR);
        let msg = Msg::Snap(vec![(me, tl(4242, 7, 2)), ([1; 8], t(5, 0))]);
        let want = [
            "06",
            "6a59070463d1200d",
            "9210000000000000",
            "0700000000000000",
            "02",
            "0101010101010101",
            "0500000000000000",
            "0000000000000000",
            "00",
        ]
        .concat();
        assert_eq!(hex(&msg.encode()), want);
        assert_eq!(Msg::decode(&unhex(&want)), Some(msg));
        assert_eq!(Msg::decode(&[SNAP2]), Some(Msg::Snap(Vec::new())));
        let mut odd = unhex(&want);
        odd[25] = 0xff;
        let Some(Msg::Snap(e)) = Msg::decode(&odd) else {
            panic!("snap2 with an unknown life should still decode");
        };
        assert_eq!(e[0].1.life, MAX_LIFE);
        let full = Msg::Snap(
            (0..MAX_MEMBERS as u8)
                .map(|i| ([i; 8], tl(i as u64, 1, 1)))
                .collect(),
        );
        let bytes = full.encode();
        assert_eq!(bytes.len(), MAX_SNAP_LEN);
        assert_eq!(bytes.len(), 2051);
        assert_eq!(Msg::decode(&bytes), Some(full));
        let mut over = bytes.clone();
        over.extend_from_slice(&[0; SNAP2_ENTRY]);
        assert_eq!(Msg::decode(&over), None);
        let mut long_old = vec![SNAP];
        long_old.resize(1 + SNAP_ENTRY * (MAX_MEMBERS + 1), 0);
        assert_eq!(Msg::decode(&long_old), None);
    }

    #[test]
    fn every_wrong_length_is_rejected() {
        let join = Msg::Join {
            room: [7; 16],
            member: [9; 8],
            claim: [5; 16],
        }
        .encode();
        for len in 1..1400 {
            let mut b = join.clone();
            b.resize(len, 0);
            assert_eq!(Msg::decode(&b).is_some(), len == JOIN_LEN, "join {len}");
            let mut d = Msg::Dmg(t(5, 1)).encode();
            d.resize(len, 0);
            assert_eq!(Msg::decode(&d).is_some(), len == DMG2_LEN, "dmg2 {len}");
            d[0] = DMG;
            assert_eq!(Msg::decode(&d).is_some(), len == DMG_LEN, "dmg {len}");
            let mut e = Msg::End.encode();
            e.resize(len, 0);
            assert_eq!(Msg::decode(&e).is_some(), len == 1, "end {len}");
            let mut s = vec![SNAP];
            s.resize(len, 0);
            let ok = len >= 1 && (len - 1) % SNAP_ENTRY == 0 && len <= 1969;
            assert_eq!(Msg::decode(&s).is_some(), ok, "snap {len}");
            s[0] = SNAP2;
            let ok = len >= 1 && (len - 1) % SNAP2_ENTRY == 0 && len <= MAX_SNAP_LEN;
            assert_eq!(Msg::decode(&s).is_some(), ok, "snap2 {len}");
        }
        assert_eq!(Msg::decode(&[]), None);
        for kind in [0x00, 0x07, 0x7f, 0xff] {
            assert_eq!(Msg::decode(&[kind]), None);
            assert_eq!(Msg::decode(&[kind; 9]), None);
            assert_eq!(Msg::decode(&[kind; 17]), None);
            assert_eq!(Msg::decode(&[kind; 25]), None);
            assert_eq!(Msg::decode(&[kind; 41]), None);
        }
    }
}
