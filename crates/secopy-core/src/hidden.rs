//! Hidden file detection (FR-12).

use std::fs::Metadata;
use std::path::Path;

/// A file or directory is hidden if its name starts with `.`, or the OS marks it
/// hidden: `UF_HIDDEN` on macOS, `HIDDEN`/`SYSTEM` attributes on Windows.
pub fn is_hidden(path: &Path, meta: &Metadata) -> bool {
    let dot_name = path
        .file_name()
        .is_some_and(|n| n.as_encoded_bytes().first() == Some(&b'.'));
    dot_name || os_hidden(meta)
}

#[cfg(target_os = "macos")]
fn os_hidden(meta: &Metadata) -> bool {
    use std::os::macos::fs::MetadataExt;
    const UF_HIDDEN: u32 = 0x8000;
    meta.st_flags() & UF_HIDDEN != 0
}

#[cfg(windows)]
fn os_hidden(meta: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
    meta.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
}

#[cfg(not(any(target_os = "macos", windows)))]
fn os_hidden(_meta: &Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn check(name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        fs::write(&path, b"x").unwrap();
        (dir, path)
    }

    #[test]
    fn dot_names_are_hidden() {
        let (_dir, path) = check(".DS_Store");
        assert!(is_hidden(&path, &fs::metadata(&path).unwrap()));
    }

    #[test]
    fn plain_names_are_visible() {
        let (_dir, path) = check("A001.mov");
        assert!(!is_hidden(&path, &fs::metadata(&path).unwrap()));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_hidden_flag_is_detected() {
        let (_dir, path) = check("flagged.mov");
        let status = std::process::Command::new("chflags")
            .arg("hidden")
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(is_hidden(&path, &fs::metadata(&path).unwrap()));
    }

    #[cfg(windows)]
    #[test]
    fn windows_hidden_attribute_is_detected() {
        let (_dir, path) = check("flagged.mov");
        let status = std::process::Command::new("attrib")
            .arg("+h")
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(is_hidden(&path, &fs::metadata(&path).unwrap()));
    }
}
