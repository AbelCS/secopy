# ASC MHL — design

Issue #154. Approved in conversation on 2026-09-30 and 2026-10-01.

## Goal

Hand media on with the industry's proof of copy. When the user turns it on, each copy writes
an ASC MHL history in the folder the files go to, which other tools (Silverstack, Hedge,
ShotPut, YoYotta) and post houses read. A history the media already has is carried on, so the
chain of custody doesn't stop at Secopy.

Success: Secopy's output passes the ASC's reference tool (`ascmhl-debug verify`, the XSD
check), and that tool can append to it; a history made by another tool is continued with every
file verified or failed; nothing about a copy changes when the setting is off.

## Decisions (from the conversation)

| # | Decision |
|---|---|
| D1 | Write only, for hand-off. Verify doesn't read MHL (a later step, if wanted). |
| D2 | Optional: Settings › **Write ASC MHL**, off by default, independent of "Write the checksum file". CLI: `--mhl`. |
| D3 | Next to the `.xxh64` checksum file, never instead of it. |
| D4 | An existing history (in the source, or already in the destination) is continued, as the standard asks: a new generation, each file `verified` or `failed` against its earlier hash. |
| D5 | Files in the destination that no history records yet are read and recorded as `original` (guidelines §2.7). How many and how big is shown before Start. |
| D6 | Copies only. Mirrors delete and archive files, and a history assumes nothing it lists goes away. |
| D7 | No directory hashes for now (optional in the specification). |

## The standard

ASC MHL specification v1.0 (March 2022) and Implementation Guidelines v1.0 (March 2023),
<https://github.com/ascmitc/mhl-specification>; the XML format is version `2.0`. Reference
implementation and XSDs: <https://github.com/ascmitc/mhl>.

- **History:** an `ascmhl/` folder at the root of the folder it covers (its *scope*): one
  manifest per generation and a chain file. Every file in the scope is recorded by the history
  (or a nested one) or matches an ignore pattern.
- **Manifest name:** `NNNN_<folder name>_<YYYY-MM-DD>_<HHMMSS>Z.mhl`, the number zero-padded to
  4 digits (more past 9999), the date in UTC at the start of the job, the folder name the
  scope's own.
- **Manifest** (`urn:ASC:MHL:v2.0`, `version="2.0"`), in this order:
  - `creatorinfo`: `creationdate` (local time with offset, seconds), `hostname`,
    `<tool version="0.19.0">Secopy</tool>`.
  - `processinfo`: `<process>transfer</process>` and `ignore` with every pattern of the
    previous generation plus Secopy's (the list only grows).
  - `hashes`: one `hash` per file: `<path size=".." lastmodificationdate="..">rel/path</path>`
    and `<xxh64 action="original|verified|failed" hashdate="..">16 lowercase hex</xxh64>`.
    Paths use `/`, relative to the scope, case and spaces kept, no duplicates.
  - `references` (only with nested histories, see below).
- **Chain file:** `ascmhl/ascmhl_chain.xml` (`urn:ASC:MHL:DIRECTORY:v2.0`), one
  `<hashlist sequencenr="N"><path>…</path><c4>…</c4></hashlist>` per generation, numbered from
  1 without gaps.
- **C4 ID** of a manifest: SHA-512 of its bytes, as a big-endian number in base58 (alphabet
  `123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz`), left-padded with `1` to 88
  characters, prefixed `c4` (90 in all).
- **Actions:** `original` is the first hash recorded for a file; `verified` matches the first
  `original` hash of the same format in the history; `failed` doesn't.
- **xxh64:** seed 0, canonical big-endian, 16 lowercase hex characters: the value Secopy's
  checksum file already holds.

## What the user sees

**Settings › Every copy:** "Write ASC MHL" (Hint: "The media industry's proof of copy, read by
other tools. Kept in an ascmhl folder."). Off by default.

**Before Start** (only with the setting on), one line in the plan:

- "ASC MHL: new history" or "ASC MHL: continues the history (generation 3)".
- When files already in the destination need reading: "Also records 212 files already here
  (48 GB)." This includes files skipped as identical that no history lists yet.
- A blocker instead, when one applies (below).

**Copying:** after the files, a "Recording ASC MHL" step reads the files that need it, with
progress, Pause and Cancel like the copy.

**Summary:** "ASC MHL written" with Show in Finder (the `ascmhl` folder). Files that don't match
their earlier hash are listed, and the job isn't Complete.

## What gets recorded

The scope is the folder the files go to (the copy root: `DEST/<name>` with Include the
directory, `DEST` without).

- **Copied files:** their xxh64 from the copy (Copy & Verify: the verified value; Copy: the
  value read from the source), `original`; `verified`/`failed` when an earlier generation lists
  the path.
- **Files already in the scope** that no generation lists (skipped as identical, or simply
  there): read and recorded as `original`.
- **Files an earlier generation lists** and this copy didn't write: not read, not listed again
  (guidelines §2.1.3).
- **Failed files:** not listed (as in the checksum file). A listed file that failed is missing
  from the destination, which the history then shows.
- **Ignored, never listed:** the standard's defaults (`.DS_Store`, `ascmhl`, `ascmhl/`), the
  macOS system files Secopy skips (`._*`, `.Spotlight-V100`, `.fseventsd`, `.Trashes`,
  `.TemporaryItems`, `.DocumentRevisions-V100`, …, from `system.rs`), and Secopy's own files
  (`secopy_*.xxh64`, `.secopy-checksums.xxh64`, the job report's name pattern).

## Existing histories

- **The source has one** (an `ascmhl` folder at the copied folder's root): with the setting on,
  Secopy copies its manifests itself (not as ordinary files), checks them against the chain,
  and adds the next generation in the destination. Each copied file is `verified` or `failed`
  against the source history's first `original` hash for that path; files it doesn't list are
  `original`.
- **The destination already has one** (a second copy, or a retry): the next generation lists
  the files this copy brought.
- **Nested histories** (a shooting-day folder whose card folders each have `ascmhl`): each
  nested history gets its own new generation for the files in its folder. The scope's root gets
  a generation (a new history if it has none) with the files outside them and, in `references`,
  a `hashlistreference` (path and C4) for each nested generation just written. Nested ones are
  written first (guidelines §2.1.4; reference example `scenario_05`).
- **With the setting off:** an `ascmhl` folder in the source is copied like any other folder,
  as today.

## Blockers (Start is refused, with why)

Only with the setting on. Each says how to go on: choose another folder, change the conflict
choice, or turn off ASC MHL.

1. **Two histories:** source and destination each have a history for the same folder, and they
   differ. "The source and the destination have different ASC MHL histories."
2. **Overwriting a recorded file:** a conflict set to Overwrite on a file a destination history
   lists. "Overwriting files its ASC MHL history lists would break it."
3. **A damaged history** in the source or the destination: a manifest missing or not matching
   its C4, a chain with gaps, XML that can't be read, an `ascmhl` folder without a chain file.
   "The ASC MHL history in … is damaged." Nothing is repaired or written over.
4. **Leaving out recorded files:** the source has a history and the copy leaves out files it
   lists (a file-type filter, or files Secopy skips as system files).
   "This copy leaves out files the source's ASC MHL history lists." (Files picked one by one
   don't carry their folder's history: the destination gets a new one.)

Each is checked when the plan is made and again at Start (#112's re-check), and again in the
job right before writing, so a history changed in between is never written over.

## Writing safely

- Run at the end of the job, after the files are durable, in this order: nested histories
  first, then the root.
- **Manifest:** created only if nothing is there (`create_new`, never through a link), written,
  synced.
- **Chain:** written to a temporary name next to it, synced, renamed into place, the folder
  synced. If anything fails, the new manifest is removed, so the history is as it was.
- A failure to write is a job failure ("ASC MHL couldn't be written: …"), not a silent skip.
- Cancel keeps the rules of the checksum file: what was copied and kept is recorded; with
  "Also remove the files already copied", no MHL is written.

## How it's built

- **secopy-core `mhl` module:** C4; manifest and chain writing (XML escaped by hand: few
  elements); reading a history (`quick-xml`): chain and C4 check, recorded paths with their
  first `original` hash, latest ignore patterns, nested histories; ignore matching (the
  .gitignore subset the patterns use).
- **Plan:** with the setting on, the plan reads the histories, finds the files to read, and
  sets the blockers; `Plan` carries what the job needs.
- **Job:** the "Recording ASC MHL" step (read with the existing hashing, progress, Pause,
  Cancel), then writing.
- **App:** the setting (`writeMhl`, exported and imported like the others), the plan line and
  blockers, the summary line and Show in Finder, Verify not counting `ascmhl` files as "not
  listed", the queue re-checking at a job's turn.
- **CLI:** `--mhl`.
- **New crates:** `quick-xml`, `sha2`, `bs58` (latest stable).

## Tests

- C4 of the standard's example manifest equals its chain file's value.
- Manifests and chains Secopy writes validate against the official XSDs (`xmllint --schema`,
  the XSDs kept under `crates/secopy-core/tests/ascmhl/`).
- One test per case above: new history, continued source history (verified and failed),
  destination history, nested histories, files already there, ignored files, each blocker, a
  failed write leaving the history as it was, cancel.
- End to end, run by hand before the PR: the reference tool's `ascmhl-debug verify` passes on
  Secopy's output, and `ascmhl create` appends a generation to it.
- A hand test with real footage, and one with a history made by another tool.

## Not now

Reading MHL in Verify; directory and root hashes; flattening and collections; mirrors;
other hash formats.

## Changes while building (2026-10-01)

- The source's `ascmhl` folder is copied by the normal copy (verified, durable, undoable) and
  then appended to. When the destination already has that history (the same, or longer), the
  source's history files aren't copied over it.
- A cancelled copy (or one that stopped as a whole) writes no ASC MHL, instead of following
  the checksum file's rules: its history would be as incomplete as the copy. Running the copy
  again finishes both.
- A path an existing history recorded only as md5, sha1 or c4 gets Secopy's xxh64 as
  `original`: Secopy can't compute those, so that file isn't verified against its earlier hash.
- `hashdate` is left out (it then means the manifest's `creationdate`).
- The ASC's example chain in its `xsd/examples` folder names a stale C4; the tests use the
  reference tool's generated `examples/scenarios/Output/scenario_01` instead.

