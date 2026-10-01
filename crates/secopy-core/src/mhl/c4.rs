//! C4 IDs (SMPTE ST 2114): how a chain file names a manifest's contents.

use sha2::{Digest, Sha512};

const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// `c4` and the SHA-512 of `bytes` as 88 base58 digits.
pub fn c4(bytes: &[u8]) -> String {
    let digest = Sha512::digest(bytes);
    let digits = bs58::encode(digest)
        .with_alphabet(&bs58::Alphabet::new(ALPHABET).expect("58 distinct ASCII digits"))
        .into_string();
    // bs58 writes one '1' per leading zero byte; C4 pads the number to 88 digits.
    let number = digits.trim_start_matches('1');
    format!("c4{number:1>88}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/ascmhl");

    #[test]
    fn the_standards_example_manifest_has_its_chains_c4() {
        let manifest =
            std::fs::read(format!("{FIXTURES}/0001_A002R2EC_2020-01-16_091500Z.mhl")).unwrap();
        assert_eq!(
            c4(&manifest),
            "c43qQABrjeiU3kteaNFtgwufQRLKxgMJEZfNSj6LcZ8fKwB568U8As9eScRsnb64NfqiAYzjNUXs81tAxPB77PUThb"
        );
    }

    #[test]
    fn every_id_is_90_characters() {
        // A digest with leading zero bytes still pads to 88 base58 digits.
        assert_eq!(c4(b"").len(), 90);
        assert!(c4(b"").starts_with("c4"));
    }
}
