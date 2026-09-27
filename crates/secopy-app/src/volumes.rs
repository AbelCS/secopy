//! The drives a finished copy can offer to eject (spec 3b-2 §2), and where macOS mounts
//! them.

use std::path::{Path, PathBuf};

use serde::Serialize;
use specta::Type;

use crate::dto::show;

/// Where macOS mounts drives.
pub const VOLUMES: &str = "/Volumes";

/// Where `path`'s file system is mounted, and the device it comes from.
fn mount_of(path: &Path) -> Option<(PathBuf, String)> {
    use std::ffi::{CStr, OsStr};
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: an all-zero `statfs` is a valid value to be overwritten.
    let mut st: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is a valid C string and `st` a valid out-parameter.
    if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
        return None;
    }
    // SAFETY: the kernel fills both names as NUL-terminated strings.
    let (on, from) = unsafe {
        (
            CStr::from_ptr(st.f_mntonname.as_ptr()),
            CStr::from_ptr(st.f_mntfromname.as_ptr()),
        )
    };
    Some((
        PathBuf::from(OsStr::from_bytes(on.to_bytes())),
        from.to_string_lossy().into_owned(),
    ))
}

/// A drive the summary can offer to eject.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DriveRef {
    pub name: String,
    pub mount_point: String,
}

/// What DiskArbitration says about a disk; `None` when it doesn't say.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct DiskFacts {
    internal: Option<bool>,
    removable: Option<bool>,
    ejectable: Option<bool>,
}

/// External, or removable media (an SD card in the built-in reader); never the root.
fn ejectable(mount_point: &Path, f: DiskFacts) -> bool {
    mount_point != Path::new("/")
        && (f.internal == Some(false) || f.removable == Some(true) || f.ejectable == Some(true))
}

/// The ejectable drive holding `path`, if any.
pub fn ejectable_drive(path: &Path) -> Option<DriveRef> {
    let (mounted_on, from) = mount_of(path)?;
    let facts = disk_arbitration::facts(from.strip_prefix("/dev/")?)?;
    ejectable(&mounted_on, facts).then(|| DriveRef {
        name: mounted_on
            .file_name()
            .map_or_else(|| show(&mounted_on), |n| n.to_string_lossy().into_owned()),
        mount_point: show(&mounted_on),
    })
}

/// macOS's own eject: every volume on the drive, and the reason when it can't.
pub fn eject(mount_point: &Path) -> Result<(), String> {
    let out = std::process::Command::new("diskutil")
        .arg("eject")
        .arg(mount_point)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(if out.stderr.is_empty() {
        &out.stdout
    } else {
        &out.stderr
    });
    Err(text.trim().to_string())
}

/// Asks DiskArbitration whether a disk is internal, removable or ejectable.
mod disk_arbitration {
    use std::ffi::{CString, c_char, c_void};

    use core_foundation::base::{CFType, TCFType, kCFAllocatorDefault};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::string::{CFString, CFStringRef};
    use core_foundation_sys::base::CFAllocatorRef;

    type Ref = *const c_void;

    #[link(name = "DiskArbitration", kind = "framework")]
    unsafe extern "C" {
        fn DASessionCreate(allocator: CFAllocatorRef) -> Ref;
        fn DADiskCreateFromBSDName(
            allocator: CFAllocatorRef,
            session: Ref,
            name: *const c_char,
        ) -> Ref;
        fn DADiskCopyDescription(disk: Ref) -> CFDictionaryRef;
        static kDADiskDescriptionDeviceInternalKey: CFStringRef;
        static kDADiskDescriptionMediaRemovableKey: CFStringRef;
        static kDADiskDescriptionMediaEjectableKey: CFStringRef;
    }

    /// DiskArbitration's description of the disk `bsd_name` (like "disk6s1").
    fn description(bsd_name: &str) -> Option<CFDictionary<CFString, CFType>> {
        let name = CString::new(bsd_name).ok()?;
        // SAFETY: each call gets valid arguments, null results are checked, and every
        // created object is wrapped so it is released.
        unsafe {
            let session = DASessionCreate(kCFAllocatorDefault);
            if session.is_null() {
                return None;
            }
            let _session = CFType::wrap_under_create_rule(session);
            let disk = DADiskCreateFromBSDName(kCFAllocatorDefault, session, name.as_ptr());
            if disk.is_null() {
                return None;
            }
            let _disk = CFType::wrap_under_create_rule(disk);
            let description = DADiskCopyDescription(disk);
            (!description.is_null()).then(|| CFDictionary::wrap_under_create_rule(description))
        }
    }

    pub(super) fn facts(bsd_name: &str) -> Option<super::DiskFacts> {
        let d = description(bsd_name)?;
        // SAFETY: the keys are DiskArbitration's constant strings.
        let flag = |key: CFStringRef| unsafe {
            d.find(CFString::wrap_under_get_rule(key))
                .and_then(|v| v.downcast::<CFBoolean>())
                .map(bool::from)
        };
        Some(super::DiskFacts {
            internal: flag(unsafe { kDADiskDescriptionDeviceInternalKey }),
            removable: flag(unsafe { kDADiskDescriptionMediaRemovableKey }),
            ejectable: flag(unsafe { kDADiskDescriptionMediaEjectableKey }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_and_removable_drives_can_be_ejected_the_macs_own_disk_cannot() {
        let facts = |internal, removable, ejectable| DiskFacts {
            internal,
            removable,
            ejectable,
        };
        let card = Path::new("/Volumes/CARD_A");
        assert!(
            ejectable(card, facts(Some(false), Some(false), Some(false))),
            "USB drive"
        );
        assert!(
            ejectable(card, facts(Some(true), Some(true), Some(true))),
            "SD card in the built-in reader"
        );
        assert!(
            !ejectable(card, facts(Some(true), Some(false), Some(false))),
            "internal disk"
        );
        assert!(
            !ejectable(Path::new("/"), facts(Some(false), Some(true), Some(true))),
            "never the root"
        );
        assert!(
            !ejectable(card, DiskFacts::default()),
            "unknown: don't offer it"
        );
    }

    #[test]
    fn the_macs_own_disk_is_never_offered() {
        assert_eq!(ejectable_drive(Path::new("/")), None);
        assert_eq!(ejectable_drive(&std::env::temp_dir()), None);
    }
}
