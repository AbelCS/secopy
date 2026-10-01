//! Secopy engine: scan, copy, verify and write checksum files (RFD 0001).
//! UI-independent; used by the desktop app, the CLI, tests and benchmarks.

#[cfg(not(target_os = "macos"))]
compile_error!("Secopy supports macOS only (issue #60).");

pub mod awake;
pub mod check;
pub mod checksum_file;
pub mod control;
pub mod copy;
pub mod error;
pub mod filter;
pub mod fsinfo;
pub mod hash;
pub mod job;
mod metadata;
pub mod mhl;
pub mod mirror;
pub mod names;
mod os;
pub mod plan;
pub mod preflight;
pub mod report;
pub mod scan;
pub mod source;
pub mod system;
pub mod verify;
