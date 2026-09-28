# Secopy

Fast, verified file copies for macOS. Copy a directory or a set of
files, optionally verify every copy with xxHash64, and get an `xxhsum`-compatible
checksum file in the destination. Queue several copies and let them run one after another.

> **Status:** early development, Apple Silicon Macs only. Design:
> [RFD 0001](docs/rfd/0001-secopy.md).

## Install

In Terminal:

```sh
curl -fsSL https://raw.githubusercontent.com/AbelCS/secopy/main/scripts/install.sh | bash
```

It downloads the latest release, checks it against its SHA-256 and installs Secopy into
Applications. Run the same command to update (quit Secopy first).

Why not just the `.dmg`: the app isn't notarized by Apple yet, and macOS 27 says a
downloaded app that isn't notarized "is damaged", with no way to open it. Files
downloaded with `curl` aren't flagged. If you did download the `.dmg` from the
[releases](https://github.com/AbelCS/secopy/releases), drag Secopy to Applications and run
`xattr -dr com.apple.quarantine /Applications/Secopy.app` once.

## Keyboard

File menu: **⌘O** choose the source, **⌘D** choose the destination, **⌘↩** start,
**⌘.** cancel. While copying, **Space** pauses and resumes. **Esc** goes back from Settings
and Profiles; **⌘,** opens Settings. **⌘1** Copy, **⌘3** Queue.

## Development

Requirements: Rust via [rustup](https://rustup.rs) (the toolchain is pinned in
`rust-toolchain.toml`), Node 24 for the UI (`nvm use` picks it from `.nvmrc`), and
`xxhsum` for the checksum compatibility test (`brew install xxhash`).

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cd ui && npm ci && npm run check && npm test
```

Run the app:

```sh
cd ui && npm run tauri dev
```

Try the CLI:

```sh
cargo run --release -p secopy-cli -- /path/to/CARD --to /path/to/backup --verify
# files that already exist but differ: keep both (default), overwrite or skip
cargo run --release -p secopy-cli -- /path/to/CARD --to /path/to/backup --on-conflict skip
# also write the job report (text and JSON)
cargo run --release -p secopy-cli -- /path/to/CARD --to /path/to/backup --report /tmp
```
