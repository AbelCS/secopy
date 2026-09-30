//! Facts about the destination volume, for pre-flight and fatal-error detection
//! (FR-16, FR-17a, FR-21).

use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// File system families that change what pre-flight checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsKind {
    Apfs,
    HfsPlus,
    Ntfs,
    ReFs,
    ExFat,
    /// FAT12/16/32.
    Fat,
    Smb,
    Nfs,
    /// Anything else, with the name the OS reported.
    Other(String),
}

/// Longest file name the file system accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameLimit {
    Bytes(usize),
    Utf16Units(usize),
}

/// Largest file FAT can hold.
pub const FAT_MAX_FILE_SIZE: u64 = (4 << 30) - 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsInfo {
    pub kind: FsKind,
    /// Probed by creating a file, not guessed from the kind.
    pub case_sensitive: bool,
    /// Free now (`statvfs`).
    pub free_bytes: u64,
    /// Free now plus the purgeable space (Time Machine local snapshots, caches) macOS frees
    /// on demand: its figure for data the user asks to store, what Finder and Disk Utility
    /// show. Never less than `free_bytes`.
    pub available_bytes: u64,
    /// The allocation unit (`f_frsize`): a file takes whole blocks, a cluster on exFAT (#135).
    pub block_size: u64,
    pub max_file_size: Option<u64>,
    pub name_limit: NameLimit,
    /// Identifies the volume; a change means it was unplugged or remounted (FR-21).
    pub device: u64,
}

impl FsKind {
    /// Windows file-name rules apply on these, whatever OS writes to them (FR-16).
    pub fn has_windows_names(&self) -> bool {
        matches!(
            self,
            FsKind::Ntfs | FsKind::ReFs | FsKind::ExFat | FsKind::Fat
        )
    }

    fn name_limit(&self) -> NameLimit {
        match self {
            FsKind::Apfs
            | FsKind::HfsPlus
            | FsKind::Ntfs
            | FsKind::ReFs
            | FsKind::ExFat
            | FsKind::Fat => NameLimit::Utf16Units(255),
            _ => NameLimit::Bytes(255),
        }
    }

    fn max_file_size(&self) -> Option<u64> {
        (*self == FsKind::Fat).then_some(FAT_MAX_FILE_SIZE)
    }

    /// Names as macOS reports them (`f_fstypename`).
    fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "apfs" => FsKind::Apfs,
            "hfs" => FsKind::HfsPlus,
            "ntfs" => FsKind::Ntfs,
            "refs" => FsKind::ReFs,
            "exfat" => FsKind::ExFat,
            "msdos" | "fat" | "fat32" => FsKind::Fat,
            "smbfs" => FsKind::Smb,
            "nfs" => FsKind::Nfs,
            other => FsKind::Other(other.to_string()),
        }
    }
}

/// Reads the facts for the volume holding `dir`. Probing case sensitivity creates and
/// removes a hidden file, so an error here also means `dir` is not writable.
pub fn fs_info(dir: &Path) -> io::Result<FsInfo> {
    let kind = sys::fs_kind(dir)?;
    let (free_bytes, block_size) = sys::free_bytes(dir)?;
    Ok(FsInfo {
        case_sensitive: probe_case_sensitive(dir)?,
        free_bytes,
        block_size,
        available_bytes: available(free_bytes, sys::important_usage_bytes(dir)),
        max_file_size: kind.max_file_size(),
        name_limit: kind.name_limit(),
        device: device_id(dir)?,
        kind,
    })
}

/// The larger of the space free now and macOS's figure for important usage. That figure is 0
/// on volumes outside the boot volume group (disk images, external drives), and space free
/// now can always be written (#108).
fn available(free: u64, important: Option<u64>) -> u64 {
    important.map_or(free, |i| i.max(free))
}

/// Identifies the volume holding `path` (`st_dev`).
pub fn device_id(path: &Path) -> io::Result<u64> {
    sys::device_id(path)
}

fn probe_case_sensitive(dir: &Path) -> io::Result<bool> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let name = format!(".secopy-probe-{}-{nanos}", std::process::id());
    let path = dir.join(&name);
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    let insensitive = fs::symlink_metadata(dir.join(name.to_uppercase())).is_ok();
    fs::remove_file(&path)?;
    Ok(!insensitive)
}

mod sys {
    use std::io;
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;

    use super::FsKind;
    use crate::os::c_path;

    pub fn fs_kind(dir: &Path) -> io::Result<FsKind> {
        let c = c_path(dir)?;
        // SAFETY: an all-zero `statfs` is a valid value to be overwritten.
        let mut st: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: `c` is NUL-terminated and `st` is a valid out-pointer.
        if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the kernel NUL-terminates `f_fstypename`.
        let name = unsafe { std::ffi::CStr::from_ptr(st.f_fstypename.as_ptr()) };
        Ok(FsKind::from_name(&name.to_string_lossy()))
    }

    /// Bytes free now, and the allocation unit.
    pub fn free_bytes(dir: &Path) -> io::Result<(u64, u64)> {
        let c = c_path(dir)?;
        // SAFETY: an all-zero `statvfs` is a valid value to be overwritten.
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        // SAFETY: `c` is NUL-terminated and `st` is a valid out-pointer.
        if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok((u64::from(st.f_bavail) * st.f_frsize, st.f_frsize.max(1)))
    }

    /// `NSURLVolumeAvailableCapacityForImportantUsageKey`: Apple's figure for data stored at
    /// the user's request. `None` when the lookup fails; 0 where the volume has no figure.
    pub fn important_usage_bytes(dir: &Path) -> Option<u64> {
        use objc2::rc::autoreleasepool;
        use objc2_foundation::{
            NSNumber, NSString, NSURL, NSURLVolumeAvailableCapacityForImportantUsageKey,
        };

        let path = dir.to_str()?;
        autoreleasepool(|_| {
            let url = NSURL::fileURLWithPath(&NSString::from_str(path));
            let mut value = None;
            // SAFETY: the key's value is an NSNumber, and `value` accepts any object.
            unsafe {
                url.getResourceValue_forKey_error(
                    &mut value,
                    NSURLVolumeAvailableCapacityForImportantUsageKey,
                )
            }
            .ok()?;
            let bytes = value?.downcast::<NSNumber>().ok()?.longLongValue();
            u64::try_from(bytes).ok()
        })
    }

    pub fn device_id(path: &Path) -> io::Result<u64> {
        Ok(std::fs::metadata(path)?.dev())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_temp_volume() {
        let dir = tempfile::tempdir().unwrap();
        let info = fs_info(dir.path()).unwrap();
        assert!(info.free_bytes > 0);
        assert!(info.available_bytes > 0);
        assert_eq!(info.device, device_id(dir.path()).unwrap());
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            0,
            "the probe file is removed"
        );
    }

    #[test]
    fn macos_answers_for_important_usage_and_fails_for_a_missing_path() {
        let dir = tempfile::tempdir().unwrap();
        assert!(sys::important_usage_bytes(dir.path()).is_some());
        assert_eq!(sys::important_usage_bytes(&dir.path().join("nope")), None);
    }

    #[test]
    fn available_is_never_less_than_free_now() {
        // Volumes outside the boot volume group (disk images, external drives) answer 0.
        assert_eq!(available(500, Some(0)), 500);
        assert_eq!(available(500, None), 500);
        assert_eq!(available(500, Some(400)), 500);
        assert_eq!(available(500, Some(9_000)), 9_000);
    }

    #[test]
    fn default_system_volumes_are_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!fs_info(dir.path()).unwrap().case_sensitive);
    }

    #[test]
    fn a_missing_directory_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(fs_info(&dir.path().join("nope")).is_err());
    }

    #[test]
    fn fat_has_windows_names_and_a_4_gib_limit() {
        let fat = FsKind::from_name("msdos");
        assert_eq!(fat, FsKind::Fat);
        assert!(fat.has_windows_names());
        assert_eq!(fat.max_file_size(), Some(FAT_MAX_FILE_SIZE));
        assert!(!FsKind::from_name("apfs").has_windows_names());
        assert_eq!(FsKind::from_name("apfs").max_file_size(), None);
        assert_eq!(
            FsKind::Other("ext4".into()).name_limit(),
            NameLimit::Bytes(255)
        );
    }
}
