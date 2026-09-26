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
