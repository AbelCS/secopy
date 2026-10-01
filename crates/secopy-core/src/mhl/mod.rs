//! ASC MHL v2.0 (#154): the media industry's proof of copy, kept in an `ascmhl` folder.

pub mod c4;
pub mod ignore;

/// The history's folder, at the root of the folder it covers.
pub const FOLDER: &str = "ascmhl";
/// The chain file inside it.
pub const CHAIN: &str = "ascmhl_chain.xml";
