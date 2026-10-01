//! Secopy's own unfinished copies. What else is left out of a copy is the ignore list's
//! (`crate::ignore`, #158).

/// Secopy's own unfinished copies: `.name.secopy-partial`, `.secopy-<hash>.partial`.
pub fn is_secopy_partial(name: &str) -> bool {
    (name.starts_with('.') && name.ends_with(".secopy-partial"))
        || (name.starts_with(".secopy-") && name.ends_with(".partial"))
}
