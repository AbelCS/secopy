//! Helpers shared by the integration tests.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use walkdir::WalkDir;

/// Creates files (and their parent folders) under `root`.
pub fn write_files(root: &Path, files: &[(&str, &[u8])]) {
    for (rel, data) in files {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }
}

/// Every file under `root` as `slash/path → contents`, excluding `.xxh64` checksum files.
pub fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    WalkDir::new(root)
        .into_iter()
        .map(Result::unwrap)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_none_or(|x| x != "xxh64"))
        .map(|e| {
            let rel = e.path().strip_prefix(root).unwrap();
            let key = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            (key, fs::read(e.path()).unwrap())
        })
        .collect()
}

/// `n` bytes of a repeating, non-trivial pattern.
pub fn pattern(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 31 % 251) as u8).collect()
}
