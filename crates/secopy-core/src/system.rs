//! Files computers leave on a card, never the camera (FR-12). Everything else is copied,
//! hidden or not: a camera can mark its own files hidden on FAT/exFAT cards.

use std::ffi::OsStr;

/// Exact names: macOS and Windows bookkeeping. A directory with one of these names is skipped
/// with everything in it.
const NAMES: &[&str] = &[
    // Secopy: a mirror's checksum file (plan 8)
    ".secopy-checksums.xxh64",
    // macOS
    ".DS_Store",
    ".Spotlight-V100",
    ".fseventsd",
    ".Trashes",
    ".Trash",
    ".TemporaryItems",
    ".DocumentRevisions-V100",
    ".VolumeIcon.icns",
    ".apdisk",
    ".localized",
    "Icon\r",
    // Windows
    "System Volume Information",
    "$RECYCLE.BIN",
    "Thumbs.db",
    "desktop.ini",
];

/// Secopy's own unfinished copies: `.name.secopy-partial`, `.secopy-<hash>.partial`.
pub fn is_secopy_partial(name: &str) -> bool {
    (name.starts_with('.') && name.ends_with(".secopy-partial"))
        || (name.starts_with(".secopy-") && name.ends_with(".partial"))
}

/// Whether `name` is a system file or directory to skip. `._*` files are AppleDouble data
/// macOS writes next to files on FAT and exFAT; `.secopy-partial` files are Secopy's own
/// unfinished copies.
pub fn is_system_file(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    NAMES.iter().any(|n| n.eq_ignore_ascii_case(&name))
        || name.starts_with("._")
        || is_secopy_partial(&name)
        // A mirror's checksum file set aside (#114): Secopy's own, never an extra file.
        || name.starts_with(".secopy-checksums.xxh64.damaged-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computer_bookkeeping_is_a_system_file() {
        for name in [
            ".DS_Store",
            "._A001.MOV",
            ".Spotlight-V100",
            ".fseventsd",
            ".Trashes",
            "System Volume Information",
            "$RECYCLE.BIN",
            "thumbs.db",
            ".A001.MOV.secopy-partial",
            ".secopy-0123456789abcdef.partial",
        ] {
            assert!(is_system_file(OsStr::new(name)), "{name}");
        }
    }

    #[test]
    fn camera_and_user_files_are_not() {
        for name in [
            ".camera_index",
            ".config",
            "A001.MOV",
            "PRIVATE",
            "DS_Store",
            "_A001.MOV",
        ] {
            assert!(!is_system_file(OsStr::new(name)), "{name}");
        }
    }
}
