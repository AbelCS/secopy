# Engine Core (M0) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Secopy engine (`secopy-core`) and a developer CLI (`secopy-cli`). Together they scan a source, copy with inline xxHash64, verify from the device, and write an `xxhsum`-compatible checksum file. Then benchmark them against `cp`/`rsync`/`robocopy` to prove the RFD speed targets before any UI work.

**Architecture:** A Cargo workspace with two crates. `secopy-core` is a UI-independent library. `scan` builds a `Selection`. `copy` streams source → `.name.secopy-partial` through a reader/writer thread pair, hashing as it reads. `verify` re-reads the partial file with the OS cache bypassed. `job` runs small-file and large-file copy lanes plus verify lanes, and emits progress events. `secopy-cli` is a thin front-end used for manual testing and benchmarks. The Tauri app (plan 3) will call the same `job::run_job` API.

**Tech Stack:** Rust stable (edition 2024), `xxhash-rust` (xxh64), `walkdir`, `chrono`, `thiserror`, `libc` (Unix), `clap`, `ctrlc`, `tempfile` (tests), GitHub Actions, `hyperfine` (benchmarks).

**Spec:** [docs/rfd/0001-secopy.md](../../rfd/0001-secopy.md). Roadmap: [2026-09-26-v1-roadmap.md](2026-09-26-v1-roadmap.md).

## Global Constraints

- Work on `main`. Conventional Commits exactly as in `CLAUDE.md`. One commit per logical change, using the messages given in each task.
- Before every commit, run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`. All three must pass.
- Toolchain: `rust-toolchain.toml` pins `channel = "stable"`. Crates use `edition = "2024"` and `rust-version = "1.88"`.
- Versions are managed by release-please only. Crates inherit `version.workspace = true` and the workspace version stays `0.0.0`. Never edit it by hand.
- Hashes: xxHash64, **seed 0**, shown as **16 lowercase hex chars** (canonical big-endian, as `xxhsum` prints).
- Checksum file name: `secopy_YYYY-MM-DD_HHMMSS.xxh64`, local time, written to the destination directory. Lines are `<hash><two spaces><relative/path>`, sorted, UTF-8, LF line endings, no comments.
- A file only ever appears under its final name once it is complete: write `.<name>.secopy-partial`, flush, then rename (FR-18).
- **Never overwrite** an existing destination file in this plan. It fails with `FileError::AlreadyExists`. Conflict policies come in plan 2.
- `secopy-core` has no UI dependencies and no network access.
- Every new source file starts with a `//!` doc comment that cites the RFD requirement IDs it implements.

## Review Focus

These are the input classes most likely to hurt a real user that the feature tests below don't cover on their own. Each one has a pinning test in the task named.

1. **Two selected files mapping to the same destination name.** This includes names that differ only in case (`x/a.txt` + `y/A.TXT` picked as loose files, or a Linux source going to a case-insensitive destination). The first file is copied, the others fail with `NameClash`, and no file is ever a corrupted mix. Pinned in Task 7 (`files_that_map_to_the_same_name_never_mix`).
2. **A destination that can't be written** (an unplugged drive, read-only media, or a path that is a file). Every file fails with a write error, the job ends, and no checksum file is written. No panic, no hang. Pinned in Task 7 (`an_unwritable_destination_fails_every_file_without_hanging`).
3. **Non-ASCII names:** combining accents (NFD, as macOS produces), CJK, emoji, spaces. Copies are byte-identical, and names appear as UTF-8 in the checksum file. Pinned in Task 7 (`unicode_names_round_trip`) and Task 4 (`xxhsum_accepts_our_checksum_file`).
4. **Cancelling in the middle of a job.** Finished files are kept and listed in the checksum file, no `.secopy-partial` files remain, and the rest are reported as not started. Pinned in Task 7 (`cancelling_mid_job_keeps_finished_files_and_removes_partials`).
5. **File sizes on buffer boundaries** (size −1, = size, +1, 2× size, 2× size +1), where the small-file path and the pipelined path meet. Copies are exact. Pinned in Task 5 (`sizes_around_the_buffer_size_copy_exactly`).

---

### Task 1: Workspace, toolchain, CI, release plumbing and xxHash64 helpers

**Files:**
- Create: `rust-toolchain.toml`, `Cargo.toml`, `crates/secopy-core/Cargo.toml`, `crates/secopy-core/src/lib.rs`, `crates/secopy-core/src/hash.rs`, `.github/workflows/ci.yml`, `README.md`
- Modify: `release-please-config.json`, `.github/workflows/release-please.yml`, `CLAUDE.md`
- Generated: `Cargo.lock` (commit it)

**Interfaces:**
- Consumes: nothing.
- Produces: `secopy_core::hash::{Xxh64, SEED: u64, hasher() -> Xxh64, hash_bytes(&[u8]) -> u64, to_hex(u64) -> String}`.

- [ ] **Step 1: Update Rust and pin the toolchain**

Run: `rustup update stable && rustc --version`
Expected: `rustc 1.98.1` or newer. Anything ≥ 1.88 works.

Create `rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 2: Create the workspace manifests**

`Cargo.toml` (workspace root). `[workspace.dependencies]` lists every dependency this plan uses, so later tasks don't touch it:

```toml
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
version = "0.0.0"
edition = "2024"
rust-version = "1.88"
publish = false

[workspace.dependencies]
secopy-core = { path = "crates/secopy-core" }
chrono = { version = "0.4.45", default-features = false, features = ["clock"] }
clap = { version = "4.6.7", features = ["derive"] }
ctrlc = "3.5.2"
libc = "0.2"
tempfile = "3.27.0"
thiserror = "2.0.21"
walkdir = "2.5.0"
xxhash-rust = { version = "0.8.18", features = ["xxh64"] }
```

`crates/secopy-core/Cargo.toml`:

```toml
[package]
name = "secopy-core"
description = "Secopy engine: scan, copy, verify and checksum files"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true

[dependencies]
chrono.workspace = true
thiserror.workspace = true
walkdir.workspace = true
xxhash-rust.workspace = true

[target.'cfg(unix)'.dependencies]
libc.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

`crates/secopy-core/src/lib.rs` (later tasks add one `mod` line each):

```rust
//! Secopy engine: scan, copy, verify and write checksum files (RFD 0001).
//! UI-independent; used by the desktop app, the CLI, tests and benchmarks.

pub mod hash;
```

- [ ] **Step 3: Write the failing tests**

Create `crates/secopy-core/src/hash.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_reference_vectors() {
        assert_eq!(hash_bytes(b""), 0xef46_db37_51d8_e999);
        assert_eq!(hash_bytes(b"abc"), 0x44bc_2cf5_ad77_0999);
    }

    #[test]
    fn streaming_matches_one_shot() {
        let data = b"secopy streaming hash";
        let mut h = hasher();
        h.update(&data[..5]);
        h.update(&data[5..]);
        assert_eq!(h.digest(), hash_bytes(data));
    }

    #[test]
    fn hex_is_16_lowercase_chars() {
        assert_eq!(to_hex(0xAB), "00000000000000ab");
        assert_eq!(to_hex(0xef46_db37_51d8_e999), "ef46db3751d8e999");
    }
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test -p secopy-core hash`
Expected: compile error, `cannot find function 'hash_bytes' in this scope` (and `hasher`, `to_hex`).

- [ ] **Step 5: Implement the helpers**

Put this at the top of `crates/secopy-core/src/hash.rs`, above the test module:

```rust
//! xxHash64 helpers. Secopy always uses seed 0 and the canonical
//! big-endian lowercase hex form that `xxhsum` prints (RFD §4).

pub use xxhash_rust::xxh64::Xxh64;

/// Seed used for every hash.
pub const SEED: u64 = 0;

/// Creates a streaming hasher with the Secopy seed.
pub fn hasher() -> Xxh64 {
    Xxh64::new(SEED)
}

/// Hashes a byte slice in one go.
pub fn hash_bytes(bytes: &[u8]) -> u64 {
    xxhash_rust::xxh64::xxh64(bytes, SEED)
}

/// Canonical form: 16 lowercase hex characters, as printed by `xxhsum`.
pub fn to_hex(hash: u64) -> String {
    format!("{hash:016x}")
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p secopy-core hash`
Expected: `3 passed`. The reference vectors (`""` → `ef46db3751d8e999`, `"abc"` → `44bc2cf5ad770999`) confirm the seed and hex format.

- [ ] **Step 7: Commit the workspace**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add rust-toolchain.toml Cargo.toml Cargo.lock crates/
git commit -m "feat(core): add cargo workspace and xxhash64 helpers"
```

- [ ] **Step 8: Add the CI workflow**

Create `.github/workflows/ci.yml`. It runs lint on Linux, and tests on Linux, macOS and Windows. The xxhsum compatibility test (Task 4) is forced to run where xxhsum is installed:

```yaml
name: ci

on:
  push:
    branches:
      - main
  pull_request:

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-targets --locked -- -D warnings

  test:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v7
      - uses: Swatinem/rust-cache@v2
      - name: Install xxhsum (Linux)
        if: runner.os == 'Linux'
        run: sudo apt-get update && sudo apt-get install -y xxhash
      - name: Install xxhsum (macOS)
        if: runner.os == 'macOS'
        run: brew install xxhash
      - run: cargo test --workspace --locked
        env:
          # The xxhsum compatibility test must not skip where xxhsum is installed.
          SECOPY_REQUIRE_XXHSUM: ${{ runner.os != 'Windows' && '1' || '0' }}
```

- [ ] **Step 9: Update release-please**

Replace `release-please-config.json`. The new `extra-files` entry keeps the workspace version in `Cargo.toml` in step with each release:

```json
{
  "$schema": "https://raw.githubusercontent.com/googleapis/release-please/main/schemas/config.json",
  "packages": {
    ".": {
      "release-type": "simple",
      "package-name": "secopy",
      "changelog-path": "CHANGELOG.md",
      "include-v-in-tag": true,
      "include-component-in-tag": false,
      "extra-files": [
        {
          "type": "toml",
          "path": "Cargo.toml",
          "jsonpath": "$.workspace.package.version"
        }
      ]
    }
  },
  "bump-minor-pre-major": true
}
```

Replace `.github/workflows/release-please.yml`. It bumps the action to v5 and refreshes `Cargo.lock` on the release PR, because release-please doesn't touch lock files:

```yaml
name: release-please

on:
  push:
    branches:
      - main

permissions:
  contents: write
  pull-requests: write
  issues: write

jobs:
  release-please:
    runs-on: ubuntu-latest
    steps:
      - uses: googleapis/release-please-action@v5
        id: release
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
          config-file: release-please-config.json
          manifest-file: .release-please-manifest.json
      # release-please bumps the version in Cargo.toml but not in Cargo.lock.
      # Refresh the lock file on the release PR so `--locked` builds keep working.
      - if: ${{ steps.release.outputs.pr }}
        uses: actions/checkout@v7
        with:
          ref: ${{ fromJSON(steps.release.outputs.pr).headBranchName }}
      - if: ${{ steps.release.outputs.pr }}
        name: Sync Cargo.lock
        run: |
          cargo update --workspace
          if ! git diff --quiet Cargo.lock; then
            git config user.name "github-actions[bot]"
            git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
            git commit -am "chore: sync cargo.lock"
            git push
          fi
```

After the first push, open the release-please run log. If the release PR exists but the "Sync Cargo.lock" steps were skipped, the action's output is not named `pr` in v5. Check the action's README for the new name and fix the `if:` conditions.

- [ ] **Step 10: Commit CI and release changes**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add lint and cross-platform test workflow"
git add release-please-config.json .github/workflows/release-please.yml
git commit -m "ci: bump release-please to v5 and sync cargo.lock on release prs"
```

- [ ] **Step 11: Add README and development notes**

Create `README.md`:

````markdown
# Secopy

Fast, verified file copies for macOS, Linux and Windows. Copy a folder or a set of
files, optionally verify every copy with xxHash64, and get an `xxhsum`-compatible
checksum file in the destination.

> **Status:** early development. The engine and a developer CLI come first; the desktop
> app follows. Design: [RFD 0001](docs/rfd/0001-secopy.md).

## Development

Requirements: Rust via [rustup](https://rustup.rs) (the toolchain is pinned in
`rust-toolchain.toml`), and `xxhsum` for the checksum compatibility test
(`brew install xxhash` or `apt install xxhash`).

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Try the CLI:

```sh
cargo run --release -p secopy-cli -- /path/to/CARD --to /path/to/backup --verify
```
````

Append to `CLAUDE.md`:

```markdown

## Development

- Layout: `crates/secopy-core` (engine library, no UI dependencies) and `crates/secopy-cli`
  (developer CLI and benchmark driver). The Tauri app and Svelte UI come in a later plan.
- Before every commit: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace`. All must pass.
- `xxhsum` must be installed locally (`brew install xxhash`). CI runs its compatibility
  test with `SECOPY_REQUIRE_XXHSUM=1` so it can't silently skip.
- Plans live in `docs/superpowers/plans/`, benchmark results in `docs/benchmarks/`.
```

```bash
git add README.md CLAUDE.md
git commit -m "docs: add readme and development commands"
```

---

### Task 2: Source model, extension filter and hidden-file detection

**Files:**
- Create: `crates/secopy-core/src/source.rs`, `crates/secopy-core/src/filter.rs`, `crates/secopy-core/src/hidden.rs`
- Modify: `crates/secopy-core/src/lib.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `source::{Source::{Directory { path: PathBuf, mode: DirMode }, Files(Vec<PathBuf>)}, DirMode::{FolderItself, ContentsOnly}}`
  - `filter::{ExtKey = Option<String>, NO_EXTENSION: &str = "(none)", ext_key(&Path) -> ExtKey, ExtensionFilter::{All, Only(BTreeSet<ExtKey>)}}`, with `matches(&self, &ExtKey) -> bool`, `is_active(&self) -> bool` and `parse_list(&str) -> Self`
  - `hidden::is_hidden(&Path, &Metadata) -> bool`

- [ ] **Step 1: Register the modules**

`crates/secopy-core/src/lib.rs` becomes:

```rust
//! Secopy engine: scan, copy, verify and write checksum files (RFD 0001).
//! UI-independent; used by the desktop app, the CLI, tests and benchmarks.

pub mod filter;
pub mod hash;
pub mod hidden;
pub mod source;
```

- [ ] **Step 2: Write the source model** (plain types, no behaviour to test)

`crates/secopy-core/src/source.rs`:

```rust
//! What the user picked as source (RFD §6.1).

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// One directory, copied recursively (FR-1, FR-6).
    Directory { path: PathBuf, mode: DirMode },
    /// One or more files, copied flat into the destination (FR-2, FR-5).
    Files(Vec<PathBuf>),
}

/// How a directory source lands in the destination (FR-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirMode {
    /// `DEST/<SOURCE_NAME>/…`
    FolderItself,
    /// `DEST/…`
    ContentsOnly,
}
```

- [ ] **Step 3: Write the failing tests**

`crates/secopy-core/src/filter.rs`, test module only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> ExtKey {
        Some(s.to_string())
    }

    #[test]
    fn ext_key_is_lowercase_last_extension() {
        assert_eq!(ext_key(Path::new("A001.MOV")), key("mov"));
        assert_eq!(ext_key(Path::new("backup.tar.gz")), key("gz"));
        assert_eq!(ext_key(Path::new("README")), None);
        assert_eq!(ext_key(Path::new("trailing.")), None);
    }

    #[test]
    fn all_matches_everything_and_is_inactive() {
        assert!(ExtensionFilter::All.matches(&None));
        assert!(ExtensionFilter::All.matches(&key("mov")));
        assert!(!ExtensionFilter::All.is_active());
    }

    #[test]
    fn parse_list_normalises_entries() {
        let f = ExtensionFilter::parse_list(" MOV, .wav ,(none),");
        assert!(f.is_active());
        assert!(f.matches(&key("mov")));
        assert!(f.matches(&key("wav")));
        assert!(f.matches(&None));
        assert!(!f.matches(&key("xml")));
    }
}
```

`crates/secopy-core/src/hidden.rs`, test module only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn check(name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        fs::write(&path, b"x").unwrap();
        (dir, path)
    }

    #[test]
    fn dot_names_are_hidden() {
        let (_dir, path) = check(".DS_Store");
        assert!(is_hidden(&path, &fs::metadata(&path).unwrap()));
    }

    #[test]
    fn plain_names_are_visible() {
        let (_dir, path) = check("A001.mov");
        assert!(!is_hidden(&path, &fs::metadata(&path).unwrap()));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_hidden_flag_is_detected() {
        let (_dir, path) = check("flagged.mov");
        let status = std::process::Command::new("chflags")
            .arg("hidden")
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(is_hidden(&path, &fs::metadata(&path).unwrap()));
    }

    #[cfg(windows)]
    #[test]
    fn windows_hidden_attribute_is_detected() {
        let (_dir, path) = check("flagged.mov");
        let status = std::process::Command::new("attrib")
            .arg("+h")
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(is_hidden(&path, &fs::metadata(&path).unwrap()));
    }
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test -p secopy-core --lib`
Expected: compile errors, `cannot find function 'ext_key'`, `cannot find type 'ExtensionFilter'`, `cannot find function 'is_hidden'`.

- [ ] **Step 5: Implement the filter**

Top of `crates/secopy-core/src/filter.rs`:

```rust
//! Extension keys and the extension filter (FR-7..FR-11).

use std::collections::BTreeSet;
use std::path::Path;

/// Lowercase last extension without the dot, or `None` for files without one (FR-9).
pub type ExtKey = Option<String>;

/// How "no extension" is written in lists such as the CLI `--ext` flag.
pub const NO_EXTENSION: &str = "(none)";

pub fn ext_key(path: &Path) -> ExtKey {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .filter(|e| !e.is_empty())
}

/// Which extensions to copy. Use `All` when every extension is selected, so
/// empty source directories are still recreated (FR-10).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ExtensionFilter {
    #[default]
    All,
    Only(BTreeSet<ExtKey>),
}

impl ExtensionFilter {
    pub fn matches(&self, key: &ExtKey) -> bool {
        match self {
            ExtensionFilter::All => true,
            ExtensionFilter::Only(keys) => keys.contains(key),
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, ExtensionFilter::Only(_))
    }

    /// Parses a comma-separated list such as `"MOV, .wav, (none)"` (FR-11).
    pub fn parse_list(list: &str) -> Self {
        let keys = list
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                if s == NO_EXTENSION {
                    None
                } else {
                    Some(s.trim_start_matches('.').to_lowercase())
                }
            })
            .collect();
        ExtensionFilter::Only(keys)
    }
}
```

- [ ] **Step 6: Implement hidden detection**

Top of `crates/secopy-core/src/hidden.rs`:

```rust
//! Hidden file detection (FR-12).

use std::fs::Metadata;
use std::path::Path;

/// A file or directory is hidden if its name starts with `.`, or the OS marks it
/// hidden: `UF_HIDDEN` on macOS, `HIDDEN`/`SYSTEM` attributes on Windows.
pub fn is_hidden(path: &Path, meta: &Metadata) -> bool {
    let dot_name = path
        .file_name()
        .is_some_and(|n| n.as_encoded_bytes().first() == Some(&b'.'));
    dot_name || os_hidden(meta)
}

#[cfg(target_os = "macos")]
fn os_hidden(meta: &Metadata) -> bool {
    use std::os::macos::fs::MetadataExt;
    const UF_HIDDEN: u32 = 0x8000;
    meta.st_flags() & UF_HIDDEN != 0
}

#[cfg(windows)]
fn os_hidden(meta: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
    meta.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
}

#[cfg(not(any(target_os = "macos", windows)))]
fn os_hidden(_meta: &Metadata) -> bool {
    false
}
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p secopy-core`
Expected: all pass. That's 9 on macOS and Windows (3 hash, 3 filter, 3 hidden) and 8 on Linux, which has no hidden flag to test.

- [ ] **Step 8: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/secopy-core/src
git commit -m "feat(core): add source model, extension filter and hidden detection"
```

---

### Task 3: Scan and selection

**Files:**
- Create: `crates/secopy-core/src/scan.rs`, `crates/secopy-core/tests/common/mod.rs`, `crates/secopy-core/tests/scan.rs`
- Modify: `crates/secopy-core/src/lib.rs` (add `pub mod scan;` in alphabetical order)

**Interfaces:**
- Consumes: `Source`, `DirMode`, `ExtKey`, `ExtensionFilter`, `ext_key`, `is_hidden` (Task 2).
- Produces:
  - `scan::ScanOptions { include_hidden: bool }`, which derives `Default`
  - `scan::ScanEntry { source: PathBuf, rel: PathBuf, size: u64, ext: ExtKey }`. `rel` is relative to the **destination** and already includes the folder name in `FolderItself` mode.
  - `scan::ExtStat { files: u64, bytes: u64 }`, `scan::ScanProblem { path: PathBuf, message: String }`
  - `scan::Scan { files, root_dir: Option<PathBuf>, empty_dirs: Vec<PathBuf>, ext_stats: BTreeMap<ExtKey, ExtStat>, skipped_hidden: u64, skipped_symlinks: Vec<PathBuf>, problems: Vec<ScanProblem> }`
  - `scan::Selection { files: Vec<ScanEntry>, dirs: Vec<PathBuf>, total_bytes: u64 }`
  - `scan::scan(&Source, &ScanOptions) -> io::Result<Scan>` and `Scan::select(&self, &ExtensionFilter) -> Selection`
  - Test helpers `common::{write_files(&Path, &[(&str, &[u8])]), read_tree(&Path) -> BTreeMap<String, Vec<u8>>, pattern(usize) -> Vec<u8>}`. Later test files reuse these through `mod common;`.

Behaviour to keep in mind:
- The walk is sorted by name and never follows symlinks. Symlinks are listed in `skipped_symlinks`.
- A hidden folder is skipped as a whole and counts as one item in `skipped_hidden`.
- The **source root itself is never treated as hidden**. walkdir passes depth-0 entries to `filter_entry` even with `min_depth(1)`.
- Files picked one by one are copied flat and are included even if hidden, because the user chose them explicitly.
- `select` recreates empty source directories only when the filter is `All` (FR-10).

- [ ] **Step 1: Write the test helpers**

`crates/secopy-core/tests/common/mod.rs`:

```rust
//! Helpers shared by the integration tests.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use walkdir::WalkDir;

/// Creates files (and their parent folders) under `root`.
pub fn write_files(root: &Path, files: &[(&str, &[u8])]) {
    for (rel, data) in files {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }
}

/// Every file under `root` as `slash/path → contents`, excluding `.xxh64` checksum files.
pub fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    WalkDir::new(root)
        .into_iter()
        .map(Result::unwrap)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_none_or(|x| x != "xxh64"))
        .map(|e| {
            let rel = e.path().strip_prefix(root).unwrap();
            let key = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            (key, fs::read(e.path()).unwrap())
        })
        .collect()
}

/// `n` bytes of a repeating, non-trivial pattern.
pub fn pattern(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 31 % 251) as u8).collect()
}
```

- [ ] **Step 2: Write the failing tests**

`crates/secopy-core/tests/scan.rs`:

```rust
mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use common::write_files;
use secopy_core::filter::ExtensionFilter;
use secopy_core::scan::{ExtStat, Scan, ScanOptions, scan};
use secopy_core::source::{DirMode, Source};

/// CARD/
///   A001.MOV, sound.wav, README, clips/B002.mov, clips/empty/,
///   .DS_Store, .hidden_dir/inner.mov   (hidden)
fn card() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let card = dir.path().join("CARD");
    write_files(
        &card,
        &[
            ("A001.MOV", b"mov-a"),
            ("sound.wav", b"wav"),
            ("README", b"r"),
            ("clips/B002.mov", b"mov-b"),
            (".DS_Store", b"x"),
            (".hidden_dir/inner.mov", b"h"),
        ],
    );
    fs::create_dir_all(card.join("clips/empty")).unwrap();
    (dir, card)
}

fn scan_card(card: &Path, mode: DirMode, include_hidden: bool) -> Scan {
    let source = Source::Directory {
        path: card.to_path_buf(),
        mode,
    };
    scan(&source, &ScanOptions { include_hidden }).unwrap()
}

fn rels(paths: impl IntoIterator<Item = PathBuf>) -> BTreeSet<String> {
    paths
        .into_iter()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect()
}

#[test]
fn folder_itself_prefixes_paths_with_the_folder_name() {
    let (_dir, card) = card();
    let scan = scan_card(&card, DirMode::FolderItself, false);
    assert_eq!(scan.root_dir, Some(PathBuf::from("CARD")));
    assert_eq!(
        rels(scan.files.iter().map(|f| f.rel.clone())),
        rels(
            [
                "CARD/A001.MOV",
                "CARD/README",
                "CARD/clips/B002.mov",
                "CARD/sound.wav"
            ]
            .map(PathBuf::from)
        )
    );
}

#[test]
fn contents_only_has_no_prefix() {
    let (_dir, card) = card();
    let scan = scan_card(&card, DirMode::ContentsOnly, false);
    assert_eq!(scan.root_dir, None);
    assert_eq!(
        rels(scan.files.iter().map(|f| f.rel.clone())),
        rels(["A001.MOV", "README", "clips/B002.mov", "sound.wav"].map(PathBuf::from))
    );
}

#[test]
fn hidden_items_are_skipped_and_counted_once() {
    let (_dir, card) = card();
    let scan = scan_card(&card, DirMode::ContentsOnly, false);
    assert_eq!(scan.skipped_hidden, 2); // .DS_Store and .hidden_dir
    assert!(
        scan.files
            .iter()
            .all(|f| !f.rel.to_string_lossy().contains("inner"))
    );
}

#[test]
fn include_hidden_scans_hidden_items() {
    let (_dir, card) = card();
    let scan = scan_card(&card, DirMode::ContentsOnly, true);
    assert_eq!(scan.skipped_hidden, 0);
    assert_eq!(scan.files.len(), 6);
}

#[test]
fn a_hidden_source_folder_is_still_scanned() {
    let dir = tempfile::tempdir().unwrap();
    let hidden_root = dir.path().join(".config");
    write_files(&hidden_root, &[("app.toml", b"x")]);
    let scan = scan_card(&hidden_root, DirMode::FolderItself, false);
    assert_eq!(
        rels(scan.files.iter().map(|f| f.rel.clone())),
        rels([PathBuf::from(".config/app.toml")])
    );
}

#[test]
fn extension_stats_group_case_insensitively() {
    let (_dir, card) = card();
    let scan = scan_card(&card, DirMode::ContentsOnly, false);
    assert_eq!(
        scan.ext_stats[&Some("mov".into())],
        ExtStat {
            files: 2,
            bytes: 10
        }
    );
    assert_eq!(
        scan.ext_stats[&Some("wav".into())],
        ExtStat { files: 1, bytes: 3 }
    );
    assert_eq!(scan.ext_stats[&None], ExtStat { files: 1, bytes: 1 });
}

#[test]
fn empty_source_dirs_are_recorded() {
    let (_dir, card) = card();
    let scan = scan_card(&card, DirMode::FolderItself, false);
    assert_eq!(
        rels(scan.empty_dirs.clone()),
        rels([PathBuf::from("CARD/clips/empty")])
    );
}

#[test]
fn selecting_without_filter_keeps_every_file_and_empty_dirs() {
    let (_dir, card) = card();
    let sel = scan_card(&card, DirMode::FolderItself, false).select(&ExtensionFilter::All);
    assert_eq!(sel.files.len(), 4);
    assert_eq!(sel.total_bytes, 14);
    assert_eq!(
        rels(sel.dirs),
        rels(["CARD", "CARD/clips", "CARD/clips/empty"].map(PathBuf::from))
    );
}

#[test]
fn selecting_with_filter_drops_other_files_and_empty_dirs() {
    let (_dir, card) = card();
    let sel =
        scan_card(&card, DirMode::FolderItself, false).select(&ExtensionFilter::parse_list("mov"));
    assert_eq!(
        rels(sel.files.iter().map(|f| f.rel.clone())),
        rels(["CARD/A001.MOV", "CARD/clips/B002.mov"].map(PathBuf::from))
    );
    assert_eq!(sel.total_bytes, 10);
    assert_eq!(
        rels(sel.dirs),
        rels(["CARD", "CARD/clips"].map(PathBuf::from))
    );
}

#[cfg(unix)]
#[test]
fn symlinks_are_skipped_and_reported() {
    let (_dir, card) = card();
    std::os::unix::fs::symlink(card.join("A001.MOV"), card.join("link.mov")).unwrap();
    let scan = scan_card(&card, DirMode::ContentsOnly, false);
    assert_eq!(scan.skipped_symlinks.len(), 1);
    assert!(scan.files.iter().all(|f| f.rel != Path::new("link.mov")));
}

#[test]
fn file_sources_are_copied_flat() {
    let dir = tempfile::tempdir().unwrap();
    write_files(dir.path(), &[("x/a.txt", b"a"), ("y/z/b.txt", b"bb")]);
    let source = Source::Files(vec![
        dir.path().join("x/a.txt"),
        dir.path().join("y/z/b.txt"),
    ]);
    let scan = scan(&source, &ScanOptions::default()).unwrap();
    assert_eq!(
        rels(scan.files.iter().map(|f| f.rel.clone())),
        rels(["a.txt", "b.txt"].map(PathBuf::from))
    );
    assert!(scan.select(&ExtensionFilter::All).dirs.is_empty());
}

#[test]
fn missing_file_source_is_reported_as_a_problem() {
    let dir = tempfile::tempdir().unwrap();
    let source = Source::Files(vec![dir.path().join("nope.mov")]);
    let scan = scan(&source, &ScanOptions::default()).unwrap();
    assert!(scan.files.is_empty());
    assert_eq!(scan.problems.len(), 1);
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p secopy-core --test scan`
Expected: compile error, `unresolved import 'secopy_core::scan'`.

- [ ] **Step 4: Implement the scan**

`crates/secopy-core/src/scan.rs`, and add `pub mod scan;` to `lib.rs`:

```rust
//! Walks the source and builds the list of files to copy (FR-1..FR-14).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::filter::{ExtKey, ExtensionFilter, ext_key};
use crate::hidden::is_hidden;
use crate::source::{DirMode, Source};

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    /// Include hidden files and folders (FR-14). Not exposed in the v1 UI.
    pub include_hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanEntry {
    /// Path of the source file.
    pub source: PathBuf,
    /// Path relative to the destination directory.
    pub rel: PathBuf,
    pub size: u64,
    pub ext: ExtKey,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExtStat {
    pub files: u64,
    pub bytes: u64,
}

/// Something under the source that could not be read during the scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanProblem {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct Scan {
    pub files: Vec<ScanEntry>,
    /// Folder created for "copy the folder itself" (FR-4a), relative to the destination.
    pub root_dir: Option<PathBuf>,
    /// Source directories with no visible children, relative to the destination (FR-6).
    pub empty_dirs: Vec<PathBuf>,
    /// File count and bytes per extension, for the filter chips (FR-7).
    pub ext_stats: BTreeMap<ExtKey, ExtStat>,
    /// Hidden files and folders skipped; a skipped folder counts once (FR-13).
    pub skipped_hidden: u64,
    /// Symlinks are never followed or copied (FR-24).
    pub skipped_symlinks: Vec<PathBuf>,
    pub problems: Vec<ScanProblem>,
}

/// The files and directories a job will create.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    pub files: Vec<ScanEntry>,
    /// Directories to create, relative to the destination, parents first.
    pub dirs: Vec<PathBuf>,
    pub total_bytes: u64,
}

pub fn scan(source: &Source, opts: &ScanOptions) -> io::Result<Scan> {
    match source {
        Source::Directory { path, mode } => scan_dir(path, *mode, opts),
        Source::Files(paths) => Ok(scan_files(paths)),
    }
}

impl Scan {
    /// Applies the extension filter and lists the directories to create (FR-8..FR-10).
    pub fn select(&self, filter: &ExtensionFilter) -> Selection {
        let files: Vec<ScanEntry> = self
            .files
            .iter()
            .filter(|f| filter.matches(&f.ext))
            .cloned()
            .collect();
        let mut dirs: BTreeSet<PathBuf> = self.root_dir.iter().cloned().collect();
        if !filter.is_active() {
            dirs.extend(self.empty_dirs.iter().cloned());
        }
        for f in &files {
            if let Some(parent) = f.rel.parent().filter(|p| !p.as_os_str().is_empty()) {
                dirs.insert(parent.to_path_buf());
            }
        }
        let total_bytes = files.iter().map(|f| f.size).sum();
        Selection {
            files,
            dirs: dirs.into_iter().collect(),
            total_bytes,
        }
    }

    fn push_file(&mut self, source: PathBuf, rel: PathBuf, size: u64) {
        let ext = ext_key(&rel);
        let stat = self.ext_stats.entry(ext.clone()).or_default();
        stat.files += 1;
        stat.bytes += size;
        self.files.push(ScanEntry {
            source,
            rel,
            size,
            ext,
        });
    }

    fn problem(&mut self, path: &Path, message: impl ToString) {
        self.problems.push(ScanProblem {
            path: path.to_path_buf(),
            message: message.to_string(),
        });
    }
}

/// Files picked one by one are copied flat, and copied even if hidden:
/// the user chose them explicitly.
fn scan_files(paths: &[PathBuf]) -> Scan {
    let mut scan = Scan::default();
    for path in paths {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => scan.skipped_symlinks.push(path.clone()),
            Ok(meta) if meta.is_file() => {
                let name = path.file_name().expect("a regular file has a file name");
                scan.push_file(path.clone(), PathBuf::from(name), meta.len());
            }
            Ok(_) => scan.problem(path, "not a regular file"),
            Err(e) => scan.problem(path, e),
        }
    }
    scan
}

fn scan_dir(root: &Path, mode: DirMode, opts: &ScanOptions) -> io::Result<Scan> {
    let root = fs::canonicalize(root)?;
    let prefix = match mode {
        DirMode::ContentsOnly => PathBuf::new(),
        DirMode::FolderItself => PathBuf::from(root.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "a drive root has no folder name; copy only its contents instead",
            )
        })?),
    };
    let mut scan = Scan {
        root_dir: (!prefix.as_os_str().is_empty()).then(|| prefix.clone()),
        ..Scan::default()
    };
    let mut dirs = BTreeSet::new();
    let mut non_empty = HashSet::new();
    let mut skipped_hidden = 0u64;

    let walker = WalkDir::new(&root)
        .follow_links(false)
        .min_depth(1)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| {
            // The root is passed to the predicate too; picking a hidden folder is allowed.
            if opts.include_hidden || e.depth() == 0 {
                return true;
            }
            let hidden = e.metadata().is_ok_and(|m| is_hidden(e.path(), &m));
            if hidden {
                skipped_hidden += 1;
            }
            !hidden
        });

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                let path = e
                    .path()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| root.clone());
                scan.problem(&path, e);
                continue;
            }
        };
        let rel = prefix.join(
            entry
                .path()
                .strip_prefix(&root)
                .expect("walkdir yields paths under the root"),
        );
        let parent = rel.parent().map(Path::to_path_buf).unwrap_or_default();
        let file_type = entry.file_type();
        if file_type.is_symlink() {
            scan.skipped_symlinks.push(entry.into_path());
        } else if file_type.is_dir() {
            dirs.insert(rel);
            non_empty.insert(parent);
        } else if file_type.is_file() {
            match entry.metadata() {
                Ok(meta) => {
                    non_empty.insert(parent);
                    scan.push_file(entry.into_path(), rel, meta.len());
                }
                Err(e) => scan.problem(entry.path(), e),
            }
        }
        // Sockets, FIFOs and devices are ignored.
    }

    scan.skipped_hidden = skipped_hidden;
    scan.empty_dirs = dirs
        .into_iter()
        .filter(|d| !non_empty.contains(d))
        .collect();
    Ok(scan)
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p secopy-core --test scan`
Expected: `12 passed` (11 on Windows, where the symlink test is compiled out).

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/secopy-core
git commit -m "feat(core): scan sources and select files by extension"
```

---

### Task 4: xxhsum-compatible checksum file

**Files:**
- Create: `crates/secopy-core/src/checksum_file.rs`, `crates/secopy-core/tests/xxhsum_compat.rs`
- Modify: `crates/secopy-core/src/lib.rs` (add `pub mod checksum_file;`)

**Interfaces:**
- Consumes: `hash::{to_hex, hash_bytes}` (Task 1), `common::{pattern, write_files}` (Task 3).
- Produces: `checksum_file::{file_name(DateTime<Local>) -> String, slash_path(&Path) -> String, format_line(u64, &Path) -> String, write(&Path, &[(PathBuf, u64)], DateTime<Local>) -> io::Result<PathBuf>}`. `slash_path` is also used by the job runner (Task 7) to compare names.

- [ ] **Step 1: Install xxhsum locally**

Run: `brew install xxhash && xxhsum --version` (on Linux: `sudo apt install xxhash`)
Expected: `xxhsum 0.8.x`.

- [ ] **Step 2: Write the failing tests**

Unit tests, as the test module of `crates/secopy-core/src/checksum_file.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::fs;

    fn at(h: u32, m: u32, s: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 9, 26, h, m, s).unwrap()
    }

    #[test]
    fn file_name_uses_local_timestamp() {
        assert_eq!(file_name(at(14, 3, 2)), "secopy_2026-09-26_140302.xxh64");
    }

    #[test]
    fn line_uses_two_spaces_and_forward_slashes() {
        let rel = Path::new("DCIM").join("A001.mov");
        assert_eq!(
            format_line(0xef46_db37_51d8_e999, &rel),
            "ef46db3751d8e999  DCIM/A001.mov"
        );
    }

    #[cfg(unix)]
    #[test]
    fn names_with_newline_or_backslash_are_escaped() {
        assert_eq!(
            format_line(1, Path::new("a\nb")),
            "\\0000000000000001  a\\nb"
        );
        assert_eq!(
            format_line(1, Path::new("a\\b")),
            "\\0000000000000001  a\\\\b"
        );
    }

    #[test]
    fn write_sorts_lines_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let entries = vec![
            (Path::new("b").join("2.mov"), 2),
            (PathBuf::from("a.mov"), 1),
        ];
        let first = write(dir.path(), &entries, at(9, 0, 0)).unwrap();
        let second = write(dir.path(), &entries, at(9, 0, 0)).unwrap();

        assert_eq!(first.file_name().unwrap(), "secopy_2026-09-26_090000.xxh64");
        assert_eq!(
            second.file_name().unwrap(),
            "secopy_2026-09-26_090000_2.xxh64"
        );
        assert_eq!(
            fs::read_to_string(&first).unwrap(),
            "0000000000000001  a.mov\n0000000000000002  b/2.mov\n"
        );
    }
}
```

Real-tool compatibility test, `crates/secopy-core/tests/xxhsum_compat.rs`:

```rust
//! FR-30: `cd DEST && xxhsum -c <file>` must pass. Needs `xxhsum` on PATH; CI sets
//! SECOPY_REQUIRE_XXHSUM=1 so the test cannot silently skip there.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{pattern, write_files};
use secopy_core::checksum_file;
use secopy_core::hash::hash_bytes;

fn xxhsum_available() -> bool {
    Command::new("xxhsum").arg("--version").output().is_ok()
}

#[test]
fn xxhsum_accepts_our_checksum_file() {
    if !xxhsum_available() {
        assert!(
            std::env::var("SECOPY_REQUIRE_XXHSUM").as_deref() != Ok("1"),
            "xxhsum is required but not installed"
        );
        eprintln!("skipping: xxhsum not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("A001.mov", pattern(5000)),
        ("clips/B 002 (copy).mov", pattern(10)),
        ("clips/ñandú.wav", b"unicode".to_vec()),
        ("empty.bin", Vec::new()),
    ];
    let refs: Vec<(&str, &[u8])> = files.iter().map(|(p, d)| (*p, d.as_slice())).collect();
    write_files(dir.path(), &refs);
    let entries: Vec<(PathBuf, u64)> = files
        .iter()
        .map(|(p, d)| (Path::new(p).to_path_buf(), hash_bytes(d)))
        .collect();

    let sums = checksum_file::write(dir.path(), &entries, chrono::Local::now()).unwrap();
    let out = Command::new("xxhsum")
        .arg("-c")
        .arg(sums.file_name().unwrap())
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "xxhsum -c failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // And it really checks: corrupt one file and xxhsum must fail.
    fs::write(dir.path().join("A001.mov"), b"tampered").unwrap();
    let out = Command::new("xxhsum")
        .arg("-c")
        .arg(sums.file_name().unwrap())
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p secopy-core --lib checksum_file`
Expected: compile errors, `cannot find function 'file_name'`, `'format_line'`, `'write'`.

- [ ] **Step 4: Implement the checksum file**

Top of `crates/secopy-core/src/checksum_file.rs`, plus `pub mod checksum_file;` in `lib.rs`:

```rust
//! The xxhsum-compatible checksum file written to the destination (FR-29..FR-32).

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Local};

use crate::hash::to_hex;

/// `secopy_YYYY-MM-DD_HHMMSS.xxh64`
pub fn file_name(now: DateTime<Local>) -> String {
    format!("secopy_{}.xxh64", now.format("%Y-%m-%d_%H%M%S"))
}

/// Relative path with `/` separators on every OS.
pub fn slash_path(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// One line without the trailing newline: `<hash>  <path>`. Paths containing `\`, CR or
/// LF use the GNU coreutils escaping: a leading `\` and escaped characters.
pub fn format_line(hash: u64, rel: &Path) -> String {
    let path = slash_path(rel);
    if path.contains(['\\', '\n', '\r']) {
        let escaped = path
            .replace('\\', "\\\\")
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        format!("\\{}  {}", to_hex(hash), escaped)
    } else {
        format!("{}  {}", to_hex(hash), path)
    }
}

/// Writes the checksum file into `dest`, sorted by path, UTF-8, LF endings.
/// Never overwrites: adds `_2`, `_3`… if the name is taken. Returns the file's path.
pub fn write(dest: &Path, entries: &[(PathBuf, u64)], now: DateTime<Local>) -> io::Result<PathBuf> {
    let mut lines: Vec<(String, String)> = entries
        .iter()
        .map(|(rel, hash)| (slash_path(rel), format_line(*hash, rel)))
        .collect();
    lines.sort();
    let mut body = String::new();
    for (_, line) in &lines {
        body.push_str(line);
        body.push('\n');
    }
    let (path, mut file) = create_unique(dest, now)?;
    file.write_all(body.as_bytes())?;
    file.sync_all()?;
    Ok(path)
}

fn create_unique(dest: &Path, now: DateTime<Local>) -> io::Result<(PathBuf, File)> {
    let name = file_name(now);
    let stem = name.trim_end_matches(".xxh64");
    for n in 1u32.. {
        let candidate = if n == 1 {
            dest.join(&name)
        } else {
            dest.join(format!("{stem}_{n}.xxh64"))
        };
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    unreachable!("ran out of checksum file names")
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p secopy-core --lib checksum_file && SECOPY_REQUIRE_XXHSUM=1 cargo test -p secopy-core --test xxhsum_compat`
Expected: `4 passed` (3 on Windows), then `xxhsum_accepts_our_checksum_file` passes. The last one proves `xxhsum -c` accepts the file and rejects a tampered copy.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/secopy-core
git commit -m "feat(core): write xxhsum-compatible checksum files"
```

---

### Task 5: Error model and single-file copy with inline hash

**Files:**
- Create: `crates/secopy-core/src/error.rs`, `crates/secopy-core/src/os.rs`, `crates/secopy-core/src/copy.rs`, `crates/secopy-core/tests/copy.rs`
- Modify: `crates/secopy-core/src/lib.rs` (add `pub mod copy;`, `pub mod error;`, `mod os;`)

**Interfaces:**
- Consumes: `hash::{hasher, hash_bytes}` (Task 1), `common::pattern` (Task 3).
- Produces:
  - `error::IoFailure { kind: io::ErrorKind, message: String }` (Clone + Eq, `From<io::Error>`)
  - `error::FileError::{ReadSource(IoFailure), WriteDest(IoFailure), ReadBack(IoFailure), HashMismatch { expected: String, actual: String }, AlreadyExists, NameClash, Cancelled}`, with constructors `read_source`/`write_dest`/`read_back(io::Error) -> FileError` and `is_fatal(&self) -> bool`. Disk full is fatal.
  - `os::{sync_file(&File) -> io::Result<()>, set_nocache(&File) -> bool}` (crate-private)
  - `copy::CopyConfig { buffer_size: usize, buffers: usize, uncached_write: bool }`, with `Default` = 4 MiB × 4, false
  - `copy::PartialCopy { partial: PathBuf, hash: u64, bytes: u64 }`
  - `copy::{partial_path(&Path) -> PathBuf, copy_to_partial(src: &Path, final_path: &Path, &CopyConfig, progress: &dyn Fn(u64), cancel: &AtomicBool) -> Result<PartialCopy, FileError>, commit(partial: &Path, final_path: &Path) -> Result<(), FileError>}`

Why `os::sync_file` and not `File::sync_all`: on macOS, `sync_all` is `F_FULLFSYNC`, which also flushes the drive's cache. The M0 prototype measured 20,000 small files at **80 s** with `sync_all` against **2.6 s** with plain `fsync` (`cp -R`: 5.3 s). The drive-cache flush happens once per job instead (Task 7).

- [ ] **Step 1: Write the failing tests**

Unit tests at the bottom of `crates/secopy-core/src/error.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_full_is_fatal() {
        let e = FileError::write_dest(io::Error::from(io::ErrorKind::StorageFull));
        assert!(e.is_fatal());
    }

    #[test]
    fn other_errors_are_not_fatal() {
        let e = FileError::read_source(io::Error::from(io::ErrorKind::PermissionDenied));
        assert!(!e.is_fatal());
        assert!(!FileError::AlreadyExists.is_fatal());
    }
}
```

`crates/secopy-core/tests/copy.rs`:

```rust
mod common;

use std::cell::Cell;
use std::fs;
use std::sync::atomic::AtomicBool;

use common::pattern;
use secopy_core::copy::{CopyConfig, commit, copy_to_partial, partial_path};
use secopy_core::error::FileError;
use secopy_core::hash::hash_bytes;

fn small_buffers() -> CopyConfig {
    CopyConfig {
        buffer_size: 7,
        buffers: 2,
        uncached_write: false,
    }
}

#[test]
fn partial_name_is_hidden_and_next_to_the_final_file() {
    let p = partial_path(std::path::Path::new("/d/clips/A001.mov"));
    assert_eq!(p, std::path::Path::new("/d/clips/.A001.mov.secopy-partial"));
}

#[test]
fn small_file_is_copied_hashed_and_committed() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    fs::write(&src, b"hello secopy").unwrap();

    let pc = copy_to_partial(
        &src,
        &dst,
        &CopyConfig::default(),
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(pc.hash, hash_bytes(b"hello secopy"));
    assert_eq!(pc.bytes, 12);
    assert!(!dst.exists(), "final name only appears on commit");

    commit(&pc.partial, &dst).unwrap();
    assert_eq!(fs::read(&dst).unwrap(), b"hello secopy");
    assert!(!pc.partial.exists());
}

#[test]
fn large_file_goes_through_the_pipeline_in_chunks() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    let data = pattern(1000);
    fs::write(&src, &data).unwrap();
    let last = Cell::new(0);

    let pc = copy_to_partial(
        &src,
        &dst,
        &small_buffers(),
        &|b| last.set(b),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(pc.hash, hash_bytes(&data));
    assert_eq!(last.get(), 1000);
    assert_eq!(fs::read(&pc.partial).unwrap(), data);
}

#[test]
fn empty_file_has_the_empty_hash() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    fs::write(&src, b"").unwrap();
    let pc = copy_to_partial(
        &src,
        &dst,
        &CopyConfig::default(),
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(pc.hash, 0xef46_db37_51d8_e999);
    assert_eq!(pc.bytes, 0);
}

#[test]
fn cancel_removes_the_partial_file() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    fs::write(&src, pattern(1000)).unwrap();

    let err = copy_to_partial(
        &src,
        &dst,
        &small_buffers(),
        &|_| {},
        &AtomicBool::new(true),
    )
    .unwrap_err();
    assert_eq!(err, FileError::Cancelled);
    assert!(!partial_path(&dst).exists());
}

#[test]
fn missing_source_is_a_read_error_and_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let dst = dir.path().join("b.bin");
    let err = copy_to_partial(
        &dir.path().join("nope"),
        &dst,
        &CopyConfig::default(),
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert!(matches!(err, FileError::ReadSource(_)));
    assert!(!partial_path(&dst).exists());
}

#[test]
fn missing_destination_folder_is_a_write_error() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.bin");
    fs::write(&src, b"x").unwrap();
    let err = copy_to_partial(
        &src,
        &dir.path().join("no/such/b.bin"),
        &CopyConfig::default(),
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert!(matches!(err, FileError::WriteDest(_)));
}

#[test]
fn sizes_around_the_buffer_size_copy_exactly() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = small_buffers(); // 7-byte buffers
    for size in [6, 7, 8, 14, 15] {
        let (src, dst) = (
            dir.path().join(format!("s{size}")),
            dir.path().join(format!("d{size}")),
        );
        let data = pattern(size);
        fs::write(&src, &data).unwrap();
        let pc = copy_to_partial(&src, &dst, &cfg, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!(pc.hash, hash_bytes(&data), "size {size}");
        assert_eq!(fs::read(&pc.partial).unwrap(), data, "size {size}");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p secopy-core --test copy`
Expected: compile error, `unresolved import 'secopy_core::copy'`.

- [ ] **Step 3: Implement the error model**

Top of `crates/secopy-core/src/error.rs`:

```rust
//! Per-file errors (FR-21).

use std::{fmt, io};

/// An I/O error reduced to plain data, so outcomes can be cloned, compared and sent to the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoFailure {
    pub kind: io::ErrorKind,
    pub message: String,
}

impl From<io::Error> for IoFailure {
    fn from(e: io::Error) -> Self {
        Self {
            kind: e.kind(),
            message: e.to_string(),
        }
    }
}

impl fmt::Display for IoFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// Why one file was not copied.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FileError {
    #[error("cannot read source: {0}")]
    ReadSource(IoFailure),
    #[error("cannot write destination: {0}")]
    WriteDest(IoFailure),
    #[error("cannot read back the copy: {0}")]
    ReadBack(IoFailure),
    #[error("hash mismatch (source {expected}, copy {actual})")]
    HashMismatch { expected: String, actual: String },
    #[error("a file with this name already exists at the destination")]
    AlreadyExists,
    #[error("another file in this copy has the same name (names are compared ignoring case)")]
    NameClash,
    #[error("cancelled")]
    Cancelled,
}

impl FileError {
    pub fn read_source(e: io::Error) -> Self {
        FileError::ReadSource(e.into())
    }

    pub fn write_dest(e: io::Error) -> Self {
        FileError::WriteDest(e.into())
    }

    pub fn read_back(e: io::Error) -> Self {
        FileError::ReadBack(e.into())
    }

    /// Errors that stop the whole job instead of just this file (FR-21).
    pub fn is_fatal(&self) -> bool {
        matches!(self, FileError::WriteDest(f) if f.kind == io::ErrorKind::StorageFull)
    }
}
```

- [ ] **Step 4: Implement the OS helpers (first part)**

`crates/secopy-core/src/os.rs`. Task 6 and Task 7 each add one more function here:

```rust
//! Platform-specific flushing and cache control (FR-18, FR-26, RFD §7.4).

use std::fs::File;
use std::io;

/// Pushes a file's data to the device. On macOS, `File::sync_all` is `F_FULLFSYNC`,
/// which also flushes the drive's cache and costs milliseconds per call; per file we
/// use plain `fsync` and flush the drive cache once per job with [`full_barrier`].
#[cfg(target_os = "macos")]
pub fn sync_file(file: &File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    // SAFETY: `fsync` on a valid descriptor owned by `file`.
    if unsafe { libc::fsync(file.as_raw_fd()) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(target_os = "macos"))]
pub fn sync_file(file: &File) -> io::Result<()> {
    file.sync_all()
}
/// Asks the OS not to keep this file's pages in cache. Only macOS supports this per
/// file descriptor; elsewhere it is a no-op. Returns true if the request was accepted.
#[cfg(target_os = "macos")]
pub fn set_nocache(file: &File) -> bool {
    use std::os::fd::AsRawFd;
    // SAFETY: `fcntl` on a valid descriptor owned by `file`, with an integer argument.
    unsafe { libc::fcntl(file.as_raw_fd(), libc::F_NOCACHE, 1) != -1 }
}

#[cfg(not(target_os = "macos"))]
pub fn set_nocache(_file: &File) -> bool {
    false
}
```

- [ ] **Step 5: Implement the copy**

`crates/secopy-core/src/copy.rs`, plus `pub mod copy;`, `pub mod error;` and `mod os;` in `lib.rs`:

```rust
//! Copies one file to a temporary "partial" file while hashing it (FR-18, FR-20, RFD §7.2).

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use crate::error::FileError;
use crate::{hash, os};

/// Tuning for the copy pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyConfig {
    /// Size of each I/O buffer. Files up to this size are copied in one read.
    pub buffer_size: usize,
    /// Buffers in flight between the reader and writer threads (min 2).
    pub buffers: usize,
    /// Keep written data out of the OS cache (macOS), so a later verify reads the device.
    pub uncached_write: bool,
}

impl Default for CopyConfig {
    fn default() -> Self {
        Self {
            buffer_size: 4 << 20,
            buffers: 4,
            uncached_write: false,
        }
    }
}

/// A fully written and flushed copy that still has its temporary name.
#[derive(Debug)]
pub struct PartialCopy {
    pub partial: PathBuf,
    /// xxHash64 of the bytes read from the source.
    pub hash: u64,
    pub bytes: u64,
}

/// Temporary name used while a file is written: `.<name>.secopy-partial`, same directory.
pub fn partial_path(final_path: &Path) -> PathBuf {
    let mut name = OsString::from(".");
    name.push(
        final_path
            .file_name()
            .expect("destination path has a file name"),
    );
    name.push(".secopy-partial");
    final_path.with_file_name(name)
}

/// Copies `src` to the partial path of `final_path`, hashing the bytes as they are read,
/// then fsyncs it. On error or cancel the partial file is removed.
/// `progress` receives the number of bytes written so far.
pub fn copy_to_partial(
    src: &Path,
    final_path: &Path,
    cfg: &CopyConfig,
    progress: &dyn Fn(u64),
    cancel: &AtomicBool,
) -> Result<PartialCopy, FileError> {
    let partial = partial_path(final_path);
    match copy_inner(src, &partial, cfg, progress, cancel) {
        Ok((hash, bytes)) => Ok(PartialCopy {
            partial,
            hash,
            bytes,
        }),
        Err(e) => {
            let _ = fs::remove_file(&partial);
            Err(e)
        }
    }
}

/// Gives a finished partial file its final name (FR-18).
pub fn commit(partial: &Path, final_path: &Path) -> Result<(), FileError> {
    fs::rename(partial, final_path).map_err(FileError::write_dest)
}

fn copy_inner(
    src: &Path,
    partial: &Path,
    cfg: &CopyConfig,
    progress: &dyn Fn(u64),
    cancel: &AtomicBool,
) -> Result<(u64, u64), FileError> {
    let mut reader = File::open(src).map_err(FileError::read_source)?;
    let len = reader.metadata().map_err(FileError::read_source)?.len();
    let mut writer = File::create(partial).map_err(FileError::write_dest)?;
    if cfg.uncached_write {
        os::set_nocache(&writer);
    }
    let result = if len <= cfg.buffer_size as u64 {
        copy_small(&mut reader, &mut writer, len, progress, cancel)?
    } else {
        copy_pipelined(reader, &mut writer, cfg, progress, cancel)?
    };
    os::sync_file(&writer).map_err(FileError::write_dest)?;
    Ok(result)
}

/// Small files: one read, one write, no extra thread.
fn copy_small(
    reader: &mut File,
    writer: &mut File,
    len: u64,
    progress: &dyn Fn(u64),
    cancel: &AtomicBool,
) -> Result<(u64, u64), FileError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(FileError::Cancelled);
    }
    let mut buf = Vec::with_capacity(len as usize);
    reader
        .read_to_end(&mut buf)
        .map_err(FileError::read_source)?;
    writer.write_all(&buf).map_err(FileError::write_dest)?;
    progress(buf.len() as u64);
    Ok((hash::hash_bytes(&buf), buf.len() as u64))
}

/// Large files: a reader thread fills and hashes buffers while this thread writes them,
/// so reading chunk N+1 overlaps writing chunk N.
fn copy_pipelined(
    mut reader: File,
    writer: &mut File,
    cfg: &CopyConfig,
    progress: &dyn Fn(u64),
    cancel: &AtomicBool,
) -> Result<(u64, u64), FileError> {
    let buffers = cfg.buffers.max(2);
    let (full_tx, full_rx) = mpsc::sync_channel::<(Vec<u8>, usize)>(buffers);
    let (empty_tx, empty_rx) = mpsc::sync_channel::<Vec<u8>>(buffers);
    for _ in 0..buffers {
        empty_tx
            .send(vec![0; cfg.buffer_size])
            .expect("channel has room for every buffer");
    }
    std::thread::scope(|s| {
        let reader_thread = s.spawn(move || -> io::Result<u64> {
            let mut hasher = hash::hasher();
            while let Ok(mut buf) = empty_rx.recv() {
                let n = read_full(&mut reader, &mut buf)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
                if full_tx.send((buf, n)).is_err() {
                    break; // writer stopped
                }
            }
            Ok(hasher.digest())
        });
        let written = write_chunks(&full_rx, &empty_tx, writer, progress, cancel);
        // Unblock the reader if the writer stopped early.
        drop(full_rx);
        drop(empty_tx);
        let read = reader_thread.join().expect("reader thread panicked");
        let written = written?;
        let hash = read.map_err(FileError::read_source)?;
        Ok((hash, written))
    })
}

fn write_chunks(
    full_rx: &mpsc::Receiver<(Vec<u8>, usize)>,
    empty_tx: &mpsc::SyncSender<Vec<u8>>,
    writer: &mut File,
    progress: &dyn Fn(u64),
    cancel: &AtomicBool,
) -> Result<u64, FileError> {
    let mut written = 0u64;
    for (buf, n) in full_rx.iter() {
        if cancel.load(Ordering::Relaxed) {
            return Err(FileError::Cancelled);
        }
        writer.write_all(&buf[..n]).map_err(FileError::write_dest)?;
        written += n as u64;
        progress(written);
        let _ = empty_tx.send(buf);
    }
    Ok(written)
}

/// Reads until `buf` is full or EOF; returns the number of bytes read.
fn read_full(r: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match r.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p secopy-core --test copy && cargo test -p secopy-core --lib error`
Expected: `8 passed` (copy) and `2 passed` (error).

- [ ] **Step 7: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/secopy-core
git commit -m "feat(core): copy files to partial files with inline xxhash64"
```

---

### Task 6: Verify from the device (cache bypass)

**Files:**
- Create: `crates/secopy-core/src/verify.rs`, `crates/secopy-core/tests/verify.rs`
- Modify: `crates/secopy-core/src/os.rs` (add `use std::path::Path;` and `open_uncached`), `crates/secopy-core/src/lib.rs` (add `pub mod verify;`)

**Interfaces:**
- Consumes: `FileError::{read_back, Cancelled}` (Task 5), `hash::hasher` (Task 1), `common::pattern` (Task 3).
- Produces:
  - `verify::CacheBypass::{Active, Unavailable}`
  - `verify::hash_from_device(path: &Path, buffer_size: usize, progress: &dyn Fn(u64), cancel: &AtomicBool) -> Result<(u64, CacheBypass), FileError>`
  - `os::open_uncached(&Path) -> io::Result<(File, bool)>` (crate-private)

How the bypass works on each OS:
- **macOS:** `F_NOCACHE` on the read, plus `CopyConfig::uncached_write` so the written pages never stay cached.
- **Linux:** `posix_fadvise(DONTNEED)`, which evicts the pages. They are clean because the copy fsynced them.
- **Windows:** `FILE_FLAG_NO_BUFFERING`, which needs sector-aligned buffers. Hence `AlignedBuf`, and a read loop driven by the file size.

- [ ] **Step 1: Write the failing tests**

`crates/secopy-core/tests/verify.rs`:

```rust
mod common;

use std::cell::Cell;
use std::fs;
use std::sync::atomic::AtomicBool;

use common::pattern;
use secopy_core::error::FileError;
use secopy_core::hash::hash_bytes;
use secopy_core::verify::{CacheBypass, hash_from_device};

#[test]
fn hashes_the_file_in_chunks_and_reports_progress() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.bin");
    // Not a multiple of 4096, so the last unbuffered read is short.
    let data = pattern(3 * 4096 + 123);
    fs::write(&path, &data).unwrap();
    let last = Cell::new(0);

    let (hash, _) =
        hash_from_device(&path, 4096, &|b| last.set(b), &AtomicBool::new(false)).unwrap();
    assert_eq!(hash, hash_bytes(&data));
    assert_eq!(last.get(), data.len() as u64);
}

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
#[test]
fn cache_bypass_is_active_on_local_disks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.bin");
    fs::write(&path, b"data").unwrap();
    let (_, bypass) = hash_from_device(&path, 4096, &|_| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(bypass, CacheBypass::Active);
}

#[test]
fn empty_file_hashes_to_the_empty_hash() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.bin");
    fs::write(&path, b"").unwrap();
    let (hash, _) = hash_from_device(&path, 4096, &|_| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(hash, 0xef46_db37_51d8_e999);
}

#[test]
fn missing_file_is_a_read_back_error() {
    let dir = tempfile::tempdir().unwrap();
    let err = hash_from_device(
        &dir.path().join("nope"),
        4096,
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert!(matches!(err, FileError::ReadBack(_)));
}

#[test]
fn cancel_stops_verification() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.bin");
    fs::write(&path, pattern(10_000)).unwrap();
    let err = hash_from_device(&path, 4096, &|_| {}, &AtomicBool::new(true)).unwrap_err();
    assert_eq!(err, FileError::Cancelled);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p secopy-core --test verify`
Expected: compile error, `unresolved import 'secopy_core::verify'`.

- [ ] **Step 3: Add `open_uncached` to the OS helpers**

In `crates/secopy-core/src/os.rs`, add `use std::path::Path;` below `use std::io;`, then append:

```rust
/// Opens `path` so reads come from the device, not the page cache.
/// The bool is false when the platform or file system cannot guarantee that.
#[cfg(target_os = "macos")]
pub fn open_uncached(path: &Path) -> io::Result<(File, bool)> {
    let file = File::open(path)?;
    let bypassed = set_nocache(&file);
    Ok((file, bypassed))
}

/// Linux: the file was fsynced, so its cached pages are clean and DONTNEED evicts them.
#[cfg(target_os = "linux")]
pub fn open_uncached(path: &Path) -> io::Result<(File, bool)> {
    use std::os::fd::AsRawFd;
    let file = File::open(path)?;
    // SAFETY: `posix_fadvise` on a valid descriptor owned by `file`.
    let rc = unsafe { libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED) };
    Ok((file, rc == 0))
}

/// Windows: FILE_FLAG_NO_BUFFERING. Reads must use sector-aligned buffers and sizes.
#[cfg(windows)]
pub fn open_uncached(path: &Path) -> io::Result<(File, bool)> {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
    match OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_NO_BUFFERING)
        .open(path)
    {
        Ok(file) => Ok((file, true)),
        Err(_) => Ok((File::open(path)?, false)),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
pub fn open_uncached(path: &Path) -> io::Result<(File, bool)> {
    Ok((File::open(path)?, false))
}
```

- [ ] **Step 4: Implement verify**

`crates/secopy-core/src/verify.rs`, plus `pub mod verify;` in `lib.rs`:

```rust
//! Re-reads a written file from the device and hashes it (FR-25, FR-26, RFD §7.3).

use std::alloc::{self, Layout};
use std::io::{self, Read};
use std::path::Path;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::FileError;
use crate::{hash, os};

/// Whether the verify read bypassed the OS page cache (FR-26).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheBypass {
    Active,
    Unavailable,
}

/// Hashes `path` reading it from the device. `progress` receives bytes read so far.
pub fn hash_from_device(
    path: &Path,
    buffer_size: usize,
    progress: &dyn Fn(u64),
    cancel: &AtomicBool,
) -> Result<(u64, CacheBypass), FileError> {
    let (mut file, bypassed) = os::open_uncached(path).map_err(FileError::read_back)?;
    let size = file.metadata().map_err(FileError::read_back)?.len();
    let mut buf = AlignedBuf::new(buffer_size);
    let mut hasher = hash::hasher();
    let mut done = 0u64;
    // Driven by the file size: with unbuffered I/O on Windows a read after a short
    // (unaligned) read fails, and on Unix a short read before EOF must not end the hash.
    while done < size {
        if cancel.load(Ordering::Relaxed) {
            return Err(FileError::Cancelled);
        }
        let chunk = buf.as_mut_slice();
        let n = read_once(&mut file, chunk).map_err(FileError::read_back)?;
        if n == 0 {
            return Err(FileError::read_back(io::ErrorKind::UnexpectedEof.into()));
        }
        hasher.update(&chunk[..n]);
        done += n as u64;
        progress(done);
    }
    let bypass = if bypassed {
        CacheBypass::Active
    } else {
        CacheBypass::Unavailable
    };
    Ok((hasher.digest(), bypass))
}

fn read_once(file: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    loop {
        match file.read(buf) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            other => return other,
        }
    }
}

/// Page-aligned buffer whose length is a multiple of 4096, as unbuffered I/O requires.
struct AlignedBuf {
    ptr: NonNull<u8>,
    layout: Layout,
}

const ALIGN: usize = 4096;

impl AlignedBuf {
    fn new(len: usize) -> Self {
        let len = len.max(ALIGN).next_multiple_of(ALIGN);
        let layout = Layout::from_size_align(len, ALIGN).expect("valid buffer layout");
        // SAFETY: `layout` has a non-zero size.
        let raw = unsafe { alloc::alloc_zeroed(layout) };
        let ptr = NonNull::new(raw).unwrap_or_else(|| alloc::handle_alloc_error(layout));
        Self { ptr, layout }
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: `ptr` points to `layout.size()` initialised bytes owned by `self`.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.layout.size()) }
    }
}

impl Drop for AlignedBuf {
    fn drop(&mut self) {
        // SAFETY: allocated in `new` with this exact layout.
        unsafe { alloc::dealloc(self.ptr.as_ptr(), self.layout) }
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p secopy-core --test verify`
Expected: `5 passed`.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/secopy-core
git commit -m "feat(core): verify copies from the device with cache bypass"
```

---

### Task 7: Job runner (lanes, verify-and-retry, progress, checksum file)

**Files:**
- Create: `crates/secopy-core/src/job.rs`, `crates/secopy-core/tests/job.rs`
- Modify: `crates/secopy-core/src/os.rs` (add `full_barrier`), `crates/secopy-core/src/lib.rs` (add `pub mod job;`), `docs/rfd/0001-secopy.md`

**Interfaces:**
- Consumes: `Selection`, `ScanEntry` (Task 3). `checksum_file::{write, slash_path}` (Task 4). `CopyConfig`, `PartialCopy`, `copy_to_partial`, `commit`, `FileError` (Task 5). `hash_from_device`, `CacheBypass` (Task 6). `os::{sync_file, full_barrier}`.
- Produces (this is the API the CLI and the Tauri app call):
  - `job::JobOptions { verify, write_checksum_file, copy: CopyConfig, small_file_threshold: u64, small_file_lanes, large_file_lanes, verify_lanes: usize, progress_interval: Duration, hooks: Hooks }`. `Default` = verify on, checksum on, 8 MiB threshold, 8/1/2 lanes, 50 ms.
  - `job::Hooks { after_copy: Option<fn(&Path, u32)> }` (`#[doc(hidden)]`, tests only)
  - `job::JobControl::{new(), cancel(&self), is_stopped(&self) -> bool}`
  - `job::Phase::{Copying, Verifying}`, `job::FileStatus::{Copied, Verified, Failed(FileError)}`
  - `job::FileOutcome { rel, size, hash: Option<u64>, status, elapsed }`
  - `job::ActiveFile { id: usize, rel, size, phase, bytes_done }`
  - `job::Progress { total_files, total_bytes, files_done, copied_bytes, verified_bytes, active: Vec<ActiveFile> }`
  - `job::Event::{Progress(Progress), FileFinished(FileOutcome)}`
  - `job::JobReport { outcomes, not_started, checksum_file: Option<PathBuf>, checksum_error: Option<String>, cache_bypass: Option<CacheBypass>, fatal: Option<FileError>, cancelled, elapsed }`, with `failed()` and `is_success()`
  - `job::run_job(&Selection, dest: &Path, &JobOptions, &JobControl, on_event: &(dyn Fn(Event) + Sync)) -> JobReport`

Design notes for the implementer:
- Files up to `small_file_threshold` go to the small lanes, bigger ones to the large lanes. Each lane pulls from a lock-free `Queue` (an atomic index into a `Vec`).
- In verify mode a copy lane hands the partial file to the verify lanes through an `mpsc` channel and moves on. So verifying file N overlaps copying file N+1 (RFD §7.3). The verify lane renames the file only after the hashes match. On a mismatch it deletes the partial and re-copies once (FR-27).
- Names that clash case-insensitively are found **before** any lane starts (Review Focus 1).
- Byte counters are "finished files + in-flight bytes". Failed files count as fully processed, so the bars end at 100 %.

- [ ] **Step 1: Write the failing tests**

`crates/secopy-core/tests/job.rs`:

```rust
mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use common::{pattern, read_tree, write_files};
use secopy_core::copy::CopyConfig;
use secopy_core::error::FileError;
use secopy_core::filter::ExtensionFilter;
use secopy_core::hash::{hash_bytes, to_hex};
use secopy_core::job::{
    Event, FileStatus, Hooks, JobControl, JobOptions, JobReport, Progress, run_job,
};
use secopy_core::scan::{ScanOptions, Selection, scan};
use secopy_core::source::{DirMode, Source};

struct Fixture {
    _dir: tempfile::TempDir,
    src: PathBuf,
    dest: PathBuf,
}

/// src/CARD with a mix of small and "large" files (large = above the 64-byte test threshold).
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src/CARD");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    let big = pattern(1000);
    write_files(
        &src,
        &[
            ("A001.mov", &big),
            ("clips/B002.mov", &pattern(333)),
            ("notes.txt", b"hello"),
            ("empty.bin", b""),
        ],
    );
    Fixture {
        _dir: dir,
        src,
        dest,
    }
}

fn select(src: &Path) -> Selection {
    let source = Source::Directory {
        path: src.to_path_buf(),
        mode: DirMode::FolderItself,
    };
    scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All)
}

/// Small buffers and a low threshold so both lanes and the pipeline are exercised.
fn opts(verify: bool) -> JobOptions {
    JobOptions {
        verify,
        copy: CopyConfig {
            buffer_size: 64,
            buffers: 3,
            uncached_write: false,
        },
        small_file_threshold: 64,
        small_file_lanes: 4,
        large_file_lanes: 2,
        ..JobOptions::default()
    }
}

fn run(sel: &Selection, dest: &Path, opts: &JobOptions) -> (JobReport, Vec<Event>) {
    let events = Mutex::new(Vec::new());
    let report = run_job(sel, dest, opts, &JobControl::new(), &|e| {
        events.lock().unwrap().push(e)
    });
    (report, events.into_inner().unwrap())
}

fn expected_tree(src: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    read_tree(src.parent().unwrap())
}

#[test]
fn copy_mode_copies_everything_and_writes_the_checksum_file() {
    let f = fixture();
    let sel = select(&f.src);
    let (report, _) = run(&sel, &f.dest, &opts(false));

    assert!(report.is_success(), "{report:?}");
    assert!(
        report
            .outcomes
            .iter()
            .all(|o| o.status == FileStatus::Copied)
    );
    assert_eq!(read_tree(&f.dest), expected_tree(&f.src));
    assert_eq!(report.cache_bypass, None);

    let sums = fs::read_to_string(report.checksum_file.unwrap()).unwrap();
    let line = format!("{}  CARD/A001.mov", to_hex(hash_bytes(&pattern(1000))));
    assert!(sums.lines().any(|l| l == line), "{sums}");
    assert_eq!(sums.lines().count(), 4);
}

#[test]
fn verify_mode_marks_files_verified() {
    let f = fixture();
    let (report, _) = run(&select(&f.src), &f.dest, &opts(true));
    assert!(report.is_success(), "{report:?}");
    assert!(
        report
            .outcomes
            .iter()
            .all(|o| o.status == FileStatus::Verified)
    );
    assert_eq!(read_tree(&f.dest), expected_tree(&f.src));
    assert!(report.cache_bypass.is_some());
}

#[test]
fn empty_source_folders_are_recreated() {
    let f = fixture();
    fs::create_dir_all(f.src.join("EMPTY")).unwrap();
    run(&select(&f.src), &f.dest, &opts(false));
    assert!(f.dest.join("CARD/EMPTY").is_dir());
}

fn flip_first_byte(path: &Path) {
    let mut data = fs::read(path).unwrap();
    data[0] ^= 0xFF;
    fs::write(path, data).unwrap();
}

#[test]
fn a_corrupted_copy_is_recopied_once_and_then_verifies() {
    let f = fixture();
    let mut o = opts(true);
    o.hooks = Hooks {
        after_copy: Some(|p, attempt| {
            if attempt == 0 && p.to_string_lossy().contains("A001") {
                flip_first_byte(p);
            }
        }),
    };
    let (report, _) = run(&select(&f.src), &f.dest, &o);
    assert!(report.is_success(), "{report:?}");
    assert_eq!(read_tree(&f.dest), expected_tree(&f.src));
}

#[test]
fn a_copy_that_stays_corrupted_fails_and_leaves_no_file() {
    let f = fixture();
    let mut o = opts(true);
    o.hooks = Hooks {
        after_copy: Some(|p, _| {
            if p.to_string_lossy().contains("A001") {
                flip_first_byte(p);
            }
        }),
    };
    let (report, _) = run(&select(&f.src), &f.dest, &o);

    let failed: Vec<_> = report.failed().collect();
    assert_eq!(failed.len(), 1);
    assert!(matches!(
        failed[0].status,
        FileStatus::Failed(FileError::HashMismatch { .. })
    ));
    assert!(!f.dest.join("CARD/A001.mov").exists());
    assert!(
        read_tree(&f.dest)
            .keys()
            .all(|k| !k.contains("secopy-partial"))
    );
    let sums = fs::read_to_string(report.checksum_file.unwrap()).unwrap();
    assert!(
        !sums.contains("A001"),
        "failed files are not listed (FR-28)"
    );
}

#[test]
fn an_existing_destination_file_is_never_overwritten() {
    let f = fixture();
    write_files(&f.dest, &[("CARD/notes.txt", b"mine")]);
    let (report, _) = run(&select(&f.src), &f.dest, &opts(false));

    let failed: Vec<_> = report.failed().collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(
        failed[0].status,
        FileStatus::Failed(FileError::AlreadyExists)
    );
    assert_eq!(fs::read(f.dest.join("CARD/notes.txt")).unwrap(), b"mine");
}

#[test]
fn cancelling_before_start_copies_nothing() {
    let f = fixture();
    let sel = select(&f.src);
    let control = JobControl::new();
    control.cancel();
    let report = run_job(&sel, &f.dest, &opts(true), &control, &|_| {});

    assert!(report.cancelled);
    assert_eq!(report.not_started, sel.files.len() as u64);
    assert!(read_tree(&f.dest).is_empty());
    assert_eq!(report.checksum_file, None);
}

#[test]
fn checksum_file_can_be_turned_off() {
    let f = fixture();
    let mut o = opts(false);
    o.write_checksum_file = false;
    let (report, _) = run(&select(&f.src), &f.dest, &o);
    assert_eq!(report.checksum_file, None);
    assert!(
        fs::read_dir(&f.dest)
            .unwrap()
            .all(|e| { e.unwrap().path().extension().is_none_or(|x| x != "xxh64") })
    );
}

#[test]
fn final_progress_event_is_complete() {
    let f = fixture();
    let sel = select(&f.src);
    let (_, events) = run(&sel, &f.dest, &opts(true));

    let last = events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Progress(p) => Some(p.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        last,
        Progress {
            total_files: 4,
            total_bytes: sel.total_bytes,
            files_done: 4,
            copied_bytes: sel.total_bytes,
            verified_bytes: sel.total_bytes,
            active: vec![],
        }
    );
    let finished = events
        .iter()
        .filter(|e| matches!(e, Event::FileFinished(_)))
        .count();
    assert_eq!(finished, 4);
}

#[test]
fn many_small_files_are_all_copied() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    for i in 0..300 {
        write_files(&src, &[(&format!("d{}/f{i}.bin", i % 7), &pattern(i))]);
    }
    let (report, _) = run(&select(&src), &dest, &opts(true));
    assert!(report.is_success(), "{report:?}");
    assert_eq!(read_tree(&dest.join("src")), read_tree(&src));
}

#[cfg(unix)]
#[test]
fn an_unreadable_file_fails_alone() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    let locked = f.src.join("notes.txt");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&locked).is_ok() {
        return; // running as root: permissions are not enforced
    }
    let (report, _) = run(&select(&f.src), &f.dest, &opts(true));

    let failed: Vec<_> = report.failed().collect();
    assert_eq!(failed.len(), 1);
    assert!(matches!(
        failed[0].status,
        FileStatus::Failed(FileError::ReadSource(_))
    ));
    assert_eq!(report.outcomes.len(), 4);
}

#[test]
fn files_that_map_to_the_same_name_never_mix() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    write_files(
        dir.path(),
        &[("x/a.txt", &pattern(500)), ("y/A.TXT", &pattern(900))],
    );
    let source = Source::Files(vec![dir.path().join("x/a.txt"), dir.path().join("y/A.TXT")]);
    let sel = scan(&source, &ScanOptions::default())
        .unwrap()
        .select(&ExtensionFilter::All);
    let (report, _) = run(&sel, &dest, &opts(false));

    let failed: Vec<_> = report.failed().collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].rel, Path::new("A.TXT"));
    assert_eq!(failed[0].status, FileStatus::Failed(FileError::NameClash));
    assert_eq!(fs::read(dest.join("a.txt")).unwrap(), pattern(500));
}

#[test]
fn an_unwritable_destination_fails_every_file_without_hanging() {
    let f = fixture();
    let not_a_dir = f.dest.join("file");
    fs::write(&not_a_dir, b"x").unwrap();
    let (report, _) = run(&select(&f.src), &not_a_dir, &opts(true));

    assert_eq!(report.outcomes.len(), 4);
    assert!(
        report
            .outcomes
            .iter()
            .all(|o| matches!(o.status, FileStatus::Failed(FileError::WriteDest(_))))
    );
    assert_eq!(report.checksum_file, None);
}

#[test]
fn unicode_names_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("Tomas");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    let names = [
        "ñandú.wav",
        "日本語 クリップ.mov",
        "cafe\u{301}.txt",
        "🎬 take 1.mov",
    ];
    for n in names {
        write_files(&src, &[(n, n.as_bytes())]);
    }
    let (report, _) = run(&select(&src), &dest, &opts(true));

    assert!(report.is_success(), "{report:?}");
    assert_eq!(read_tree(&dest.join("Tomas")), read_tree(&src));
    let sums = fs::read_to_string(report.checksum_file.unwrap()).unwrap();
    for n in names {
        assert!(
            sums.contains(&format!("  Tomas/{n}\n")),
            "{n} missing in:\n{sums}"
        );
    }
}

#[test]
fn cancelling_mid_job_keeps_finished_files_and_removes_partials() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    for i in 0..50 {
        write_files(&src, &[(&format!("f{i:02}.bin"), &pattern(2000))]);
    }
    let mut o = opts(true);
    o.small_file_lanes = 1;
    o.large_file_lanes = 1;
    let control = JobControl::new();
    let report = run_job(&select(&src), &dest, &o, &control, &|e| {
        if let Event::FileFinished(_) = e {
            control.cancel();
        }
    });

    assert!(report.cancelled);
    assert!(report.not_started > 0);
    let tree = read_tree(&dest);
    assert!(
        tree.keys().all(|k| !k.contains("secopy-partial")),
        "{:?}",
        tree.keys()
    );
    let ok: Vec<_> = report
        .outcomes
        .iter()
        .filter(|o| o.status == FileStatus::Verified)
        .collect();
    assert!(!ok.is_empty());
    assert_eq!(tree.len(), ok.len(), "only verified files remain");
    let sums = fs::read_to_string(report.checksum_file.unwrap()).unwrap();
    assert_eq!(sums.lines().count(), ok.len());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p secopy-core --test job`
Expected: compile error, `unresolved import 'secopy_core::job'`.

- [ ] **Step 3: Add `full_barrier` to the OS helpers**

In `crates/secopy-core/src/os.rs`, insert after `sync_file`:

```rust
/// Makes everything written to the volume holding `dir` durable, including the
/// drive's own cache. Called once at the end of a job.
#[cfg(target_os = "macos")]
pub fn full_barrier(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

#[cfg(not(target_os = "macos"))]
pub fn full_barrier(_dir: &Path) -> io::Result<()> {
    Ok(())
}
```

- [ ] **Step 4: Implement the job runner**

`crates/secopy-core/src/job.rs`, plus `pub mod job;` in `lib.rs`:

```rust
//! Runs a copy job: copy lanes, verify lanes, progress events and the checksum file (RFD §7).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use chrono::Local;

use crate::checksum_file;
use crate::copy::{self, CopyConfig, PartialCopy};
use crate::error::FileError;
use crate::scan::{ScanEntry, Selection};
use crate::verify::{self, CacheBypass};
use crate::{hash, os};

#[derive(Debug, Clone)]
pub struct JobOptions {
    /// Copy & Verify mode (FR-25).
    pub verify: bool,
    /// Write the `.xxh64` checksum file (FR-29).
    pub write_checksum_file: bool,
    pub copy: CopyConfig,
    /// Files up to this size go to the small-file lanes (RFD §7.2).
    pub small_file_threshold: u64,
    pub small_file_lanes: usize,
    pub large_file_lanes: usize,
    pub verify_lanes: usize,
    /// How often `Event::Progress` is emitted.
    pub progress_interval: Duration,
    #[doc(hidden)]
    pub hooks: Hooks,
}

impl Default for JobOptions {
    fn default() -> Self {
        Self {
            verify: true,
            write_checksum_file: true,
            copy: CopyConfig::default(),
            small_file_threshold: 8 << 20,
            small_file_lanes: 8,
            large_file_lanes: 1,
            verify_lanes: 2,
            progress_interval: Duration::from_millis(50),
            hooks: Hooks::default(),
        }
    }
}

/// Fault injection for tests. Not part of the stable API.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Hooks {
    /// Called with the partial file after each copy attempt (0 = first), before verifying.
    pub after_copy: Option<fn(&Path, u32)>,
}

/// Lets another thread cancel a running job (FR-23).
#[derive(Debug, Default)]
pub struct JobControl {
    stop: AtomicBool,
}

impl JobControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stops the job: no new files start, in-flight partial files are removed.
    pub fn cancel(&self) {
        self.stop.store(true, Relaxed);
    }

    pub fn is_stopped(&self) -> bool {
        self.stop.load(Relaxed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Phase {
    Copying = 0,
    Verifying = 1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    Copied,
    Verified,
    Failed(FileError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOutcome {
    pub rel: PathBuf,
    pub size: u64,
    /// Source hash; `None` if the file failed before it was hashed.
    pub hash: Option<u64>,
    pub status: FileStatus,
    pub elapsed: Duration,
}

/// A file currently being copied or verified (RFD §5.3, "Active files").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveFile {
    /// Index in `Selection::files`; stable for the whole job.
    pub id: usize,
    pub rel: PathBuf,
    pub size: u64,
    pub phase: Phase,
    /// Bytes done in the current phase.
    pub bytes_done: u64,
}

/// Raw counters; the UI derives speed and ETA from successive snapshots.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Progress {
    pub total_files: u64,
    pub total_bytes: u64,
    pub files_done: u64,
    pub copied_bytes: u64,
    pub verified_bytes: u64,
    pub active: Vec<ActiveFile>,
}

#[derive(Debug, Clone)]
pub enum Event {
    Progress(Progress),
    FileFinished(FileOutcome),
}

#[derive(Debug, Clone)]
pub struct JobReport {
    /// In the order files finished.
    pub outcomes: Vec<FileOutcome>,
    /// Files never started because the job was cancelled or hit a fatal error.
    pub not_started: u64,
    pub checksum_file: Option<PathBuf>,
    pub checksum_error: Option<String>,
    /// `None` when not verifying.
    pub cache_bypass: Option<CacheBypass>,
    pub fatal: Option<FileError>,
    pub cancelled: bool,
    pub elapsed: Duration,
}

impl JobReport {
    pub fn failed(&self) -> impl Iterator<Item = &FileOutcome> {
        self.outcomes
            .iter()
            .filter(|o| matches!(o.status, FileStatus::Failed(_)))
    }

    pub fn is_success(&self) -> bool {
        self.fatal.is_none()
            && !self.cancelled
            && self.not_started == 0
            && self.failed().next().is_none()
    }
}

/// Copies every file in `sel` into `dest`. Blocks until done; call from a worker thread.
/// `on_event` is called from several threads.
pub fn run_job(
    sel: &Selection,
    dest: &Path,
    opts: &JobOptions,
    control: &JobControl,
    on_event: &(dyn Fn(Event) + Sync),
) -> JobReport {
    let started = Instant::now();
    for dir in &sel.dirs {
        // A failure here surfaces as a per-file write error.
        let _ = fs::create_dir_all(dest.join(dir));
    }
    let clashes = find_name_clashes(sel);
    let runner = Runner::new(sel, dest, opts, control, on_event, clashes);
    let (small, large): (Vec<usize>, Vec<usize>) =
        (0..sel.files.len()).partition(|&i| sel.files[i].size <= opts.small_file_threshold);
    let small = Queue::new(small);
    let large = Queue::new(large);
    let finished = AtomicBool::new(false);
    let (verify_tx, verify_rx) = mpsc::channel::<VerifyTask>();
    let verify_rx = Mutex::new(verify_rx);

    std::thread::scope(|s| {
        let ticker = s.spawn(|| {
            while !finished.load(Relaxed) {
                std::thread::sleep(opts.progress_interval);
                runner.emit_progress();
            }
        });
        let mut workers = Vec::new();
        for (queue, lanes) in [
            (&small, opts.small_file_lanes),
            (&large, opts.large_file_lanes),
        ] {
            for _ in 0..lanes.max(1) {
                let (runner, verify_tx) = (&runner, verify_tx.clone());
                workers.push(s.spawn(move || runner.copy_lane(queue, &verify_tx)));
            }
        }
        // Verify lanes end when every copy lane has dropped its sender.
        drop(verify_tx);
        if opts.verify {
            for _ in 0..opts.verify_lanes.max(1) {
                let (runner, verify_rx) = (&runner, &verify_rx);
                workers.push(s.spawn(move || runner.verify_lane(verify_rx)));
            }
        }
        for worker in workers {
            worker.join().expect("worker thread panicked");
        }
        finished.store(true, Relaxed);
        ticker.join().expect("progress thread panicked");
        runner.emit_progress();
    });

    let bypass_unavailable = runner.bypass_unavailable.load(Relaxed);
    let outcomes = runner
        .outcomes
        .into_inner()
        .expect("outcomes lock poisoned");
    let fatal = runner.fatal.into_inner().expect("fatal lock poisoned");
    let (checksum_file, checksum_error) = if opts.write_checksum_file {
        write_checksum(dest, &outcomes)
    } else {
        (None, None)
    };
    make_durable(dest, &sel.dirs);
    JobReport {
        not_started: (sel.files.len() - outcomes.len()) as u64,
        outcomes,
        checksum_file,
        checksum_error,
        cache_bypass: opts.verify.then_some(if bypass_unavailable {
            CacheBypass::Unavailable
        } else {
            CacheBypass::Active
        }),
        cancelled: control.is_stopped() && fatal.is_none(),
        fatal,
        elapsed: started.elapsed(),
    }
}

/// Marks every file whose destination path repeats an earlier one, ignoring case:
/// two lanes writing the same name would corrupt each other, and case-insensitive
/// destinations (macOS, Windows, exFAT) treat `a.txt` and `A.TXT` as one file.
fn find_name_clashes(sel: &Selection) -> Vec<bool> {
    let mut seen = HashSet::new();
    sel.files
        .iter()
        .map(|f| !seen.insert(checksum_file::slash_path(&f.rel).to_lowercase()))
        .collect()
}

fn write_checksum(dest: &Path, outcomes: &[FileOutcome]) -> (Option<PathBuf>, Option<String>) {
    let entries: Vec<(PathBuf, u64)> = outcomes
        .iter()
        .filter(|o| matches!(o.status, FileStatus::Copied | FileStatus::Verified))
        .filter_map(|o| o.hash.map(|h| (o.rel.clone(), h)))
        .collect();
    if entries.is_empty() {
        return (None, None);
    }
    match checksum_file::write(dest, &entries, Local::now()) {
        Ok(path) => (Some(path), None),
        Err(e) => (None, Some(e.to_string())),
    }
}

/// Makes the job durable: one fsync per directory for the renames, then one
/// drive-cache flush for the whole volume (RFD §7.4).
fn make_durable(dest: &Path, dirs: &[PathBuf]) {
    #[cfg(unix)]
    for dir in dirs
        .iter()
        .map(|d| dest.join(d))
        .chain([dest.to_path_buf()])
    {
        if let Ok(f) = fs::File::open(&dir) {
            let _ = os::sync_file(&f);
        }
    }
    #[cfg(not(unix))]
    let _ = dirs;
    let _ = os::full_barrier(dest);
}

/// Lock-free work list shared by the lanes of one kind.
struct Queue {
    items: Vec<usize>,
    next: AtomicUsize,
}

impl Queue {
    fn new(items: Vec<usize>) -> Self {
        Self {
            items,
            next: AtomicUsize::new(0),
        }
    }

    fn pop(&self) -> Option<usize> {
        self.items.get(self.next.fetch_add(1, Relaxed)).copied()
    }
}

/// Live state of one in-flight file.
struct Slot {
    id: usize,
    rel: PathBuf,
    size: u64,
    phase: AtomicU8,
    bytes: AtomicU64,
    /// True once this file's size is included in `Runner::copied_done`.
    copy_counted: AtomicBool,
}

impl Slot {
    fn set_phase(&self, phase: Phase) {
        self.bytes.store(0, Relaxed);
        self.phase.store(phase as u8, Relaxed);
    }

    fn phase(&self) -> Phase {
        if self.phase.load(Relaxed) == Phase::Verifying as u8 {
            Phase::Verifying
        } else {
            Phase::Copying
        }
    }
}

struct VerifyTask {
    slot: Arc<Slot>,
    idx: usize,
    partial: PartialCopy,
    started: Instant,
}

struct Runner<'a> {
    sel: &'a Selection,
    dest: &'a Path,
    opts: &'a JobOptions,
    copy_cfg: CopyConfig,
    control: &'a JobControl,
    on_event: &'a (dyn Fn(Event) + Sync),
    /// Indexed like `Selection::files`; true = skip with `FileError::NameClash`.
    clashes: Vec<bool>,
    copied_done: AtomicU64,
    verified_done: AtomicU64,
    files_done: AtomicU64,
    active: Mutex<Vec<Arc<Slot>>>,
    outcomes: Mutex<Vec<FileOutcome>>,
    fatal: Mutex<Option<FileError>>,
    bypass_unavailable: AtomicBool,
}

impl<'a> Runner<'a> {
    fn new(
        sel: &'a Selection,
        dest: &'a Path,
        opts: &'a JobOptions,
        control: &'a JobControl,
        on_event: &'a (dyn Fn(Event) + Sync),
        clashes: Vec<bool>,
    ) -> Self {
        let copy_cfg = CopyConfig {
            uncached_write: opts.verify,
            ..opts.copy.clone()
        };
        Self {
            sel,
            dest,
            opts,
            copy_cfg,
            control,
            on_event,
            clashes,
            copied_done: AtomicU64::new(0),
            verified_done: AtomicU64::new(0),
            files_done: AtomicU64::new(0),
            active: Mutex::new(Vec::new()),
            outcomes: Mutex::new(Vec::new()),
            fatal: Mutex::new(None),
            bypass_unavailable: AtomicBool::new(false),
        }
    }

    fn copy_lane(&self, queue: &Queue, verify_tx: &mpsc::Sender<VerifyTask>) {
        while !self.control.is_stopped() {
            let Some(idx) = queue.pop() else { break };
            self.copy_one(idx, verify_tx);
        }
    }

    fn copy_one(&self, idx: usize, verify_tx: &mpsc::Sender<VerifyTask>) {
        let entry = &self.sel.files[idx];
        let final_path = self.dest.join(&entry.rel);
        let started = Instant::now();
        let slot = self.begin(idx, entry);
        let blocked = if self.clashes[idx] {
            Some(FileError::NameClash)
        } else if fs::symlink_metadata(&final_path).is_ok() {
            Some(FileError::AlreadyExists)
        } else {
            None
        };
        if let Some(e) = blocked {
            return self.finish(&slot, entry, None, FileStatus::Failed(e), started);
        }
        let partial = match self.copy_attempt(entry, &final_path, &slot, 0) {
            Ok(partial) => partial,
            Err(e) => return self.finish(&slot, entry, None, FileStatus::Failed(e), started),
        };
        self.count_copied(&slot);
        if self.opts.verify {
            slot.set_phase(Phase::Verifying);
            let task = VerifyTask {
                slot,
                idx,
                partial,
                started,
            };
            // Verify lanes live until every sender is dropped, so this cannot fail.
            verify_tx.send(task).expect("verify lanes are running");
            return;
        }
        let hash = partial.hash;
        match copy::commit(&partial.partial, &final_path) {
            Ok(()) => self.finish(&slot, entry, Some(hash), FileStatus::Copied, started),
            Err(e) => {
                let _ = fs::remove_file(&partial.partial);
                self.finish(&slot, entry, Some(hash), FileStatus::Failed(e), started)
            }
        }
    }

    fn copy_attempt(
        &self,
        entry: &ScanEntry,
        final_path: &Path,
        slot: &Slot,
        attempt: u32,
    ) -> Result<PartialCopy, FileError> {
        slot.set_phase(Phase::Copying);
        let partial = copy::copy_to_partial(
            &entry.source,
            final_path,
            &self.copy_cfg,
            &|b| slot.bytes.store(b, Relaxed),
            &self.control.stop,
        )?;
        if let Some(hook) = self.opts.hooks.after_copy {
            hook(&partial.partial, attempt);
        }
        Ok(partial)
    }

    fn verify_lane(&self, rx: &Mutex<mpsc::Receiver<VerifyTask>>) {
        loop {
            let task = match rx.lock().expect("verify queue lock poisoned").recv() {
                Ok(task) => task,
                Err(_) => break,
            };
            self.verify_one(task);
        }
    }

    /// Verifies the partial file, re-copying once on mismatch (FR-27), then commits it.
    fn verify_one(&self, task: VerifyTask) {
        let VerifyTask {
            slot,
            idx,
            mut partial,
            started,
        } = task;
        let entry = &self.sel.files[idx];
        let final_path = self.dest.join(&entry.rel);
        let mut attempt = 0;
        let result = loop {
            slot.set_phase(Phase::Verifying);
            let (actual, bypass) = match verify::hash_from_device(
                &partial.partial,
                self.copy_cfg.buffer_size,
                &|b| slot.bytes.store(b, Relaxed),
                &self.control.stop,
            ) {
                Ok(v) => v,
                Err(e) => break Err(e),
            };
            if bypass == CacheBypass::Unavailable {
                self.bypass_unavailable.store(true, Relaxed);
            }
            if actual == partial.hash {
                break copy::commit(&partial.partial, &final_path).map(|()| partial.hash);
            }
            let _ = fs::remove_file(&partial.partial);
            if attempt == 1 {
                break Err(FileError::HashMismatch {
                    expected: hash::to_hex(partial.hash),
                    actual: hash::to_hex(actual),
                });
            }
            attempt += 1;
            partial = match self.copy_attempt(entry, &final_path, &slot, attempt) {
                Ok(p) => p,
                Err(e) => break Err(e),
            };
        };
        match result {
            Ok(hash) => self.finish(&slot, entry, Some(hash), FileStatus::Verified, started),
            Err(e) => {
                let _ = fs::remove_file(&partial.partial);
                self.finish(&slot, entry, None, FileStatus::Failed(e), started)
            }
        }
    }

    fn begin(&self, idx: usize, entry: &ScanEntry) -> Arc<Slot> {
        let slot = Arc::new(Slot {
            id: idx,
            rel: entry.rel.clone(),
            size: entry.size,
            phase: AtomicU8::new(Phase::Copying as u8),
            bytes: AtomicU64::new(0),
            copy_counted: AtomicBool::new(false),
        });
        self.active
            .lock()
            .expect("active lock poisoned")
            .push(slot.clone());
        slot
    }

    fn count_copied(&self, slot: &Slot) {
        if !slot.copy_counted.swap(true, Relaxed) {
            self.copied_done.fetch_add(slot.size, Relaxed);
        }
    }

    fn finish(
        &self,
        slot: &Arc<Slot>,
        entry: &ScanEntry,
        hash: Option<u64>,
        status: FileStatus,
        started: Instant,
    ) {
        self.active
            .lock()
            .expect("active lock poisoned")
            .retain(|s| !Arc::ptr_eq(s, slot));
        // A finished file counts as fully processed, so the bars reach 100 % even with failures.
        self.count_copied(slot);
        if self.opts.verify {
            self.verified_done.fetch_add(slot.size, Relaxed);
        }
        self.files_done.fetch_add(1, Relaxed);
        if let FileStatus::Failed(e) = &status
            && e.is_fatal()
        {
            self.fatal
                .lock()
                .expect("fatal lock poisoned")
                .get_or_insert(e.clone());
            self.control.cancel();
        }
        let outcome = FileOutcome {
            rel: entry.rel.clone(),
            size: entry.size,
            hash,
            status,
            elapsed: started.elapsed(),
        };
        self.outcomes
            .lock()
            .expect("outcomes lock poisoned")
            .push(outcome.clone());
        (self.on_event)(Event::FileFinished(outcome));
    }

    fn emit_progress(&self) {
        (self.on_event)(Event::Progress(self.progress()));
    }

    fn progress(&self) -> Progress {
        let active = self.active.lock().expect("active lock poisoned");
        let mut copied = self.copied_done.load(Relaxed);
        let mut verified = self.verified_done.load(Relaxed);
        let files = active
            .iter()
            .map(|s| {
                let phase = s.phase();
                let bytes_done = s.bytes.load(Relaxed);
                match phase {
                    Phase::Copying if !s.copy_counted.load(Relaxed) => copied += bytes_done,
                    Phase::Copying => {}
                    Phase::Verifying => verified += bytes_done,
                }
                ActiveFile {
                    id: s.id,
                    rel: s.rel.clone(),
                    size: s.size,
                    phase,
                    bytes_done,
                }
            })
            .collect();
        let total_bytes = self.sel.total_bytes;
        Progress {
            total_files: self.sel.files.len() as u64,
            total_bytes,
            files_done: self.files_done.load(Relaxed),
            copied_bytes: copied.min(total_bytes),
            verified_bytes: verified.min(total_bytes),
            active: files,
        }
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p secopy-core --test job`
Expected: `15 passed` (14 on Windows). Then run everything:
`SECOPY_REQUIRE_XXHSUM=1 cargo test --workspace`. Expected: all pass.

- [ ] **Step 6: Commit the engine**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/secopy-core
git commit -m "feat(core): run copy jobs with verify lanes, retries and progress events"
```

- [ ] **Step 7: Record the two new rules in the RFD**

In `docs/rfd/0001-secopy.md`, add this row to the §6.4 table, after FR-17:

```markdown
| FR-17a | Files whose destination paths are equal when compared **ignoring case** (e.g. `x/a.txt` and `y/A.TXT` picked as loose files) are never copied over each other: the first one is copied, the others fail with "another file in this copy has the same name". | M |
```

Replace the bullets under **§7.4 Durability** with:

```markdown
- Each file is flushed to the device (`fsync`) before its rename (FR-18). On macOS this is
  plain `fsync`, not `F_FULLFSYNC`: flushing the drive's cache per file made 20,000 small
  files take 80 s instead of 2.6 s in the M0 prototype.
- At the end of the job: one `fsync` per created directory (makes the renames durable), then
  one drive-cache flush for the whole volume (`F_FULLFSYNC` on macOS).
- If the destination is removable, show "Safe to eject".
```

Add to the §14 decision log:

```markdown
| 2026-09-26 | Same-name files in one job (compared ignoring case) fail with a clear error instead of overwriting each other (FR-17a). |
| 2026-09-26 | Durability: plain `fsync` per file, one drive-cache flush per job (§7.4). |
```

```bash
git add docs/rfd/0001-secopy.md
git commit -m "docs(rfd): record name clash rule and flush strategy"
```

---

### Task 8: Developer CLI

**Files:**
- Create: `crates/secopy-cli/Cargo.toml`, `crates/secopy-cli/src/main.rs`, `crates/secopy-cli/tests/cli.rs`

**Interfaces:**
- Consumes: the whole public `secopy-core` API (Tasks 2–7).
- Produces: the binary `secopy-cli <SOURCES>... --to <DEST> [--contents] [--verify] [--ext LIST] [--no-checksum] [--include-hidden]`.
  - Exit codes: `0` all good, `1` some files failed, cancelled or fatal, `2` usage error.
  - Progress goes to stderr and the summary to stdout, so benchmarks can discard the progress output.

- [ ] **Step 1: Create the crate manifest**

`crates/secopy-cli/Cargo.toml`:

```toml
[package]
name = "secopy-cli"
description = "Command-line front-end for the Secopy engine (development and benchmarks)"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true

[dependencies]
clap.workspace = true
ctrlc.workspace = true
secopy-core.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 2: Write the failing tests**

`crates/secopy-cli/tests/cli.rs`:

```rust
use std::fs;
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_secopy-cli"))
}

#[test]
fn copies_a_folder_with_verify_and_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("CARD");
    let dest = dir.path().join("dest");
    fs::create_dir_all(src.join("clips")).unwrap();
    fs::create_dir_all(&dest).unwrap();
    fs::write(src.join("clips/A001.mov"), b"movie").unwrap();

    let out = cli()
        .arg(&src)
        .arg("--to")
        .arg(&dest)
        .arg("--verify")
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        fs::read(dest.join("CARD/clips/A001.mov")).unwrap(),
        b"movie"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("1 files ok, 0 failed"), "{stdout}");
    assert!(stdout.contains("checksum file:"), "{stdout}");
}

#[test]
fn contents_flag_skips_the_folder_itself() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("CARD");
    let dest = dir.path().join("dest");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dest).unwrap();
    fs::write(src.join("a.wav"), b"a").unwrap();

    let out = cli()
        .arg(&src)
        .arg("--to")
        .arg(&dest)
        .arg("--contents")
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(dest.join("a.wav").is_file());
}

#[test]
fn mixing_a_folder_and_files_is_a_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.wav"), b"a").unwrap();
    fs::create_dir_all(dir.path().join("folder")).unwrap();

    let out = cli()
        .arg(dir.path().join("a.wav"))
        .arg(dir.path().join("folder"))
        .arg("--to")
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn a_failed_file_gives_exit_code_one() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    fs::write(dir.path().join("a.wav"), b"new").unwrap();
    fs::write(dest.join("a.wav"), b"old").unwrap();

    let out = cli()
        .arg(dir.path().join("a.wav"))
        .arg("--to")
        .arg(&dest)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(fs::read(dest.join("a.wav")).unwrap(), b"old");
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p secopy-cli`
Expected: build error, because `src/main.rs` doesn't exist yet (`can't find bin target`).

- [ ] **Step 4: Implement the CLI**

`crates/secopy-cli/src/main.rs`:

```rust
//! Command-line front-end for the Secopy engine, used for development and benchmarks (RFD §10, M0).

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clap::Parser;
use secopy_core::filter::ExtensionFilter;
use secopy_core::job::{self, Event, FileStatus, JobControl, JobOptions, JobReport, Progress};
use secopy_core::scan::{self, ScanOptions};
use secopy_core::source::{DirMode, Source};

#[derive(Parser, Debug)]
#[command(
    name = "secopy-cli",
    version,
    about = "Fast file copy with xxHash64 verification"
)]
struct Args {
    /// One directory, or one or more files.
    #[arg(required = true)]
    sources: Vec<PathBuf>,
    /// Destination directory (must exist).
    #[arg(long, short = 't')]
    to: PathBuf,
    /// Copy only what is inside the source directory, not the directory itself.
    #[arg(long)]
    contents: bool,
    /// Re-read every copy from the destination and compare hashes.
    #[arg(long)]
    verify: bool,
    /// Only copy these extensions, e.g. "mov,wav". "(none)" means files without extension.
    #[arg(long)]
    ext: Option<String>,
    /// Do not write the .xxh64 checksum file.
    #[arg(long)]
    no_checksum: bool,
    /// Include hidden files and folders.
    #[arg(long)]
    include_hidden: bool,
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::from(2)
        }
    }
}

fn run(args: Args) -> Result<ExitCode, String> {
    if !args.to.is_dir() {
        return Err(format!(
            "destination {} is not a directory",
            args.to.display()
        ));
    }
    let source = source_from(&args)?;
    let scan = scan::scan(
        &source,
        &ScanOptions {
            include_hidden: args.include_hidden,
        },
    )
    .map_err(|e| e.to_string())?;
    for p in &scan.problems {
        eprintln!("warning: {}: {}", p.path.display(), p.message);
    }
    for link in &scan.skipped_symlinks {
        eprintln!("skipped symlink: {}", link.display());
    }
    let filter = args
        .ext
        .as_deref()
        .map(ExtensionFilter::parse_list)
        .unwrap_or_default();
    let selection = scan.select(&filter);
    eprintln!(
        "{} files, {} ({} hidden items skipped)",
        selection.files.len(),
        fmt_bytes(selection.total_bytes),
        scan.skipped_hidden
    );

    let opts = JobOptions {
        verify: args.verify,
        write_checksum_file: !args.no_checksum,
        ..JobOptions::default()
    };
    let control = Arc::new(JobControl::new());
    let handler_control = control.clone();
    ctrlc::set_handler(move || handler_control.cancel()).map_err(|e| e.to_string())?;

    let started = Instant::now();
    let last_print = Mutex::new(Instant::now());
    let report = job::run_job(
        &selection,
        &args.to,
        &opts,
        &control,
        &|event| match event {
            Event::Progress(p) => {
                let mut last = last_print.lock().unwrap();
                if last.elapsed() >= Duration::from_millis(500) {
                    *last = Instant::now();
                    eprint!("\r{}", progress_line(&p, started.elapsed()));
                }
            }
            Event::FileFinished(o) => {
                if let FileStatus::Failed(e) = &o.status {
                    eprintln!("\rFAILED {}: {e}", o.rel.display());
                }
            }
        },
    );
    eprintln!();
    print_summary(&report, selection.total_bytes);
    Ok(if report.is_success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// One directory → directory source; otherwise only files are allowed (RFD Q6).
fn source_from(args: &Args) -> Result<Source, String> {
    if let [only] = args.sources.as_slice()
        && only.is_dir()
    {
        let mode = if args.contents {
            DirMode::ContentsOnly
        } else {
            DirMode::FolderItself
        };
        return Ok(Source::Directory {
            path: only.clone(),
            mode,
        });
    }
    if let Some(dir) = args.sources.iter().find(|p| p.is_dir()) {
        return Err(format!(
            "{} is a directory; pass either one directory or only files",
            dir.display()
        ));
    }
    Ok(Source::Files(args.sources.clone()))
}

fn progress_line(p: &Progress, elapsed: Duration) -> String {
    let speed = p.copied_bytes as f64 / elapsed.as_secs_f64().max(0.001);
    format!(
        "copied {} / {} ({:.1} %)  verified {}  files {}/{}  {}/s   ",
        fmt_bytes(p.copied_bytes),
        fmt_bytes(p.total_bytes),
        percent(p.copied_bytes, p.total_bytes),
        fmt_bytes(p.verified_bytes),
        p.files_done,
        p.total_files,
        fmt_bytes(speed as u64),
    )
}

fn print_summary(report: &JobReport, total_bytes: u64) {
    let secs = report.elapsed.as_secs_f64().max(0.001);
    let failed = report.failed().count();
    println!(
        "{} files ok, {} failed, {} not started",
        report.outcomes.len() - failed,
        failed,
        report.not_started
    );
    println!(
        "{} in {:.2} s ({}/s)",
        fmt_bytes(total_bytes),
        secs,
        fmt_bytes((total_bytes as f64 / secs) as u64)
    );
    if let Some(bypass) = report.cache_bypass {
        println!("verify cache bypass: {bypass:?}");
    }
    if let Some(path) = &report.checksum_file {
        println!("checksum file: {}", path.display());
    }
    if let Some(e) = &report.checksum_error {
        println!("checksum file NOT written: {e}");
    }
    if let Some(e) = &report.fatal {
        println!("stopped: {e}");
    }
    if report.cancelled {
        println!("cancelled");
    }
}

fn percent(done: u64, total: u64) -> f64 {
    if total == 0 {
        100.0
    } else {
        done as f64 * 100.0 / total as f64
    }
}

/// Decimal units, like Finder and Explorer's "size on disk" dialogs.
fn fmt_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p secopy-cli`
Expected: `4 passed`.

- [ ] **Step 6: Try it by hand**

Run: `cargo run --release -p secopy-cli -- crates --to "$(mktemp -d)" --verify`
Expected: a progress line, then `N files ok, 0 failed, 0 not started`, `verify cache bypass: Active`, and the path of the `.xxh64` file.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/secopy-cli Cargo.lock
git commit -m "feat(cli): add secopy-cli for development and benchmarks"
```

---

### Task 9: Benchmarks and the M0 performance check

**Files:**
- Create: `scripts/bench.sh`, `scripts/bench.ps1`, `docs/benchmarks/<date>-<host>.md` (generated)

**Interfaces:**
- Consumes: `secopy-cli` (Task 8).
- Produces: benchmark reports that are compared with NFR-1..NFR-3.

- [ ] **Step 1: Add the macOS/Linux benchmark script**

`scripts/bench.sh` (then `chmod +x scripts/bench.sh`):

```bash
#!/usr/bin/env bash
# Compares Secopy with cp and rsync on real hardware (RFD NFR-1..NFR-3, milestone M0).
#
# Usage: scripts/bench.sh [--generate] [--purge] <source-dir> <dest-dir>
#   --generate  first fill <source-dir> with the reference data sets:
#               large/ (LARGE_GIB x 1 GiB files, default 4)
#               small/ (SMALL_COUNT x 16 KiB files, default 20000)
#   --purge     drop the OS file cache before every run (asks for sudo), so the
#               source is read from the device and not from RAM
#
# Put <source-dir> and <dest-dir> on the devices you want to measure
# (e.g. card reader -> SSD). Needs hyperfine and python3.
# Results go to docs/benchmarks/<date>-<host>.md.
set -euo pipefail

generate=false
purge=false
while [[ $# -gt 0 && $1 == --* ]]; do
  case $1 in
    --generate) generate=true ;;
    --purge) purge=true ;;
    *) echo "unknown flag: $1" >&2; exit 2 ;;
  esac
  shift
done
[[ $# -eq 2 ]] || { sed -n '4,15p' "$0"; exit 2; }
src=$1
dest=$2
command -v hyperfine >/dev/null || { echo "hyperfine is required (brew/apt install hyperfine)" >&2; exit 2; }

repo=$(cd "$(dirname "$0")/.." && pwd)
cargo build --release -p secopy-cli --manifest-path "$repo/Cargo.toml"
secopy="$repo/target/release/secopy-cli"

if $generate; then
  mkdir -p "$src"
  python3 - "$src" "${LARGE_GIB:-4}" "${SMALL_COUNT:-20000}" <<'PY'
import os, sys
root, large, small = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
os.makedirs(f"{root}/large", exist_ok=True)
chunk = os.urandom(1 << 20)
for i in range(large):
    with open(f"{root}/large/clip{i:02}.bin", "wb") as f:
        for _ in range(1024):
            f.write(chunk)
data = os.urandom(16 << 10)
for i in range(small):
    d = f"{root}/small/d{i % 100:03}"
    os.makedirs(d, exist_ok=True)
    with open(f"{d}/f{i:05}.bin", "wb") as f:
        f.write(data)
PY
fi

drop_caches=true
if $purge; then
  case $(uname) in
    Darwin) drop_caches="sync && sudo purge" ;;
    Linux) drop_caches="sync && echo 3 | sudo tee /proc/sys/vm/drop_caches >/dev/null" ;;
  esac
  sudo -v
fi

out_dir="$repo/docs/benchmarks"
mkdir -p "$out_dir"
report="$out_dir/$(date +%Y-%m-%d)-$(hostname -s).md"
{
  echo "# Benchmark $(date '+%Y-%m-%d %H:%M') on $(hostname -s)"
  echo
  echo "- OS: $(uname -sr)"
  echo "- Source: \`$src\`"
  echo "- Destination: \`$dest\`"
  echo "- Cache purged between runs: $purge"
  echo "- Secopy: $("$secopy" --version)"
} >"$report"

for set in large small; do
  [[ -d "$src/$set" ]] || continue
  run="$dest/secopy-bench-run"
  prepare="rm -rf '$run' && mkdir -p '$run' && $drop_caches"
  tmp=$(mktemp)
  hyperfine --runs 3 --prepare "$prepare" --export-markdown "$tmp" \
    -n "cp -R" "cp -R '$src/$set' '$run/'" \
    -n "rsync -a" "rsync -a '$src/$set' '$run/'" \
    -n "secopy copy" "'$secopy' '$src/$set' --to '$run'" \
    -n "secopy copy+verify" "'$secopy' '$src/$set' --to '$run' --verify"
  { echo; echo "## $set/ ($(du -sh "$src/$set" | cut -f1))"; echo; cat "$tmp"; } >>"$report"
  rm -rf "$tmp" "$run"
done
echo "Results: $report"
```

- [ ] **Step 2: Add the Windows benchmark script**

`scripts/bench.ps1`. This hasn't been run yet; the first Windows run in Step 5 validates it:

```powershell
# Compares Secopy with robocopy on Windows (RFD NFR-1..NFR-3, milestone M0).
#
# Usage: scripts\bench.ps1 -Source D:\bench\src -Dest E:\bench [-Generate]
#   -Generate  first fill -Source with large\ (LargeGiB x 1 GiB) and small\ (SmallCount x 16 KiB)
# Reports the median of 3 runs to docs\benchmarks\<date>-<computer>.md.
param(
    [Parameter(Mandatory)] [string] $Source,
    [Parameter(Mandatory)] [string] $Dest,
    [switch] $Generate,
    [int] $LargeGiB = 4,
    [int] $SmallCount = 20000
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path "$PSScriptRoot\..").Path
cargo build --release -p secopy-cli --manifest-path "$repo\Cargo.toml"
if ($LASTEXITCODE) { exit $LASTEXITCODE }
$secopy = "$repo\target\release\secopy-cli.exe"

if ($Generate) {
    $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    New-Item -ItemType Directory -Force "$Source\large" | Out-Null
    $chunk = New-Object byte[] (1MB)
    $rng.GetBytes($chunk)
    for ($i = 0; $i -lt $LargeGiB; $i++) {
        $f = [System.IO.File]::Create(("{0}\large\clip{1:D2}.bin" -f $Source, $i))
        for ($j = 0; $j -lt 1024; $j++) { $f.Write($chunk, 0, $chunk.Length) }
        $f.Dispose()
    }
    $data = New-Object byte[] (16KB)
    $rng.GetBytes($data)
    for ($i = 0; $i -lt $SmallCount; $i++) {
        $dir = "{0}\small\d{1:D3}" -f $Source, ($i % 100)
        if ($i -lt 100) { New-Item -ItemType Directory -Force $dir | Out-Null }
        [System.IO.File]::WriteAllBytes(("{0}\f{1:D5}.bin" -f $dir, $i), $data)
    }
}

# Runs $Run three times into a fresh target folder and returns the median seconds.
function Measure-Median([scriptblock] $Run) {
    $target = "$Dest\secopy-bench-run"
    $times = foreach ($n in 1..3) {
        if (Test-Path $target) { Remove-Item -Recurse -Force $target }
        New-Item -ItemType Directory $target | Out-Null
        (Measure-Command { & $Run $target }).TotalSeconds
    }
    Remove-Item -Recurse -Force $target
    ($times | Sort-Object)[1]
}

$lines = @(
    ("# Benchmark {0:yyyy-MM-dd HH:mm} on $env:COMPUTERNAME" -f (Get-Date)),
    '',
    "- OS: $([System.Environment]::OSVersion.VersionString)",
    "- Source: ``$Source``",
    "- Destination: ``$Dest``",
    "- Median of 3 runs; OS cache not purged",
    "- Secopy: $(& $secopy --version)"
)
foreach ($set in 'large', 'small') {
    if (-not (Test-Path "$Source\$set")) { continue }
    $results = [ordered]@{
        'robocopy /E /MT:8'  = Measure-Median { param($t) robocopy "$Source\$set" "$t\$set" /E /MT:8 /NFL /NDL /NJH /NJS /NP | Out-Null }
        'secopy copy'        = Measure-Median { param($t) & $secopy "$Source\$set" --to $t 2>&1 | Out-Null }
        'secopy copy+verify' = Measure-Median { param($t) & $secopy "$Source\$set" --to $t --verify 2>&1 | Out-Null }
    }
    $lines += @('', "## $set/", '', '| Command | Median [s] |', '|:---|---:|')
    foreach ($k in $results.Keys) { $lines += ('| `{0}` | {1:N2} |' -f $k, $results[$k]) }
}
$out = "$repo\docs\benchmarks"
New-Item -ItemType Directory -Force $out | Out-Null
$report = "$out\{0:yyyy-MM-dd}-$env:COMPUTERNAME.md" -f (Get-Date)
Set-Content -Path $report -Value $lines -Encoding utf8
Write-Output "Results: $report"
```

- [ ] **Step 3: Smoke-test the script**

Run: `brew install hyperfine && LARGE_GIB=1 SMALL_COUNT=5000 scripts/bench.sh --generate /tmp/secopy-bench/src /tmp/secopy-bench/dst`
Expected: two hyperfine tables and `Results: docs/benchmarks/<date>-<host>.md`. In the prototype run on this Mac, Secopy copy was about 1.5× faster than `cp -R` on large files and 2.3× faster on small files. Delete this smoke-run report; it's warm-cache, same-disk data.

```bash
rm docs/benchmarks/*.md
git add scripts/
git commit -m "chore: add benchmark scripts"
```

- [ ] **Step 4: Run the real benchmarks (the user, on real hardware)**

Ask the user to run these with `--purge`, each into its own report:

1. Internal SSD → internal SSD.
2. Card reader (a real camera card) → internal SSD.
3. Internal SSD → external USB HDD or SSD.
4. On a Windows machine: `scripts\bench.ps1 -Source <card> -Dest <ssd> -Generate`.

- [ ] **Step 5: Compare against the targets and record findings**

For each report, check these targets (RFD §8):
- **NFR-1:** large-file copy is at least 95 % of `cp` / `robocopy` throughput, so its time is at most 1.05× theirs.
- **NFR-2:** copy+verify time is at most 1.3× copy when the source and destination are different devices, and at most 2.1× on the same device.
- **NFR-3:** small files are within 10 % of `rsync` / `robocopy /MT`.

Append a `## Findings` section to each report: pass or fail per NFR, with the numbers. Every miss becomes a work item for plan 2. Likely suspects: per-file `FlushFileBuffers` on Windows for small files, the lane counts, and the buffer size.

```bash
git add docs/benchmarks/
git commit -m "docs: add m0 benchmark results"
```

- [ ] **Step 6: Push and check CI (ask the user first, per CLAUDE.md)**

Run: `git push origin main`
Then check GitHub → Actions:
- `ci` should be green on ubuntu, macos and windows. Windows is the first real run of the `NO_BUFFERING` and hidden-attribute code.
- `release-please` should open a PR titled `chore(main): release 0.1.0`, and the lock-file sync step should have run on it.
- Merging that PR is the user's call.
