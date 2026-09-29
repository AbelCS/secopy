# Secopy

Fast, verified file copies for macOS: a camera card, a whole volume, any directory or a set
of files. Secopy is built to never lose a file and never say "done" when something wasn't
copied.

- **Copy & Verify:** every copy is read back from the destination and checked against the
  source's xxHash64 before it gets its final name; an `xxhsum`-compatible checksum file and a
  job report are written.
- **Copy presets:** a saved source and its settings, loaded in one click.
- **Queue:** set up several copies or mirrors and let them run one after another.
- **Mirror:** keep a backup identical to a directory. New and changed files are copied and
  verified; files deleted in the origin are archived for N days (or deleted), only after a
  clean copy and a preview.
- **Verify:** point at a backup, a copy or a whole drive; every file its checksum files list
  is read again and compared, so silent damage shows up. Mirrors keep a checksum file too.
- **Cancel** can also remove the files already copied, leaving the destination as it was.
- **Menu bar:** close the window during a job and it keeps going in the menu bar, with its
  progress; the window comes back from there (a setting, on by default).
- **Export and import:** settings and presets in a `.secopy` file, for a new Mac or to share
  presets; importing shows what's inside first and never overwrites anything silently.

> **Status:** early development, Apple Silicon Macs only. Secopy is macOS only by design:
> there are no Windows or Linux versions. Design:
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
and Copy presets; **⌘,** opens Settings. The tabs at the top: **⌘1** Copy, **⌘2** Mirror,
**⌘3** Verify; **⌘4** opens the Queue.

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
cargo run --release -p secopy-cli -- /path/to/source --to /path/to/backup --verify
# files that already exist but differ: keep both (default), overwrite or skip
cargo run --release -p secopy-cli -- /path/to/source --to /path/to/backup --on-conflict skip
# also write the job report (text and JSON)
cargo run --release -p secopy-cli -- /path/to/source --to /path/to/backup --report /tmp
# make the backup a mirror of the directory; --dry-run shows what would change first
cargo run --release -p secopy-cli -- /path/to/Footage --to /path/to/backup --mirror --dry-run
# read every file a backup's checksum files list again and compare (exit 1 if any changed)
cargo run --release -p secopy-cli -- --check /path/to/backup
```
