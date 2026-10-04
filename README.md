# Secopy

Fast, verified copies for macOS: a camera card, a whole drive, any directory or a set of files.
Secopy is built to never lose a file, and to never say "done" when something wasn't copied.

![The New copy screen: a source, its file types, a destination, and Start](docs/images/setup.png)

> Secopy runs on Macs with Apple silicon. It is macOS only by design: there are no Windows or
> Linux versions. It speaks English and Spanish.

## Why trust it

- **Every file is checked, not assumed.** In Copy & Verify, each copy is read back from the
  destination drive (bypassing the Mac's cache, so it really reads the drive) and compared
  with the source by its XXH128 checksum. The source is read only once.
- **A file with its final name is always complete.** Files are written under a temporary name,
  flushed to the drive and only then renamed, so a copy cut short by a pulled cable never
  looks finished.
- **Nothing is overwritten or skipped silently.** Files already at the destination are shown
  before you start, and you choose what happens to the ones that differ. Every file that
  failed is listed, with why.
- **Proof you can check later.** Each copy writes a checksum file next to it, which
  [`xxhsum`](https://github.com/Cyan4973/xxHash) can check, and a report of everything
  that happened. Verify reads a copy again months later to find silent damage. For media
  workflows, a copy can also keep an [ASC MHL](https://github.com/ascmitc/mhl)
  history, read by tools such as Silverstack and Hedge.

## What it does

- **Copy & Verify** a directory or picked files, keeping the directory structure, dates and
  hidden files. Names you never want copied (like `.DS_Store`, or `*.LRF` proxies) are left
  out by a list in Settings, and counted; each copy or preset can add its own.
- **Copy presets:** a saved source and its options (file types, the directory itself or only
  what's in it, names to ignore), loaded in one step.
- **Mirror:** keep a destination identical to an origin, one way. New and changed files are
  copied and verified; files deleted in the origin are archived for a number of days (or
  deleted), only after a preview and a clean copy.
- **Verify:** point at a copy, a backup or a whole drive; every file its checksum files list
  is read again and compared.
- **Queue:** set up several copies, mirrors and verifies, and let them run one after another.
- **Cancel** can also remove the files already copied.
- **Menu bar:** close the window during a job and it keeps going, with its progress in the
  menu bar.
- **Export and import** settings and presets in a `.secopy` file, for a new Mac or to share
  presets.

The [user guide](docs/user-guide.md) explains every screen.

## Install

In Terminal:

```sh
curl -fsSL https://raw.githubusercontent.com/AbelCS/secopy/main/scripts/install.sh | bash
```

It downloads the latest release, checks it against its SHA-256 and installs Secopy into
Applications. Run the same command to update (quit Secopy first).

Why not just the `.dmg`: the app isn't notarized by Apple yet, and macOS says a downloaded
app that isn't notarized "is damaged", with no way to open it. Files downloaded with `curl`
aren't flagged. If you did download the `.dmg` from the
[releases](https://github.com/AbelCS/secopy/releases), drag Secopy to Applications and run
`xattr -dr com.apple.quarantine /Applications/Secopy.app` once.

## Quick start

1. **Source:** drop a directory (or files) on **From**, or press **Choose…**.
2. **Destination:** drop a directory on **To**, or choose one. Secopy shows where the files
   will go, whether there's room, and anything that's already there.
3. Keep **Copy & Verify** selected, and press **Start** (⌘↩).
4. When it's done, the summary says exactly what happened. **Open checksum file** or
   **Save report…** keep the proof.

## Keyboard

**⌘O** choose the source, **⌘D** choose the destination, **⌘↩** start, **⌘.** cancel.
While a job runs, **Space** pauses and resumes. **⌘,** opens Settings; **Esc** goes back.
The tabs: **⌘1** Copy, **⌘2** Mirror, **⌘3** Verify; **⌘4** opens the Queue.

## Command line

The same engine runs in Terminal, as `secopy-cli` (in each release, or build it as below):

```sh
secopy-cli /Volumes/CARD_A/DCIM --to /Volumes/Backup/Day01 --verify
secopy-cli /Volumes/SSD/Footage --to /Volumes/NAS/Footage --mirror --dry-run
secopy-cli --check /Volumes/Backup          # exit 1 if any file changed
secopy-cli --help
```

## Documentation

- [User guide](docs/user-guide.md): every screen and what it does.
- [Contributing](CONTRIBUTING.md): building, tests and conventions.
- [Design (RFD 0001)](docs/rfd/0001-secopy.md): requirements and decisions.
- [Design system](docs/design/design-system.md) and [translations](docs/i18n.md).

## License

Copyright © 2026 Abel Castro Suárez.

Secopy is free software: you can redistribute it and/or modify it under the terms of the
[GNU General Public License](LICENSE) as published by the Free Software Foundation, either
version 3 of the License, or (at your option) any later version. It is distributed in the
hope that it will be useful, but **without any warranty**; see the license for details.
