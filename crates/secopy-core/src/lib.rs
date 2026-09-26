//! Secopy engine: scan, copy, verify and write checksum files (RFD 0001).
//! UI-independent; used by the desktop app, the CLI, tests and benchmarks.

pub mod filter;
pub mod hash;
pub mod hidden;
pub mod scan;
pub mod source;
