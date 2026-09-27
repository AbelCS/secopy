//! The drives FROM offers (plan 3b-1): the volumes mounted under `/Volumes`.

use std::fs;
use std::path::Path;

use serde::Serialize;
use specta::Type;

use crate::dto::show;

/// Where macOS mounts drives.
pub const VOLUMES: &str = "/Volumes";

#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DriveView {
    pub name: String,
    pub path: String,
    #[specta(type = specta_typescript::Number)]
    pub total_bytes: u64,
    #[specta(type = specta_typescript::Number)]
    pub free_bytes: u64,
}

/// The drives under `volumes`, by name. Leaves out links (the Mac's own disk is a link to
/// `/`), hidden entries, anything that isn't a folder, and the drive holding `dest`.
pub fn list(volumes: &Path, dest: Option<&Path>) -> Vec<DriveView> {
    let Ok(entries) = fs::read_dir(volumes) else {
        return Vec::new();
    };
    let mut drives: Vec<DriveView> = entries
        .filter_map(Result::ok)
        // `file_type` doesn't follow links, so the link to `/` isn't a folder here.
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| !e.file_name().as_encoded_bytes().starts_with(b"."))
        .filter(|e| dest.is_none_or(|d| !d.starts_with(e.path())))
        .filter_map(|e| {
            let (total_bytes, free_bytes) = space(&e.path())?;
            Some(DriveView {
                name: e.file_name().to_string_lossy().into_owned(),
                path: show(&e.path()),
                total_bytes,
                free_bytes,
            })
        })
        .collect();
    drives.sort_by_key(|d| d.name.to_lowercase());
    drives
}

/// Total and free bytes of the file system holding `path`.
fn space(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: an all-zero `statfs` is a valid value to be overwritten.
    let mut st: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is a valid C string and `st` a valid out-parameter.
    if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
        return None;
    }
    let block = u64::from(st.f_bsize);
    Some((st.f_blocks * block, st.f_bavail * block))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drives_are_listed_by_name_without_links_hidden_entries_or_the_destination() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["SSD_T7", "card_a", ".Trashes", "RAID"] {
            fs::create_dir(dir.path().join(name)).unwrap();
        }
        std::os::unix::fs::symlink("/", dir.path().join("Macintosh HD")).unwrap();
        fs::write(dir.path().join("a file"), b"").unwrap();
        let dest = dir.path().join("RAID/Day01");
        let drives = list(dir.path(), Some(&dest));
        let names: Vec<_> = drives.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["card_a", "SSD_T7"]);
        assert_eq!(drives[0].path, show(&dir.path().join("card_a")));
        assert!(
            drives
                .iter()
                .all(|d| d.total_bytes > 0 && d.free_bytes <= d.total_bytes)
        );
        assert_eq!(
            list(dir.path(), None).len(),
            3,
            "RAID is back without a destination"
        );
    }

    #[test]
    fn a_missing_volumes_folder_lists_nothing() {
        assert!(list(Path::new("/no/such/volumes"), None).is_empty());
    }
}
