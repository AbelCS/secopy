# Mirror — Design

On top of the job queue ([design](2026-09-28-job-queue-design.md), #50, 0.7.0). Requirements
live in [RFD 0001](../../rfd/0001-secopy.md) (§5.8, FR-44..FR-52); decisions made here are also
logged in RFD §14. Issue: #51. Target release: 0.8.0. Inspiration, not a model: the Mirror app
(titleunknown/Mirror-Releases) and its write-up on fainimade.blog.

## Goal

Keep a backup identical to its original with one click (or overnight in the queue): what's new
is copied, what changed is updated, what was deleted is removed from the backup — and nothing
is ever lost by mistake.

**Success looks like:** a "Footage → NAS" preset; Preview says "+ 12 new, ↻ 3 changed, − 5
deleted (archived), = 2,410 unchanged"; Run mirror; the NAS matches the SSD, every written file
verified, the 5 removed files in the archive for 30 days.

## Decisions

| # | Decision |
|---|---|
| M1 | **One-way only.** The origin is never written to. |
| M2 | **Presets, run by hand or from the queue.** No schedule in this version. |
| M3 | **Changed = size or modification date differs** (dates within 2 s count as equal: exFAT and some NAS round them). An optional **deep check** per preset also compares contents by xxHash64. |
| M4 | **Everything Mirror writes is verified**, as in Copy & Verify. |
| M5 | **Deleted in the origin → archived (kept N days, default 30) or deleted**, per preset. The archive is a hidden directory on the destination, so it works on SMB and moving into it is instant. |
| M6 | **Deletions last, and only after a clean copy phase:** if any file failed or the run was cancelled, nothing is archived or deleted. |
| M7 | **A guard:** a missing or empty origin, or a run that would remove more than half of the destination's files, needs confirmation by hand and fails in the queue. |
| M8 | **A manual run always previews first.** Queue runs don't (nobody is there); the guard protects them. |
| M9 | **Left out of this version:** scheduling, detecting moved/renamed files, several destinations, exclusions beyond system files, restoring from the archive inside the app (Finder does it). |

## 1. Presets (the Mirror section)

The sidebar's **Mirror** shows the presets on the left and the selected one on the right, like
Profiles:

| Field | Notes |
|---|---|
| Name | Unique, like profiles. |
| Origin | Full path, Choose…. |
| Destination | Full path, Choose…. Not the origin, not inside it, and the origin not inside it. |
| Deleted files | **Archive, keep for N days** (default 30) or **Delete**. |
| Deep check | Off by default: "Also compare every file's contents (reads both sides completely; slow on big libraries)." |

Actions: **Preview…** (primary), **Add to queue**, **Delete…**; **+ New preset**. Edits are
saved with Save / Revert as in Profiles.

## 2. Preview

Comparing both sides (a fast scan of each; with the deep check, both are read) shows what the
run would do, before anything is touched:

```
Footage → NAS                                                    Preview
/Volumes/SSD/Footage  →  /Volumes/Media/Footage
 + 12 new          38.2 GB
 ↻ 3 changed        4.1 GB
 − 5 deleted in the origin   → archived, kept 30 days
 = 2,410 unchanged
[All | New | Changed | Deleted]
 C0101.MP4                                  2.1 GB   New
 C0044.MP4                                  1.8 GB   Changed: newer in the origin
 old/C0003.MP4                              1.2 GB   Deleted in the origin
[Cancel]                                        [Add to queue] [Run mirror]
```

- Counts and sizes per kind; the file list filtered by kind (virtualized, like the finished list).
- "Already in sync." when there's nothing to do (Run mirror disabled).
- The guard (M7), when it trips, shows its reason above the list, and Run mirror asks to confirm.
- Run mirror runs **this** plan (see Safety for what changed since).

## 3. Running and summary

- The Copying screen as today, with the phase "Mirroring" and a third line for deletions
  ("Archiving 5 files" / "Deleting 5 files") after the copies.
- The Summary's headline: "Mirrored: 12 new, 3 updated, 5 archived", or "3 files failed —
  nothing was removed". Actions: Show in Finder (destination), Save report…, and **Done**
  (back to the preset). No checksum file is written by Mirror (it would be part of the mirror).
- In the queue, a mirror run is a job like a copy ("Mirror · Footage → NAS"); it plans at its
  turn, and a tripped guard fails it with the reason.

## 4. Safety

- **Order:** new and changed files are copied and verified; then, only if nothing failed and
  the run wasn't cancelled, deleted files are archived or deleted; then emptied directories
  that no longer exist in the origin are removed.
- **Replacing a changed file:** the new copy is written to a partial file and verified; then
  the old version is moved to the archive (Archive mode) or removed; then the partial is
  renamed into place. The destination never holds a half-written file under its real name.
- **Between preview and run:** a file planned for copying that vanished fails that file (so no
  deletions); a file planned for removal that reappeared in the origin is left alone.
- **The archive:** `<destination>/.secopy-archive/<YYYY-MM-DD HH.MM.SS>/<original path>`, one
  directory per run. It is never compared or mirrored. At the start of each run, run
  directories older than N days are removed. In Delete mode the archive is left as it is.
- **Ignored on both sides:** system files (`.DS_Store`, `._*`… as in Copy), symlinks (counted
  and shown), and the archive. A `.DS_Store` Finder puts in the destination is never "deleted
  in the origin".
- **Names:** compared after Unicode normalization (NFC), so SMB shares that return names in
  another form don't make every file look new and deleted; on a case-insensitive destination,
  a name that only changed case is updated, not archived and copied.

## Architecture

| Unit | Purpose |
|---|---|
| `secopy-core` `mirror` | `plan(origin, destination, options) -> MirrorPlan` (new, changed with the reason, deleted, unchanged, directories to create/remove, guard result) from two scans; the deep check. `run(plan, …)`: the copy phase on the existing pipeline (partial, verify, archive-then-rename for changed), the deletion phase, archive clean-up; progress events like a job. |
| `secopy-cli` | `secopy-cli mirror <origin> --to <destination> [--delete \| --archive-days N] [--deep] [--dry-run]`, to try it on real drives. |
| `secopy-app` `store` | Presets in `mirrors.json` (version 1), validated like profiles. |
| `secopy-app` `queue` | A second job kind: `{ "kind": "mirror", "preset": "<id>" }`, planned at its turn. |
| commands | `mirror_presets`, `create_/edit_/delete_mirror_preset`, `preview_mirror(id)`, `mirror_preview_page(filter, offset, limit)`, `run_mirror` (progress on a channel), `add_mirror_to_queue(id)`. |
| UI | The Mirror section (presets, editor), the Preview screen, the Mirroring progress line, the mirror Summary. |

## Errors

- Origin or destination missing: Preview says which ("NAS isn't connected."), nothing runs.
- A preset's origin and destination overlapping: refused when saved.
- A file failing to copy: reported like Copy; no deletions (M6).
- A file that can't be archived or deleted (in use, permissions): reported, the run ends as
  failed, the rest of the deletions continue.

## Testing

- **Engine (Rust):** planning — new, changed by size, changed by date beyond 2 s, equal within
  2 s, deleted, unchanged, directories, system files and the archive ignored, NFC names, the
  case-only rename; the deep check finds same-size-and-date different contents; the guard
  (empty origin, more than half); the run — changed files replaced atomically with the old one
  archived, deletions only after a clean copy phase, a failure or cancel removes nothing,
  archive clean-up by age, Delete mode; a planned file vanished / a deleted one reappeared.
- **App (Rust):** presets round trip and validation (overlap); a mirror job in the queue.
- **UI (Vitest):** the Mirror section and editor; Preview counts, filters, "Already in sync",
  the guard's confirmation; the mirror Summary's headline and "nothing was removed".
- **Manual:** SSD → SMB share with new, changed and deleted files; a second run is "Already in
  sync"; restore a file from the archive in Finder; pull the destination mid-run.

## Review focus

1. **Nothing is lost:** no deletion after any failure or cancel; a changed file's old version
   is never removed before its new copy is verified.
2. **The destination's own files** (`.DS_Store`, the archive, files the NAS adds) never count
   as deleted in the origin, and the archive never mirrors itself.
3. **Names on SMB** (NFC/NFD) and case-insensitive destinations never cause copy-and-delete loops.
4. **The guard** can't be bypassed in the queue, and a missing origin never looks like "everything deleted".
5. **Preview then run** with changes in between: runs what was previewed, safely.
