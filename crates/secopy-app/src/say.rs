//! What the engine reports, as messages for the UI (#84). The engine's English `Display` stays
//! for reports and the CLI; here each variant becomes a catalog key with its details.

use std::io;

use secopy_core::check::{self, ProblemKind};
use secopy_core::error::{FatalError, FileError, IoFailure};
use secopy_core::fsinfo::NameLimit;
use secopy_core::mirror::{Guard, NotRemoved, PlanError, RemovalError};
use secopy_core::names::NameProblem;
use secopy_core::preflight::Blocker;
use secopy_core::scan::{DriveRoot, ScanProblem, ScanProblemKind};

use crate::message::{Message, Size};
use crate::msg;

/// An OS error by its kind; one Secopy has no words for shows the OS's own.
pub fn io_failure(e: &IoFailure) -> Message {
    use io::ErrorKind as K;
    match e.kind {
        K::NotFound => msg!("errors.os.notFound"),
        K::PermissionDenied => msg!("errors.os.permissionDenied"),
        K::ReadOnlyFilesystem => msg!("errors.os.readOnly"),
        K::StorageFull => msg!("errors.os.full"),
        K::QuotaExceeded => msg!("errors.os.quota"),
        K::FileTooLarge => msg!("errors.os.tooLarge"),
        K::InvalidFilename => msg!("errors.os.badName"),
        K::NotADirectory => msg!("errors.os.notADirectory"),
        K::IsADirectory => msg!("errors.os.isADirectory"),
        K::AlreadyExists => msg!("errors.os.exists"),
        K::DirectoryNotEmpty => msg!("errors.os.notEmpty"),
        K::TimedOut => msg!("errors.os.timedOut"),
        K::UnexpectedEof => msg!("errors.os.endedEarly"),
        K::ResourceBusy => msg!("errors.os.busy"),
        K::NetworkDown | K::NetworkUnreachable | K::HostUnreachable | K::NotConnected => {
            msg!("errors.os.network")
        }
        K::StaleNetworkFileHandle => msg!("errors.os.stale"),
        K::Unsupported => msg!("errors.os.unsupported"),
        _ => msg!("errors.os.unknown", text = e.message.as_str()),
    }
}

pub fn io_error(e: &io::Error) -> Message {
    io_failure(&IoFailure {
        kind: e.kind(),
        message: e.to_string(),
    })
}

pub fn file_error(e: &FileError) -> Message {
    match e {
        FileError::ReadSource(io) => msg!("errors.file.readSource", why = io_failure(io)),
        FileError::WriteDest(io) => msg!("errors.file.writeDest", why = io_failure(io)),
        FileError::ReadBack(io) => msg!("errors.file.readBack", why = io_failure(io)),
        FileError::ChangedWhileRead => msg!("errors.file.changedWhileRead"),
        FileError::HashMismatch { expected, actual } => {
            msg!(
                "errors.file.hashMismatch",
                expected = expected,
                actual = actual
            )
        }
        FileError::AlreadyExists => msg!("errors.file.alreadyExists"),
        FileError::NameClash => msg!("errors.file.nameClash"),
        FileError::PartialInUse => msg!("errors.file.partialInUse"),
        FileError::InvalidName(p) => name_problem(p),
        FileError::TooLarge { limit } => msg!("errors.file.tooLarge", size = Size(*limit)),
        FileError::InTheWay { path } => msg!("errors.file.inTheWay", path = path),
        FileError::SourceChanged => msg!("errors.file.sourceChanged"),
        FileError::Changed { expected, actual } => {
            msg!("errors.file.changed", expected = expected, actual = actual)
        }
        FileError::Missing => msg!("errors.file.missing"),
        FileError::IsLink => msg!("errors.file.isLink"),
        FileError::IsDirectory => msg!("errors.file.isDirectory"),
        FileError::NotAFile => msg!("errors.file.notAFile"),
        FileError::Cancelled => msg!("errors.file.cancelled"),
    }
}

pub fn name_problem(p: &NameProblem) -> Message {
    match p {
        NameProblem::InvalidChar(c) if c.is_control() => msg!("errors.name.controlChar"),
        NameProblem::InvalidChar(c) => msg!("errors.name.invalidChar", char = c.to_string()),
        NameProblem::Reserved => msg!("errors.name.reserved"),
        NameProblem::TrailingDotOrSpace => msg!("errors.name.trailingDotOrSpace"),
        NameProblem::TooLong {
            limit: NameLimit::Bytes(n) | NameLimit::Utf16Units(n),
        } => msg!("errors.name.tooLong", max = *n),
    }
}

pub fn fatal(e: &FatalError) -> Message {
    match e {
        FatalError::DiskFull => msg!("errors.fatal.diskFull"),
        FatalError::DestinationGone => msg!("errors.fatal.destinationGone"),
        FatalError::SourceGone => msg!("errors.fatal.sourceGone"),
    }
}

/// Secopy itself failed (a panic, a lost thread).
pub fn internal() -> Message {
    msg!("errors.internal")
}

pub fn blocker(b: &Blocker) -> Message {
    match b {
        Blocker::DestMissing => msg!("errors.blocker.destMissing"),
        Blocker::DestNotWritable(io) => {
            msg!("errors.blocker.destNotWritable", why = io_failure(io))
        }
        Blocker::DestInsideSource => msg!("errors.blocker.destInsideSource"),
        Blocker::NotEnoughSpace { needed, free } => {
            msg!(
                "errors.blocker.notEnoughSpace",
                needed = Size(*needed),
                free = Size(*free)
            )
        }
    }
}

/// Why something under a source couldn't be read (without its path).
pub fn scan_why(kind: &ScanProblemKind) -> Message {
    match kind {
        ScanProblemKind::NotAFile => msg!("errors.scan.notAFile"),
        ScanProblemKind::Io(io) => io_failure(io),
        ScanProblemKind::Loop => msg!("errors.scan.loop"),
    }
}

/// "{path}: {why}".
pub fn scan_problem(p: &ScanProblem) -> Message {
    msg!(
        "errors.scan.problem",
        path = &p.path,
        why = scan_why(&p.kind)
    )
}

/// Why a source can't be scanned at all.
pub fn scan_error(e: &io::Error) -> Message {
    if e.get_ref().is_some_and(|r| r.is::<DriveRoot>()) {
        msg!("errors.scan.driveRoot")
    } else {
        io_error(e)
    }
}

pub fn check_why(kind: &ProblemKind) -> Message {
    match kind {
        ProblemKind::NotALine => msg!("errors.check.notALine"),
        ProblemKind::NotAChecksum { hex } => msg!("errors.check.notAChecksum", hex = hex),
        ProblemKind::PathUnreadable => msg!("errors.check.pathUnreadable"),
        ProblemKind::Unreadable(Some(io), _) => {
            msg!("errors.check.unreadable", why = io_failure(io))
        }
        ProblemKind::Unreadable(None, _) => {
            msg!("errors.check.unreadable", why = msg!("errors.scan.loop"))
        }
        ProblemKind::Outside { path } => msg!("errors.check.outside", path = path),
    }
}

/// "{file}:{line}: {why}", or "{file}: {why}". A line number is text: no separators.
pub fn check_problem(p: &check::Problem) -> Message {
    let why = check_why(&p.kind);
    match p.line {
        Some(line) => msg!(
            "errors.check.atLine",
            file = &p.file,
            line = line.to_string(),
            why = why
        ),
        None => msg!("errors.check.inFile", file = &p.file, why = why),
    }
}

pub fn guard(g: &Guard) -> Message {
    match g {
        Guard::Unread { count, first } => msg!(
            "mirror.guard.unread",
            count = *count,
            path = &first.path,
            why = scan_why(&first.kind),
        ),
        Guard::EmptyOrigin => msg!("mirror.guard.emptyOrigin"),
        Guard::TooMany { removals, files } => {
            msg!("mirror.guard.tooMany", removals = *removals, files = *files)
        }
    }
}

pub fn plan_error(e: &PlanError) -> Message {
    match e {
        PlanError::OriginMissing(origin) => msg!("errors.mirror.originMissing", path = origin),
        PlanError::Same => msg!("errors.mirror.same"),
        PlanError::DestinationInOrigin => msg!("errors.mirror.destinationInOrigin"),
        PlanError::OriginInDestination => msg!("errors.mirror.originInDestination"),
        PlanError::ArchiveNotDir => msg!("errors.mirror.archiveNotDir"),
        PlanError::Scan {
            drive_root: true, ..
        } => msg!("errors.scan.driveRoot"),
        PlanError::Scan { io, .. } => io_failure(io),
        PlanError::Blocked(b) => blocker(b),
        PlanError::Cancelled => msg!("errors.mirror.previewCancelled"),
    }
}

pub fn not_removed(e: &NotRemoved) -> Message {
    match e {
        NotRemoved::Cancelled => msg!("mirror.notRemoved.cancelled"),
        NotRemoved::Stopped => msg!("mirror.notRemoved.stopped"),
        NotRemoved::Failed(n) => msg!("mirror.notRemoved.failed", count = *n),
        NotRemoved::NotClean => msg!("mirror.notRemoved.notClean"),
    }
}

pub fn removal(e: &RemovalError) -> Message {
    match e {
        RemovalError::ChangedAfterPreview => msg!("mirror.removal.changed"),
        RemovalError::Io(io) => io_failure(io),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::path::PathBuf;

    use secopy_core::check::{self, ProblemKind};
    use secopy_core::error::{FatalError, FileError, IoFailure};
    use secopy_core::fsinfo::NameLimit;
    use secopy_core::mirror::{Guard, NotRemoved, PlanError, RemovalError};
    use secopy_core::names::NameProblem;
    use secopy_core::preflight::Blocker;
    use secopy_core::scan::{ScanProblem, ScanProblemKind};

    use crate::message::{Arg, Message};


    #[test]
    fn a_quota_isnt_a_full_drive() {
        assert_eq!(io_failure(&failure(io::ErrorKind::QuotaExceeded, "x")).key, "errors.os.quota");
    }

    fn failure(kind: io::ErrorKind, message: &str) -> IoFailure {
        IoFailure {
            kind,
            message: message.into(),
        }
    }

    fn text(m: &Message, name: &str) -> String {
        match &m.args[name] {
            Arg::Text(t) => t.clone(),
            other => panic!("{name} is {other:?}"),
        }
    }

    fn nested(m: &Message, name: &str) -> Message {
        match &m.args[name] {
            Arg::Message(inner) => (**inner).clone(),
            other => panic!("{name} is {other:?}"),
        }
    }

    #[test]
    fn os_errors_by_kind_and_the_rest_as_they_are() {
        assert_eq!(
            io_failure(&failure(
                io::ErrorKind::PermissionDenied,
                "Permission denied (os error 13)"
            ))
            .key,
            "errors.os.permissionDenied"
        );
        assert_eq!(
            io_failure(&failure(io::ErrorKind::StorageFull, "x")).key,
            "errors.os.full"
        );
        assert_eq!(
            io_failure(&failure(io::ErrorKind::NotFound, "x")).key,
            "errors.os.notFound"
        );
        let other = io_failure(&failure(
            io::ErrorKind::Other,
            "Something odd (os error 99)",
        ));
        assert_eq!(other.key, "errors.os.unknown");
        assert_eq!(text(&other, "text"), "Something odd (os error 99)");
        assert_eq!(
            io_error(&io::Error::from(io::ErrorKind::ReadOnlyFilesystem)).key,
            "errors.os.readOnly"
        );
    }

    #[test]
    fn file_errors_keep_their_details() {
        let m = file_error(&FileError::HashMismatch {
            expected: "a1".into(),
            actual: "b2".into(),
        });
        assert_eq!(m.key, "errors.file.hashMismatch");
        assert_eq!(
            (text(&m, "expected"), text(&m, "actual")),
            ("a1".into(), "b2".into())
        );
        let m = file_error(&FileError::ReadSource(failure(
            io::ErrorKind::PermissionDenied,
            "x",
        )));
        assert_eq!(m.key, "errors.file.readSource");
        assert_eq!(nested(&m, "why").key, "errors.os.permissionDenied");
        let m = file_error(&FileError::InTheWay {
            path: PathBuf::from("/Volumes/A/x"),
        });
        assert_eq!(text(&m, "path"), "/Volumes/A/x");
        assert_eq!(
            file_error(&FileError::TooLarge {
                limit: 4_294_967_295
            })
            .args["size"],
            Arg::Size {
                bytes: 4_294_967_295.0
            }
        );
        assert_eq!(
            file_error(&FileError::ChangedWhileRead).key,
            "errors.file.changedWhileRead"
        );
        let m = file_error(&FileError::InvalidName(NameProblem::InvalidChar(':')));
        assert_eq!(m.key, "errors.name.invalidChar");
        assert_eq!(text(&m, "char"), ":");
        assert_eq!(
            name_problem(&NameProblem::InvalidChar('\u{1}')).key,
            "errors.name.controlChar"
        );
        assert_eq!(
            name_problem(&NameProblem::TooLong {
                limit: NameLimit::Bytes(255)
            })
            .args["max"],
            Arg::Number(255.0)
        );
    }

    #[test]
    fn what_stops_a_job_and_what_blocks_it() {
        assert_eq!(fatal(&FatalError::DiskFull).key, "errors.fatal.diskFull");
        assert_eq!(
            fatal(&FatalError::SourceGone).key,
            "errors.fatal.sourceGone"
        );
        let m = blocker(&Blocker::NotEnoughSpace {
            needed: 2_000,
            free: 1_000,
        });
        assert_eq!(m.key, "errors.blocker.notEnoughSpace");
        assert_eq!(m.args["needed"], Arg::Size { bytes: 2_000.0 });
        assert_eq!(m.args["free"], Arg::Size { bytes: 1_000.0 });
        let m = blocker(&Blocker::DestNotWritable(failure(
            io::ErrorKind::ReadOnlyFilesystem,
            "x",
        )));
        assert_eq!(nested(&m, "why").key, "errors.os.readOnly");
    }

    #[test]
    fn scan_and_check_problems_name_their_place() {
        let p = ScanProblem {
            path: PathBuf::from("/a/b"),
            message: "x".into(),
            kind: ScanProblemKind::NotAFile,
        };
        let m = scan_problem(&p);
        assert_eq!(
            (m.key.as_str(), text(&m, "path")),
            ("errors.scan.problem", "/a/b".into())
        );
        assert_eq!(nested(&m, "why").key, "errors.scan.notAFile");
        let drive_root =
            std::io::Error::new(io::ErrorKind::InvalidInput, secopy_core::scan::DriveRoot);
        assert_eq!(scan_error(&drive_root).key, "errors.scan.driveRoot");
        let p = check::Problem {
            file: PathBuf::from("a.xxh64"),
            line: Some(1234),
            reason: "x".into(),
            kind: ProblemKind::NotAChecksum { hex: "zz".into() },
        };
        let m = check_problem(&p);
        assert_eq!(m.key, "errors.check.atLine");
        assert_eq!(
            text(&m, "line"),
            "1234",
            "a line number isn't a count: no separators"
        );
        assert_eq!(text(&nested(&m, "why"), "hex"), "zz");
    }

    #[test]
    fn a_mirror_says_why_it_cant_run_or_remove() {
        assert_eq!(
            plan_error(&PlanError::Cancelled).key,
            "errors.mirror.previewCancelled"
        );
        assert_eq!(plan_error(&PlanError::Same).key, "errors.mirror.same");
        assert_eq!(
            plan_error(&PlanError::Blocked(Blocker::DestMissing)).key,
            "errors.blocker.destMissing"
        );
        let first = ScanProblem {
            path: PathBuf::from("/o/x"),
            message: "x".into(),
            kind: ScanProblemKind::Loop,
        };
        let m = guard(&Guard::Unread { count: 3, first });
        assert_eq!(
            (m.key.as_str(), m.args["count"].clone()),
            ("mirror.guard.unread", Arg::Number(3.0))
        );
        assert_eq!(
            guard(&Guard::TooMany {
                removals: 2,
                files: 3
            })
            .args["files"],
            Arg::Number(3.0)
        );
        assert_eq!(
            not_removed(&NotRemoved::Failed(2)).args["count"],
            Arg::Number(2.0)
        );
        assert_eq!(
            removal(&RemovalError::ChangedAfterPreview).key,
            "mirror.removal.changed"
        );
        assert_eq!(internal().key, "errors.internal");
    }
}
