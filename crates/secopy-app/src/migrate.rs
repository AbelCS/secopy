//! Moves what Secopy 0.2.0 saved under its old identifier (RFD §14, 2026-09-27).

use std::fs;
use std::path::Path;

/// The identifier of Secopy 0.2.0; its data folder sits next to the current one.
pub const OLD_IDENTIFIER: &str = "com.belisoft.secopy";

/// Moves `old_data/reports` to `new_data/reports` when only the old one exists. `Ok(true)`
/// when something moved.
pub fn move_old_reports(old_data: &Path, new_data: &Path) -> Result<bool, String> {
    let (from, to) = (old_data.join("reports"), new_data.join("reports"));
    if !from.is_dir() || to.exists() {
        return Ok(false);
    }
    fs::create_dir_all(new_data).map_err(|e| format!("{}: {e}", new_data.display()))?;
    fs::rename(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_reports_move_to_the_new_folder_once() {
        let dir = tempfile::tempdir().unwrap();
        let (old, new) = (dir.path().join(OLD_IDENTIFIER), dir.path().join("new"));
        fs::create_dir_all(old.join("reports")).unwrap();
        fs::write(old.join("reports/a_report.txt"), b"report").unwrap();
        assert_eq!(move_old_reports(&old, &new), Ok(true));
        assert_eq!(
            fs::read(new.join("reports/a_report.txt")).unwrap(),
            b"report"
        );
        assert!(!old.join("reports").exists());
        assert_eq!(
            move_old_reports(&old, &new),
            Ok(false),
            "nothing left to move"
        );
    }

    #[test]
    fn existing_new_reports_are_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let (old, new) = (dir.path().join("old"), dir.path().join("new"));
        fs::create_dir_all(old.join("reports")).unwrap();
        fs::create_dir_all(new.join("reports")).unwrap();
        assert_eq!(move_old_reports(&old, &new), Ok(false));
        assert!(old.join("reports").is_dir());
    }
}
