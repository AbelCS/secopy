//! The drives FROM offers (plan 3b-1): the volumes mounted under `/Volumes`.

use std::fs;
use std::path::{Path, PathBuf};

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

/// What `statfs` says about an entry of `/Volumes`.
#[derive(Debug, Clone, PartialEq)]
struct Mount {
    name: String,
    path: PathBuf,
    /// Where its file system is mounted; the path itself for a real drive.
    mounted_on: PathBuf,
    /// Finder shows it (not `nobrowse`, like Recovery).
    browsable: bool,
    /// On this Mac, not a network share.
    local: bool,
    /// A mounted `.dmg`.
    disk_image: bool,
    total_bytes: u64,
    free_bytes: u64,
}

/// The drives under `volumes`, by name: real drives Finder would show, not disk images, and
/// not the one holding `dest`.
pub fn list(volumes: &Path, dest: Option<&Path>) -> Vec<DriveView> {
    let Ok(entries) = fs::read_dir(volumes) else {
        return Vec::new();
    };
    let mounts = entries
        .filter_map(Result::ok)
        // `file_type` doesn't follow links, so the link to `/` isn't a folder here.
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| !e.file_name().as_encoded_bytes().starts_with(b"."))
        .filter_map(|e| mount_info(&e.path()));
    drives(mounts, dest)
}

fn drives(mounts: impl IntoIterator<Item = Mount>, dest: Option<&Path>) -> Vec<DriveView> {
    let mut drives: Vec<DriveView> = mounts
        .into_iter()
        .filter(|m| m.mounted_on == m.path && m.browsable && m.local && !m.disk_image)
        .filter(|m| dest.is_none_or(|d| !d.starts_with(&m.path)))
        .map(|m| DriveView {
            name: m.name,
            path: show(&m.path),
            total_bytes: m.total_bytes,
            free_bytes: m.free_bytes,
        })
        .collect();
    drives.sort_by_key(|d| d.name.to_lowercase());
    drives
}

/// Where `path`'s file system is mounted, the device it comes from, and `statfs`'s answer.
fn mount_of(path: &Path) -> Option<(PathBuf, String, libc::statfs)> {
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
    let mounted_on = PathBuf::from(OsStr::from_bytes(on.to_bytes()));
    Some((mounted_on, from.to_string_lossy().into_owned(), st))
}

fn mount_info(path: &Path) -> Option<Mount> {
    let (mounted_on, from, st) = mount_of(path)?;
    let is_mount = mounted_on == path;
    let block = u64::from(st.f_bsize);
    Some(Mount {
        name: path.file_name()?.to_string_lossy().into_owned(),
        path: path.to_path_buf(),
        browsable: st.f_flags & libc::MNT_DONTBROWSE as u32 == 0,
        local: st.f_flags & libc::MNT_LOCAL as u32 != 0,
        // Only asked for real mounts: a folder left in /Volumes is on the Mac's own disk.
        disk_image: is_mount
            && from
                .strip_prefix("/dev/")
                .is_some_and(disk_arbitration::is_disk_image),
        mounted_on,
        total_bytes: st.f_blocks * block,
        free_bytes: st.f_bavail * block,
    })
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
    let (mounted_on, from, _) = mount_of(path)?;
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

/// Asks DiskArbitration what a disk is: a mounted `.dmg` has the model "Disk Image".
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
        static kDADiskDescriptionDeviceModelKey: CFStringRef;
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

    pub fn is_disk_image(bsd_name: &str) -> bool {
        let Some(d) = description(bsd_name) else {
            return false;
        };
        // SAFETY: the key is DiskArbitration's constant string.
        let key = unsafe { CFString::wrap_under_get_rule(kDADiskDescriptionDeviceModelKey) };
        d.find(&key)
            .and_then(|v| v.downcast::<CFString>())
            .is_some_and(|model| model == "Disk Image")
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
    use std::path::PathBuf;

    fn mount(name: &str) -> Mount {
        let path = PathBuf::from(format!("/Volumes/{name}"));
        Mount {
            name: name.into(),
            mounted_on: path.clone(),
            path,
            browsable: true,
            local: true,
            disk_image: false,
            total_bytes: 64_000,
            free_bytes: 20_000,
        }
    }

    #[test]
    fn only_real_local_browsable_mounts_that_arent_disk_images_are_drives() {
        let leftover = Mount {
            mounted_on: "/System/Volumes/Data".into(),
            ..mount("Backups of Mac 1")
        };
        let hidden = Mount {
            browsable: false,
            ..mount("Recovery")
        };
        let network = Mount {
            local: false,
            ..mount("NAS")
        };
        let image = Mount {
            disk_image: true,
            ..mount("Secopy")
        };
        let all = [
            mount("SSD_T7"),
            leftover,
            hidden,
            network,
            image,
            mount("card_a"),
        ];
        let names = |d: Vec<DriveView>| d.into_iter().map(|d| d.name).collect::<Vec<_>>();
        assert_eq!(names(drives(all.clone(), None)), ["card_a", "SSD_T7"]);
        let dest = Path::new("/Volumes/SSD_T7/Day01");
        assert_eq!(
            names(drives(all, Some(dest))),
            ["card_a"],
            "not the destination's drive"
        );
    }

    #[test]
    fn folders_left_in_volumes_are_not_drives() {
        // Old Time Machine backups leave folders like these behind; none is a mount point.
        let dir = tempfile::tempdir().unwrap();
        for name in ["Backups of Mac", "CARD_A", ".Trashes"] {
            fs::create_dir(dir.path().join(name)).unwrap();
        }
        std::os::unix::fs::symlink("/", dir.path().join("Macintosh HD")).unwrap();
        fs::write(dir.path().join("a file"), b"").unwrap();
        assert!(list(dir.path(), None).is_empty());
    }

    #[test]
    fn a_missing_volumes_folder_lists_nothing() {
        assert!(list(Path::new("/no/such/volumes"), None).is_empty());
    }

    #[test]
    fn listed_drives_are_real_mounts_and_not_disk_images() {
        // On this Mac: whatever is mounted, the list only has real, non-image drives.
        for d in list(Path::new(VOLUMES), None) {
            let m = mount_info(Path::new(&d.path)).unwrap();
            assert_eq!(m.mounted_on, m.path, "{}", d.name);
            assert!(m.browsable && m.local && !m.disk_image, "{}", d.name);
        }
    }

    #[test]
    fn a_mounted_disk_image_is_not_a_drive() {
        use std::process::Command;
        let dir = tempfile::tempdir().unwrap();
        // Canonical: macOS reports mounts under /var as /private/var.
        let root = dir.path().canonicalize().unwrap();
        let image = root.join("image.dmg");
        let volumes = root.join("Volumes");
        let mount = volumes.join("IMAGE");
        fs::create_dir_all(&mount).unwrap();
        let ok = |c: &mut Command| c.output().is_ok_and(|o| o.status.success());
        let created = ok(Command::new("hdiutil")
            .args([
                "create", "-quiet", "-size", "1m", "-fs", "HFS+", "-volname", "IMAGE",
            ])
            .arg(&image));
        if !created {
            return; // no hdiutil here
        }
        assert!(ok(Command::new("hdiutil")
            .args(["attach", "-quiet", "-nobrowse", "-mountpoint"])
            .arg(&mount)
            .arg(&image)));
        let info = mount_info(&mount).unwrap();
        let listed = list(&volumes, None);
        let _ = Command::new("hdiutil")
            .args(["detach", "-quiet"])
            .arg(&mount)
            .status();
        assert_eq!(info.mounted_on, mount, "it is a real mount");
        assert!(info.disk_image, "and a disk image");
        assert!(listed.is_empty());
    }

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
