//! XXH128 (XXH3 128-bit, seed 0), Secopy's one hash, in the canonical lowercase hex form
//! `xxhsum -H2` prints (RFD §4, #178).

/// An XXH128 (XXH3 128-bit, seed 0): Secopy's one hash. A type of its own, so a hash is never
/// mixed with a number, cut short, or compared with a hash of another width (#178).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hash(u128);

impl Hash {
    /// Characters in the canonical form.
    pub const HEX_LEN: usize = 32;

    /// A hash with this value: for tests and fixtures.
    pub const fn from_u128(value: u128) -> Hash {
        Hash(value)
    }

    /// Hashes a byte slice in one go.
    pub fn of(bytes: &[u8]) -> Hash {
        Hash(twox_hash::XxHash3_128::oneshot(bytes))
    }

    /// Canonical form: 32 lowercase hex characters, as `xxhsum -H2` prints.
    pub fn to_hex(self) -> String {
        format!("{:032x}", self.0)
    }

    /// Reads the canonical form back: exactly 32 hex digits (either case), nothing else.
    pub fn from_hex(text: &str) -> Option<Hash> {
        (text.len() == Self::HEX_LEN && text.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| u128::from_str_radix(text, 16).ok().map(Hash))
            .flatten()
    }
}

impl std::fmt::Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl std::fmt::Debug for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Hash({})", self.to_hex())
    }
}

/// A streaming XXH128: the same value as [`Hash::of`] over the whole input, however it is split.
#[derive(Clone)]
pub struct Hasher(twox_hash::XxHash3_128);

impl Hasher {
    pub fn new() -> Hasher {
        Hasher(twox_hash::XxHash3_128::new())
    }

    pub fn update(&mut self, bytes: &[u8]) {
        self.0.write(bytes);
    }

    pub fn digest(&self) -> Hash {
        Hash(self.0.finish_128())
    }
}

impl Default for Hasher {
    fn default() -> Self {
        Hasher::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xxh128_matches_xxhsum() {
        // `printf abc | xxhsum -H2`, `printf '' | xxhsum -H2`
        assert_eq!(
            Hash::of(b"abc").to_hex(),
            "06b05ab6733a618578af5f94892f3950"
        );
        assert_eq!(Hash::of(b"").to_hex(), "99aa06d3014798d86001c324468d497f");
    }

    #[test]
    fn streaming_equals_one_shot_for_every_size_class() {
        // XXH3 takes other paths for 0, 1–3, 4–8, 9–16, 17–128, 129–240 and longer inputs,
        // and the copy feeds 4 MiB buffers: every size, split anywhere, hashes the same.
        let data: Vec<u8> = (0..(4u32 << 20) + 300)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect();
        let sizes = [
            0,
            1,
            3,
            4,
            8,
            9,
            16,
            17,
            128,
            129,
            240,
            241,
            1024,
            65_537,
            (4 << 20) - 1,
            4 << 20,
            (4 << 20) + 1,
        ];
        for &n in &sizes {
            let whole = Hash::of(&data[..n]);
            for split in [1, 7, 64, 240, 4096, 1 << 20] {
                let mut h = Hasher::new();
                for chunk in data[..n].chunks(split) {
                    h.update(chunk);
                }
                assert_eq!(h.digest(), whole, "size {n}, chunks of {split}");
            }
        }
    }

    #[test]
    fn hex_is_32_lowercase_and_read_back_strictly() {
        let h = Hash::from_u128(0xAB);
        assert_eq!(h.to_hex(), "000000000000000000000000000000ab");
        assert_eq!(Hash::from_hex(&h.to_hex()), Some(h));
        assert_eq!(
            Hash::from_hex("06B05AB6733A618578AF5F94892F3950"),
            Some(Hash::of(b"abc"))
        );
        let long = "0".repeat(33);
        for bad in [
            "",
            "ab",
            "ef46db3751d8e999",
            long.as_str(),
            "06b05ab6733a618578af5f94892f395g",
            " 6b05ab6733a618578af5f94892f3950",
            "+6b05ab6733a618578af5f94892f3950",
        ] {
            assert_eq!(Hash::from_hex(bad), None, "{bad:?}");
        }
    }

    /// Speed check (spec): run by hand on a release build, never in CI:
    /// `cargo test --release -p secopy-core --lib hash::tests::xxh128_is_at_least_as_fast -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn xxh128_is_at_least_as_fast() {
        let data = vec![0x5au8; 256 << 20];
        let t = std::time::Instant::now();
        let mut h = Hasher::new();
        for _ in 0..16 {
            for chunk in data.chunks(4 << 20) {
                h.update(chunk);
            }
        }
        std::hint::black_box(h.digest());
        let gbs = (16.0 * data.len() as f64) / t.elapsed().as_secs_f64() / 1e9;
        eprintln!("XXH128 streaming: {gbs:.1} GB/s");
        assert!(gbs >= 15.6, "{gbs:.1} GB/s is slower than xxh64 was");
    }
}
