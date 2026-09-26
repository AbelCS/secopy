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
    Ext4,
    Btrfs,
    Xfs,
    Ntfs,
    ReFs,
    ExFat,
    /// FAT12/16/32.
    Fat,
    Smb,
    Nfs,
    /// Anything else, with the name or magic number the OS reported.
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
    pub free_bytes: u64,
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

    /// Names as macOS (`f_fstypename`) and Windows (`GetVolumeInformation`) report them.
    #[cfg_attr(target_os = "linux", allow(dead_code))]
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

    /// Linux `statfs.f_type` magic numbers.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    fn from_magic(magic: u64) -> Self {
        match magic {
            0xEF53 => FsKind::Ext4,
            0x9123_683E => FsKind::Btrfs,
            0x5846_5342 => FsKind::Xfs,
            0x4D44 => FsKind::Fat,
            0x2011_BAB0 => FsKind::ExFat,
            0x7366_746E | 0x5346_544E => FsKind::Ntfs,
            0xFE53_4D42 | 0xFF53_4D42 | 0x517B => FsKind::Smb,
            0x6969 => FsKind::Nfs,
            other => FsKind::Other(format!("0x{other:x}")),
        }
    }
}

/// Reads the facts for the volume holding `dir`. Probing case sensitivity creates and
/// removes a hidden file, so an error here also means `dir` is not writable.
pub fn fs_info(dir: &Path) -> io::Result<FsInfo> {
    let kind = sys::fs_kind(dir)?;
    Ok(FsInfo {
        case_sensitive: probe_case_sensitive(dir)?,
        free_bytes: sys::free_bytes(dir)?,
        max_file_size: kind.max_file_size(),
        name_limit: kind.name_limit(),
        device: device_id(dir)?,
        kind,
    })
}

/// Identifies the volume holding `path` (`st_dev` on Unix, the volume serial on Windows).
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

#[cfg(unix)]
mod sys {
    use std::io;
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;

    use super::FsKind;
    use crate::os::c_path;

    #[cfg(target_os = "macos")]
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

    #[cfg(target_os = "linux")]
    pub fn fs_kind(dir: &Path) -> io::Result<FsKind> {
        let c = c_path(dir)?;
        // SAFETY: an all-zero `statfs` is a valid value to be overwritten.
        let mut st: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: `c` is NUL-terminated and `st` is a valid out-pointer.
        if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
            return Err(io::Error::last_os_error());
        }
        #[allow(clippy::unnecessary_cast)] // the field's type differs between targets
        Ok(FsKind::from_magic(st.f_type as u64))
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    pub fn fs_kind(_dir: &Path) -> io::Result<FsKind> {
        Ok(FsKind::Other("unknown".into()))
    }

    pub fn free_bytes(dir: &Path) -> io::Result<u64> {
        let c = c_path(dir)?;
        // SAFETY: an all-zero `statvfs` is a valid value to be overwritten.
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        // SAFETY: `c` is NUL-terminated and `st` is a valid out-pointer.
        if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
            return Err(io::Error::last_os_error());
        }
        #[allow(clippy::unnecessary_cast)] // the fields' types differ between targets
        Ok(st.f_bavail as u64 * st.f_frsize as u64)
    }

    pub fn device_id(path: &Path) -> io::Result<u64> {
        Ok(std::fs::metadata(path)?.dev())
    }
}

#[cfg(windows)]
mod sys {
    use std::fs::{File, OpenOptions};
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;

    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, GetDiskFreeSpaceExW, GetVolumeInformationByHandleW,
    };

    use super::FsKind;

    /// Directories can only be opened with backup semantics.
    fn open_dir(path: &Path) -> io::Result<File> {
        OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
    }

    /// (file system name, volume serial number)
    fn volume(path: &Path) -> io::Result<(String, u32)> {
        let file = open_dir(path)?;
        let mut serial = 0u32;
        let mut name = [0u16; 64];
        // SAFETY: valid handle; the out-pointers live for the call; null for unused ones.
        let ok = unsafe {
            GetVolumeInformationByHandleW(
                file.as_raw_handle(),
                std::ptr::null_mut(),
                0,
                &mut serial,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                name.as_mut_ptr(),
                name.len() as u32,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
        Ok((String::from_utf16_lossy(&name[..len]), serial))
    }

    pub fn fs_kind(dir: &Path) -> io::Result<FsKind> {
        Ok(FsKind::from_name(&volume(dir)?.0))
    }

    pub fn free_bytes(dir: &Path) -> io::Result<u64> {
        let wide: Vec<u16> = dir.as_os_str().encode_wide().chain([0]).collect();
        let mut available = 0u64;
        // SAFETY: `wide` is NUL-terminated; unused out-pointers are null.
        let ok = unsafe {
            GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut available,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(available)
    }

    pub fn device_id(path: &Path) -> io::Result<u64> {
        Ok(u64::from(volume(path)?.1))
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
        assert_eq!(info.device, device_id(dir.path()).unwrap());
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            0,
            "the probe file is removed"
        );
    }

    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn default_system_volumes_are_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!fs_info(dir.path()).unwrap().case_sensitive);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_temp_volumes_are_case_sensitive() {
        let dir = tempfile::tempdir().unwrap();
        assert!(fs_info(dir.path()).unwrap().case_sensitive);
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
        assert_eq!(FsKind::from_magic(0x4D44), FsKind::Fat);
        assert!(!FsKind::from_name("apfs").has_windows_names());
        assert_eq!(FsKind::from_name("apfs").max_file_size(), None);
        assert_eq!(
            FsKind::from_magic(0xEF53).name_limit(),
            NameLimit::Bytes(255)
        );
    }
}
