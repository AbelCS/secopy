# Secopy

Fast, verified file copies for macOS. Copy a folder or a set of
files, optionally verify every copy with xxHash64, and get an `xxhsum`-compatible
checksum file in the destination.

> **Status:** early development, Apple Silicon Macs only. Design:
> [RFD 0001](docs/rfd/0001-secopy.md).

## Install

Download `Secopy_<version>_aarch64.dmg` from the
[latest release](https://github.com/AbelCS/secopy/releases/latest) and drag Secopy to
Applications. The app isn't signed yet, so the first time macOS refuses to open it:
right-click Secopy in Applications, choose **Open**, then **Open** again. After that it
starts normally.

## Development

Requirements: Rust via [rustup](https://rustup.rs) (the toolchain is pinned in
`rust-toolchain.toml`), Node 24 for the UI, and `xxhsum` for the checksum compatibility
test (`brew install xxhash`).

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
