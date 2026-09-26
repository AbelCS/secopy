# Secopy

Cross-platform desktop app (macOS, Linux, Windows) for fast file copies with optional
xxHash64 verification and a checksum file written to the destination.

- **Source of truth:** [docs/rfd/0001-secopy.md](docs/rfd/0001-secopy.md). Read it before designing or
  changing behaviour. Requirement IDs (FR-x, NFR-x) are used in code comments, commits and
  tests when relevant.
- **Decisions:** when the user makes a product or technical decision, record it in the RFD
  decision log (§14) and update the affected sections in the same change.
- **Stack:** Tauri 2 · Rust engine (`secopy-core`, UI-independent library) · Svelte + TypeScript UI.
- **Language:** code, comments, docs and commit messages in English.

## Git workflow

- Work on the branch you are on. Don't create, switch or rename branches unless the user
  asks.
- Remote: GitHub. Releases are cut from `main` (see Releases below).
- Commit and push only when the user asks.
- Never rewrite published history: no force-push, and no amending or rebasing commits that
  are already pushed.

## Commit messages: Conventional Commits 1.0

Format:

```
<type>(<scope>): <summary>

[optional body: what and why, wrapped at 72 chars]

[optional footers, e.g. BREAKING CHANGE: …, Refs: #12]
```

- **Summary:** imperative mood, lowercase, no trailing period, ≤ 72 characters.
- **One logical change per commit.** Don't mix a refactor with a feature.
- **Types:**

  | Type | Use for | Version bump |
  |---|---|---|
  | `feat` | user-visible feature | minor |
  | `fix` | bug fix | patch |
  | `perf` | performance improvement | patch |
  | `refactor` | code change with no behaviour change | none |
  | `docs` | documentation only (RFD, README, comments) | none |
  | `test` | tests only | none |
  | `build` | build system, dependencies, packaging | none |
  | `ci` | CI configuration | none |
  | `chore` | maintenance that fits nothing else | none |
  | `style` | formatting only | none |
  | `revert` | reverting a previous commit | depends |

- **Scopes** (optional, but use them when one applies): `core` (Rust engine), `app`
  (Tauri shell), `ui` (Svelte), `cli`, `rfd`, `ci`, `release`.
- **Breaking changes:** add `!` after the type/scope (`feat(core)!: …`) **and** a
  `BREAKING CHANGE:` footer explaining the migration.

Examples:

```
feat(core): compute source xxhash64 during copy
fix(ui): keep eta stable when throughput drops to zero
docs(rfd): record decision on symlink handling
perf(core): overlap verify of file n with copy of file n+1
```

## Versioning: SemVer 2.0

- Versions are `MAJOR.MINOR.PATCH`. Git tags are `vX.Y.Z` on `main`.
- Development starts at **0.1.0**. While on `0.x`:
  - a breaking change bumps **minor**
  - `feat` bumps **minor**
  - `fix` and `perf` bump **patch**
- **1.0.0** is released when the RFD v1 scope ships.
- Pre-releases use SemVer suffixes: `0.3.0-beta.1`.

## Releases: release-please

Releases are automated by [release-please](https://github.com/googleapis/release-please)
(`.github/workflows/release-please.yml`). That's why commit messages must follow the
format exactly.

- Each push to `main` updates an open **release PR**. That PR bumps the version,
  updates `CHANGELOG.md` and `version.txt`, and updates every file listed in `extra-files`.
- **Merging the release PR is the release:** it creates the `vX.Y.Z` tag and the GitHub release.
- **Never bump versions, edit `CHANGELOG.md`, or create release tags by hand.**
- Config: `release-please-config.json` (release-type `simple`, `bump-minor-pre-major`) and
  `.release-please-manifest.json` (last released version; `0.0.0` means nothing has been
  released yet, so the first `feat` releases `0.1.0`).
- Every manifest that carries a version must be listed under `extra-files` in
  `release-please-config.json` when the manifest is created, so all versions stay identical.
  Today that is only the root `Cargo.toml`; the UI and Tauri manifests join in plan 3:

  ```json
  "extra-files": [
    { "type": "toml", "path": "Cargo.toml", "jsonpath": "$.workspace.package.version" },
    { "type": "json", "path": "ui/package.json", "jsonpath": "$.version" },
    { "type": "json", "path": "src-tauri/tauri.conf.json", "jsonpath": "$.version" }
  ]
  ```

  Use the real paths when the files are created.
- To force a specific version (e.g. `1.0.0`), add a commit with the footer
  `Release-As: 1.0.0`.

## Development

- Layout: `crates/secopy-core` (engine library, no UI dependencies) and `crates/secopy-cli`
  (developer CLI and benchmark driver). The Tauri app and Svelte UI come in plan 3.
- Before every commit: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace`. All must pass. When a change touches Linux- or Windows-only
  `cfg` code, also run clippy with `--target x86_64-unknown-linux-gnu` and
  `--target x86_64-pc-windows-msvc` (add them once with `rustup target add`).
- `xxhsum` must be installed locally (`brew install xxhash`). CI runs its compatibility
  test with `SECOPY_REQUIRE_XXHSUM=1` so it can't silently skip.
- Plans live in `docs/superpowers/plans/`, designs in `docs/superpowers/specs/`, benchmark
  results in `docs/benchmarks/`.
