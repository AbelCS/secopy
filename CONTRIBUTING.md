# Contributing to Secopy

Thanks for helping. Secopy copies footage people can't shoot again, so one rule comes before
every other: **never lose, corrupt or silently skip a file, and never report success when
something wasn't copied, verified or removed as planned.** Every change to the engine, or to
how a result is shown, is judged by that and comes with a test that fails without it.

By contributing, you agree that your contribution is licensed under the project's
[GPL-3.0-or-later](LICENSE).

## The project

| Part | Where | What |
|---|---|---|
| Engine | `crates/secopy-core` | Scan, copy, verify, mirror, check, reports. No UI. |
| App | `crates/secopy-app` | Tauri 2 shell: commands, jobs, the queue, menus, the menu bar. |
| CLI | `crates/secopy-cli` | The engine in Terminal (`secopy-cli --help`). |
| UI | `ui/` | Svelte 5 + TypeScript. |
| Design | `docs/rfd/0001-secopy.md` | Requirements (FR-x, NFR-x) and the decision log. |
| Records | `docs/superpowers/`, `docs/qa/` | How each feature was designed and built, and the reviews: history, not the current state. |

Secopy is **macOS only (Apple silicon)** by design: building for another OS is a compile
error. Read the [RFD](docs/rfd/0001-secopy.md) before changing behaviour; requirement IDs go
in comments, tests and commits where they apply.

## Build and test

Requirements: the Xcode Command Line Tools (`xcode-select --install`), Rust via
[rustup](https://rustup.rs) (`rust-toolchain.toml` selects stable with rustfmt and Clippy),
Node 24 (`nvm use` reads `.nvmrc`), and `xxhsum` for the checksum compatibility test
(`brew install xxhash`).

Everything CI checks, locally:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
SECOPY_REQUIRE_XXHSUM=1 SECOPY_DEVICE_TESTS=1 cargo test --workspace --locked
cd ui && npm ci && npm run check && npm test
```

The two variables make the `xxhsum` test fail instead of skipping, and run the fault tests on
macOS RAM disks (disk full, unplugging, FAT32/exFAT, case-sensitive APFS). Run them before
changing `copy`, `os`, `preflight` or the job runner.

Run the app with `cd ui && npm run tauri dev`; build it with `npm run tauri build`. The
engine alone: `cargo run --release -p secopy-cli -- --help`.

- **The UI's bindings** (`ui/src/lib/bindings.ts`) are generated from the Rust commands. After
  changing a command or a type the UI sees, run `SECOPY_UPDATE_BINDINGS=1 cargo test -p
  secopy-app` and commit the result; a test fails when they're out of date.
- **The gallery** shows every screen with fake data: `cd ui && npm run dev`, then open
  `/gallery.html#setup` (or `#progress`, `#summary`, `#mirror-preview`, …). Use it to look at
  a change without a real copy. `scripts/screenshots.sh` remakes the user guide's screenshots
  from it.
- **Before a release**, the app is checked by hand with a real card:
  [the checklist](docs/testing/macos-app-checklist.md).

## How to write code here

- **Tests first.** Write the test, watch it fail, then make it pass. A test that passes before
  the change proves nothing.
- **Words live in the catalogs.** Every text the app shows is in `ui/src/locales/en.json`, and
  its Spanish in `es.json`: a key added or changed in one is added or changed in the other.
  The UI uses `t("key")`; Rust sends `msg!("key", …)` and never English. Tests fail on a word
  written in a component, a key nobody uses, a Rust key the catalog lacks, or a translation
  out of step with English. See [docs/i18n.md](docs/i18n.md), which also has the terms to use.
- **Words and UI:** follow the [design system](docs/design/design-system.md) and the terms in
  [docs/i18n.md](docs/i18n.md): "directory", not "folder"; "source" and "destination"; short
  button labels.
- **Decisions** (product or technical) go in the RFD's decision log in the same change.
- Code, comments and docs in English.

## Issues, branches and pull requests

- **Every change has an issue**: a feature, a fix, docs, a refactor. Reference it in commits
  (`Refs #12`) and close it from the pull request (`Closes #12`).
- **Branches:** reuse the branch you're on if it isn't `main`; otherwise
  `<type>/<issue>-<slug>` from `main`, e.g. `fix/12-long-paths`.
- **Commits** follow [Conventional Commits](https://www.conventionalcommits.org):
  `feat(core): …`, `fix(ui): …`, `docs: …`. Types: `feat`, `fix`, `perf`, `refactor`,
  `docs`, `test`, `build`, `ci`, `chore`, `style`. Scopes: `core`, `app`, `ui`, `cli`, `rfd`,
  `ci`, `release`. Imperative, lowercase, no trailing period; one logical change per commit.
- **Pull requests** are merged with a **rebase merge**, never squashed, so each commit reaches
  the changelog. Pushed history is never rewritten (no force-push).

## Releases

Releases are automated by [release-please](https://github.com/googleapis/release-please).
Every push to `main` updates an open release pull request with the next version, from the
commit types (`fix` and `perf` bump the patch version, `feat` the minor, and a breaking change,
marked `!`, the major), and the changelog. Merging
it tags the release, builds the app and the CLI on macOS and attaches them to the GitHub
release. Anything merged before an open release pull request ships in it, so merge that one
first when it should go out alone. A commit with the footer `Release-As: 1.2.0` forces a
version. Versions, `CHANGELOG.md` and tags are never edited by hand.
