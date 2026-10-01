//! ASC MHL v2.0 (#154): the media industry's proof of copy, kept in an `ascmhl` folder.

pub mod c4;
pub mod ignore;
pub mod prepare;
pub mod read;
pub mod run;
pub mod write;

/// The history's folder, at the root of the folder it covers.
pub const FOLDER: &str = "ascmhl";
/// The chain file inside it.
pub const CHAIN: &str = "ascmhl_chain.xml";

/// What a copy job needs to record its ASC MHL (`JobOptions::mhl`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MhlJob {
    pub plan: prepare::MhlPlan,
    /// This Secopy's version, for the manifest's `tool`.
    pub tool_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// The first hash recorded for this file.
    Original,
    /// The same as the first `original` hash the history has for it.
    Verified,
    /// Not the same: the file changed since it was first recorded.
    Failed,
}

/// One file in a generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// `/`-separated, relative to the scope.
    pub rel: String,
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
    pub xxh64: u64,
    pub action: Action,
}

/// A nested history's generation written in the same job: its manifest's path relative to
/// this scope, and its C4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub path: String,
    pub c4: String,
}

/// One line of a chain file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainEntry {
    pub sequence: u32,
    pub file: String,
    pub c4: String,
}

/// What one generation says, before it's written.
#[derive(Debug, Clone)]
pub struct Generation {
    pub created: chrono::DateTime<chrono::Local>,
    pub hostname: String,
    pub tool_version: String,
    pub ignore: Vec<String>,
    pub records: Vec<Record>,
    pub references: Vec<Reference>,
}
