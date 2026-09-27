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
- **Conventions:** [Semantic Versioning 2.0](#versioning-semver-20),
  [Conventional Commits 1.0](#commit-messages-conventional-commits-10),
  [release-please](#releases-release-please) for releases, and a
  [GitHub issue for every task](#github-issue-driven-workflow): feature, bug fix, docs,
  refactor or chore.

## Git workflow

- Every change is tracked by a GitHub issue and lands on `main` through a pull request
  (see [GitHub Issue-Driven Workflow](#github-issue-driven-workflow)).
- Branches: if you are already on a branch other than `main` (Conductor workspaces come
  with one), work there. Otherwise create `<type>/<issue-number>-<short-slug>` from `main`,
  e.g. `fix/12-long-windows-paths`. Don't rename existing branches.
- Remote: GitHub (`AbelCS/secopy`). Releases are cut from `main` (see Releases below).
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
  It's the user's call.
- **Merge PRs with a rebase merge** (`gh pr merge --rebase`), never squash: a squash turns
  all of a PR's commits into one message, and release-please loses the individual
  `feat`/`fix` entries. Before merging feature work, check `gh pr list` for an open
  `chore(main): release …` PR and ask whether it should be merged first, because anything
  merged before it ships in that release.
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
- Engine flow: `scan → select → preflight → Plan::resolve → run_job`, then `Report` for
  the job report. Design notes per plan are in `docs/superpowers/specs/`.
- Before every commit: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace`. All must pass. When a change touches Linux- or Windows-only
  `cfg` code, also run clippy with `--target x86_64-unknown-linux-gnu` and
  `--target x86_64-pc-windows-msvc` (add them once with `rustup target add`).
- `xxhsum` must be installed locally (`brew install xxhash`). CI runs its compatibility
  test with `SECOPY_REQUIRE_XXHSUM=1` so it can't silently skip.
- Fault tests on real volumes (disk full, unplugging, FAT32/exFAT, case-sensitive APFS) use
  macOS RAM disks and run only with `SECOPY_DEVICE_TESTS=1`; CI's macOS job sets it. Run
  them locally before changing `copy`, `os`, `preflight` or the job runner.
- Plans live in `docs/superpowers/plans/`, designs in `docs/superpowers/specs/`, benchmark
  results in `docs/benchmarks/`.

## GitHub Issue-Driven Workflow

Every task gets a GitHub issue, and the issue is closed when the task is done.

### 0. Make sure there is an issue
- If the user asks for work that has no issue yet, create one before starting:
  `gh issue create --title "<type>(<scope>): <summary>" --body "<what and why, acceptance criteria>"`.
  The title uses the Conventional Commits format of the change it will produce. Tell the
  user the issue number.
- Work found along the way that doesn't belong to the current issue (a separate bug, a
  follow-up) gets its own issue instead of being folded in silently.

### 1. Understand the issue
- Fetch it with `gh issue view <number> --comments` (or by URL).
- Read the title, body, labels, and all comments. Follow links to related issues, PRs, or design docs.
- Do not start work until the requirements are clear. If anything is ambiguous, ask the user before proceeding.

### 2. Mark the issue as in progress
- Assign yourself: `gh issue edit <number> --add-assignee @me`.
- If the repo uses a project board, move the issue to the "In Progress" column.
- Optionally post a brief comment on the issue indicating that work has started.

### 3. Create a branch
- Follow the branch rule in [Git workflow](#git-workflow): reuse the branch you are on if
  it isn't `main`, otherwise branch from `main`.
- New branches are named `<type>/<issue-number>-<short-slug>`, e.g. `feat/123-add-login`, `fix/456-null-pointer`.
- The `<type>` should match the conventional commit type the eventual PR will use (`feat`, `fix`, `docs`, `refactor`, etc.).

### 4. Implement the change
- Follow the project's other workflow rules (conventional commits, TDD, commit discipline, etc.).
- Make incremental commits — do not let unrelated changes accumulate.
- Reference the issue in commit footers where it helps: `Refs: #123`.

### 5. Push and open a PR
- When the user asks to push: `git push -u origin <branch>`.
- Open the PR with `gh pr create`. The PR description MUST include a closing keyword that links the issue, e.g. `Closes #123`, so the issue auto-closes on merge.
- Use a PR title that follows conventional commit format and clearly summarizes the change.

### 6. Update issue status after PR opens
- Verify the issue is linked from the PR (the closing keyword does this automatically).
- If the repo uses a project board, move the issue to "In Review" (or the equivalent column).
- Report the PR URL back to the user.

### 7. Close the issue when the work is complete
- A merged PR with `Closes #123` closes the issue; confirm it with `gh issue view 123`.
- If it is still open (no closing keyword, or the work landed another way), close it:
  `gh issue close 123 --comment "Done in #<pr> / <commit>"`.
- Work that is dropped is closed as not planned, with the reason:
  `gh issue close 123 --reason "not planned" --comment "<why>"`.
