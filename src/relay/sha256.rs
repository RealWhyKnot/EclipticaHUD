use std::fmt::Write as _;

const K: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

const H0: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

pub(crate) struct Sha256 {
    h: [u32; 8],
    buf: [u8; 64],
    fill: usize,
    len: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Sha256 {
            h: H0,
            buf: [0; 64],
            fill: 0,
            len: 0,
        }
    }
}

impl Sha256 {
    pub(crate) fn update(&mut self, mut data: &[u8]) {
        self.len = self.len.wrapping_add(crate::num::to_u64(data.len()));
        while !data.is_empty() {
            let take = (64 - self.fill).min(data.len());
            self.buf[self.fill..self.fill + take].copy_from_slice(&data[..take]);
            self.fill += take;
            data = &data[take..];
            if self.fill == 64 {
                let block = self.buf;
                self.compress(&block);
                self.fill = 0;
            }
        }
    }

    pub(crate) fn finish(mut self) -> [u8; 32] {
        let bits = self.len.wrapping_mul(8);
        self.update(&[0x80]);
        while self.fill != 56 {
            self.update(&[0]);
        }
        self.update(&bits.to_be_bytes());
        let mut out = [0u8; 32];
        for (o, h) in out.as_chunks_mut::<4>().0.iter_mut().zip(self.h) {
            *o = h.to_be_bytes();
        }
        out
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut words = [0u32; 64];
        for (word, chunk) in words.iter_mut().zip(block.as_chunks::<4>().0) {
            *word = u32::from_be_bytes(*chunk);
        }
        for at in 16..64 {
            let near = words[at - 15];
            let far = words[at - 2];
            let s0 = near.rotate_right(7) ^ near.rotate_right(18) ^ (near >> 3);
            let s1 = far.rotate_right(17) ^ far.rotate_right(19) ^ (far >> 10);
            words[at] = words[at - 16]
                .wrapping_add(s0)
                .wrapping_add(words[at - 7])
                .wrapping_add(s1);
        }
        let mut state = self.h;
        for (constant, word) in K.iter().zip(words) {
            let s1 =
                state[4].rotate_right(6) ^ state[4].rotate_right(11) ^ state[4].rotate_right(25);
            let ch = (state[4] & state[5]) ^ (!state[4] & state[6]);
            let t1 = state[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(*constant)
                .wrapping_add(word);
            let s0 =
                state[0].rotate_right(2) ^ state[0].rotate_right(13) ^ state[0].rotate_right(22);
            let maj = (state[0] & state[1]) ^ (state[0] & state[2]) ^ (state[1] & state[2]);
            let t2 = s0.wrapping_add(maj);
            state.rotate_right(1);
            state[4] = state[4].wrapping_add(t1);
            state[0] = t1.wrapping_add(t2);
        }
        for (sum, value) in self.h.iter_mut().zip(state) {
            *sum = sum.wrapping_add(value);
        }
    }
}

pub(crate) fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut s = Sha256::default();
    for p in parts {
        s.update(p);
    }
    s.finish()
}

pub(crate) fn hex(b: &[u8]) -> String {
    b.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nist_vectors() {
        let cases: [(&[u8], &str); 4] = [
            (
                b"",
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                b"abc",
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
                "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            ),
            (
                b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu",
                "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1",
            ),
        ];
        for (msg, want) in cases {
            assert_eq!(hex(&sha256(&[msg])), want);
        }
    }

    #[test]
    fn one_million_a() {
        let mut s = Sha256::default();
        let chunk = [b'a'; 1000];
        for _ in 0..1000 {
            s.update(&chunk);
        }
        assert_eq!(
            hex(&s.finish()),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
        assert_eq!(
            hex(&sha256(&[&vec![b'a'; 1_000_000]])),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn split_updates_match_one_shot() {
        let msg: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
        let whole = sha256(&[&msg]);
        for cut in [0, 1, 55, 56, 63, 64, 65, 127, 128, 999, 1000] {
            assert_eq!(sha256(&[&msg[..cut], &msg[cut..]]), whole, "{cut}");
        }
        for len in 50..=130 {
            let m = &msg[..len];
            let mut s = Sha256::default();
            for b in m {
                s.update(std::slice::from_ref(b));
            }
            assert_eq!(s.finish(), sha256(&[m]), "{len}");
        }
    }
}
