//! xxHash64 helpers. Secopy always uses seed 0 and the canonical
//! big-endian lowercase hex form that `xxhsum` prints (RFD §4).

pub use xxhash_rust::xxh64::Xxh64;

/// Seed used for every hash.
const SEED: u64 = 0;

/// Creates a streaming hasher with the Secopy seed.
pub fn hasher() -> Xxh64 {
    Xxh64::new(SEED)
}

/// Hashes a byte slice in one go.
pub fn hash_bytes(bytes: &[u8]) -> u64 {
    xxhash_rust::xxh64::xxh64(bytes, SEED)
}

/// Canonical form: 16 lowercase hex characters, as printed by `xxhsum`.
pub fn to_hex(hash: u64) -> String {
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_reference_vectors() {
        assert_eq!(hash_bytes(b""), 0xef46_db37_51d8_e999);
        assert_eq!(hash_bytes(b"abc"), 0x44bc_2cf5_ad77_0999);
    }

    #[test]
    fn streaming_matches_one_shot() {
        let data = b"secopy streaming hash";
        let mut h = hasher();
        h.update(&data[..5]);
        h.update(&data[5..]);
        assert_eq!(h.digest(), hash_bytes(data));
    }

    #[test]
    fn hex_is_16_lowercase_chars() {
        assert_eq!(to_hex(0xAB), "00000000000000ab");
        assert_eq!(to_hex(0xef46_db37_51d8_e999), "ef46db3751d8e999");
    }
}
