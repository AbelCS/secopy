# Secopy user guide

Secopy copies files and proves it did. This guide walks through every screen. For installing,
see the [README](../README.md#install).

The window has three tabs, **Copy**, **Mirror** and **Verify**, plus **Queue** and
**Settings** on the right.

Secopy is in English and Spanish. It starts in the Mac's language when it has it (English
otherwise); **Settings › General › Language** chooses another. Reports and the command-line
tool stay in English.

## Copy

![New copy](images/setup.png)

### From: the source

Drop a directory or files on **From**, or press **Choose…** (⌘O).

- **A directory** is copied with everything inside it, keeping its structure and file dates.
  By default only what's inside it is copied; **Include the "…" directory** copies the
  directory itself (the files land in `destination/CLIP/…`).
- **Files** picked one by one are copied side by side into the destination.
- **Hidden files are copied** (cameras hide some of their own). Inside a directory, names on
  Settings › Always ignore when copying are never copied: by default the files computers leave
  behind, like `.DS_Store`, `._*` or `Thumbs.db`. Their count is shown, so nothing disappears
  without a word. **Also ignore**, under the file types, adds names for this copy only (and
  its preset): for example `.gitkeep` for most copies but not for a Template backup. Files you
  pick one by one are copied even if a pattern matches them.
- **Symlinks** are not followed; they're counted as skipped, like special files (FIFOs,
  sockets, devices). Nothing is written through a symlinked directory in the destination
  either: a file whose path goes through one fails, saying so.
- **File types** lists every extension with its count and size, largest first. Click one to
  leave it out; **All** and **None** select every type or none.

Secopy copies a file's contents, its dates and its permissions. Extended attributes, Finder
tags, ACLs and resource forks are not copied.

### Copy presets

A preset is a saved source with its options: whether the directory itself is copied, which
file types, and its own **Also ignore** list. Choose one in **Preset** and its source is
loaded (or Secopy says the source isn't found). A change you make afterwards lasts for this
copy only, unless you press **Update** (save it into the preset) or **Save as…** (a new
preset). **Manage presets…** opens the list to create, edit, export or delete presets. When
typing file types there, `*` means every type. A preset never holds a destination: you choose
it for each copy.

### To: the destination

Drop a directory on **To**, or press **Choose…** (⌘D); **Recent…** lists the last ones.
Before you can start, Secopy checks and shows:

- **Where the files go**, the space available (as Finder counts it), and the drive's format.
- **What stops the copy:** the destination isn't there or can't be written to, there isn't
  enough space, or the destination is inside the source. Start stays off.
- **Needs purgeable space:** the copy fits only once macOS frees purgeable space (Time Machine
  local snapshots, caches). It frees it on demand, but not always in time; if it doesn't, the
  copy stops when the disk is full. To free it first, delete the local snapshots in Disk
  Utility.
- **Files that will fail:** a name the destination drive doesn't allow (FAT32 and exFAT
  refuse `: * ? " < > |`, for example), a file too large for FAT32's 4 GB limit, something
  in the way, two names that are one file on the drive, or a file that would land on a source
  file. You can start anyway; those files are listed as failed. Names are never changed.
- **Files already there.** Files with the same size and date are skipped and counted as
  "already at the destination, not checked". For files that **differ**, choose once:
  **Keep both** (the new copy is named `name (1).ext`), **Overwrite** (the old file is
  replaced only once the new one is complete, and verified in Copy & Verify) or **Skip**. The
  choice is for the files shown: another source or destination starts again with Keep both.
  With Overwrite, Start's status says how many files it replaces.
- **Leftovers:** unfinished files from an interrupted copy, replaced by this one.

When every file is already there, Start stays off and says "Nothing to copy."

### Copy or Copy & Verify

- **Copy & Verify** (recommended) reads every file back from the destination drive after
  writing it and compares its checksum with the source's. A copy that doesn't match is
  copied again once; if it still doesn't match, it fails and nothing is committed under its
  name (a file that was already there stays as it was).
- **Copy** doesn't read back, which is faster; the checksum file is still written from the
  source.

How Copy & Verify works: Secopy reads each file from the source once, computing its checksum
as it writes it, then reads the copy back from the destination drive while the next file
copies. The read-back bypasses the Mac's cache, so it reads the drive (the report says if a
drive doesn't allow it). The source is read again only for a file copied again after a
mismatch. On an SSD destination the read-back costs little extra time; on a spinning hard
drive, writing and reading at once is slower (the RFD, §7.5, explains why Secopy works this
way and what may change).

Press **Start** (⌘↩), or **Add to queue** to run it later. Start checks the destination
again first; if it changed since (files added or gone, another drive), it doesn't start and
shows it as it is now.

## While it copies

![Copying](images/progress.png)

The bars show what's been copied and verified, with the speed and the time left. **Active**
shows the files in progress (small files together), and the list below every file that's
finished, with its checksum and status; **Failed only** filters it.

- **Pause** (Space) stops reading and writing until **Resume**.
- **Cancel…** (⌘.) asks first. Files already copied stay, and the file in progress is removed.
  Tick **Also remove the files already copied** to remove the files this job created. A file
  it overwrote keeps its new version (only a mirror's archive can bring the old one back); the
  summary lists anything that couldn't be removed or restored.
- **Close the window** and the copy goes on, with its progress in the menu bar (see
  [Menu bar](#menu-bar)). Quitting asks first.

## Summary

![Summary](images/summary.png)

The headline says what happened, and never that everything was copied unless every file was.
Below it: the figures, every file that failed and why, and every file with its checksum and
status (Verified, Copied, Skipped, Failed, Cancelled).

- **Retry** sets up a new copy of just the failed files; press Start to run it.
- **Show in Finder** opens the destination; **Open checksum file** opens the proof;
  **Show ASC MHL** opens the history, when one was written.
- **Save report…** saves the report as text (and JSON next to it).
- **New copy** starts again, keeping the destination.

### The checksum file and the report

Each copy writes a checksum file in the destination directory, named
`secopy_2026-09-27_140302.xxh128`: one line per file it copied, with its XXH128 and its path.
Skipped and failed files aren't listed, and when nothing was copied none is written. It's a
standard format; in Terminal, `cd` to the destination and run `xxhsum -c secopy_…xxh128` to
check every file.

**ASC MHL** (Settings, off by default) is the media industry's proof of copy, read by tools
such as Silverstack and Hedge:

- Each copy writes an `ascmhl` directory in the directory the files go to, or adds to the one
  already there (or brought by the source), marking each file as matching its earlier checksum
  or not; a file that doesn't match makes the copy not complete. Files already in that
  directory that no history lists are read too, and before Start the plan says how many.
- Start is refused when the history can't be kept right: two different histories, Overwrite on
  a file it lists, a damaged history, or a file-type filter that leaves out files it lists.
- Mirrors don't write it, and Verify doesn't read it.

The report lists the settings, times, counts and the result of every file, including the
skipped ones. Every report is kept in
`~/Library/Application Support/com.latecommits.secopy/reports`; a setting also saves it next
to the checksum file.

## Mirror

A mirror keeps a backup identical to a directory, one way: the origin is never written to.

- **New mirror** asks for a **Name**, the **Origin** and the **Destination**, what happens to
  **files deleted in the origin** (**Archive them** for a number of days, 30 by default, or
  **Delete them**), and the **Comparison**: **Standard** (size and modification date) or
  **Paranoid** (compares the checksums of both copies; very slow: reads all data on both
  sides).
- **Preview…** works out what a run would do before anything is touched, and lists every
  change (**Show more** for the next page):

![Mirror preview](images/mirror-preview.png)

- **Start** runs exactly what the preview showed. New and changed files are copied and
  verified; a changed file is replaced only once its new copy is verified. Files deleted in
  the origin are archived into the destination's hidden `.secopy-archive` directory (or
  deleted) **only after every copy succeeded**: a failed or cancelled run archives or deletes
  none of them. Once archiving or deleting has begun, it finishes (Cancel is off meanwhile).
- **Days to keep:** at the start of every run, before copying, Secopy removes archived files
  older than the preset's days, counted from when they were archived. A change applies to
  everything already archived: shorter, and the next run removes the files now past them (the
  editor says so); longer, and what's still there is kept longer.
- **Switching from Archive to Delete** asks what to do with what's already archived: **Delete
  them now**, or **Keep them** for the preset's days (runs keep removing them when due). If the
  destination isn't found, or a job is running, **Delete them at the next run**: that run
  deletes the archive before copying anything; the Archive section says "Deleted at the next
  run", and switching back to Archive cancels it. Files that can't be deleted are listed, and
  go when they're due. Changing the destination at the same time leaves the old destination's
  archive as it is.
- If a run looks wrong (the origin is empty, the origin or the destination can't be fully
  read, or more than half of the backup would be removed), Secopy asks first; in the queue,
  such a run doesn't start. A missing origin stops the run.
- Symlinks in the origin aren't followed, and what's under one is never removed from the
  backup: a directory moved to another disk and left as a symlink keeps its backup while that
  disk is away.
- The destination keeps a checksum file of its own, `.secopy-checksums.xxh128`, so the backup
  can be verified. Every run records the files it verified, even one that didn't end cleanly.
- **Archive** (under the editor): files, size and oldest run in the destination's
  `.secopy-archive`. **Show in Finder** opens it; **Delete archive…** deletes it, after asking.
  Not while a job runs.

Every question before something that can't be undone has the safe answer first: Return and
Esc never delete.

## Verify

Verify reads a copy again and compares every file with its checksum files, to find silent
damage (a failing drive, a file changed by something else).

**Choose…** a directory (a copy, a backup, or a whole drive): Secopy finds every `.xxh128`
checksum file inside it and shows how many files they list. **Start** reads each one again
from the drive and marks it **Intact**, **Changed**, **Missing** or unreadable. When several
checksum files list a file, the newest one wins. Files no checksum file lists are counted as
not checked: nothing says what they should contain. Nothing is written to the drive and
nothing is repaired.

Copies made by Secopy before 0.24 have `.xxh64` checksum files, which Verify doesn't read;
`xxhsum -c` checks them.

![Verify summary](images/verify-summary.png)

## Queue

![Queue](images/queue.png)

**Add to queue** on Copy, Mirror or Verify adds the job. A copy keeps its source, destination
and options as set up; a mirror runs its preset as it is at its turn; Settings (the ignore
list, ASC MHL) are read when each job starts. On the Queue screen, reorder jobs with the
arrows, remove them, **Clear…** them all (after asking), and choose what happens **if a job
fails**: continue with the next job or stop the queue. **Start** runs them one after another;
each is checked when its turn comes, and one that can't start fails with its reason. A copy
set to Overwrite replaces only the files it listed when it was queued; if others differ at its
turn (another card with the same names), it doesn't start. The Mac stays awake for the whole
run, and one notification says how it went.

At the end, every job has its result and its own summary. Jobs that completed leave the
queue; the others stay, with the reason they failed. The queue is kept when Secopy quits.

## Menu bar

Close the window while a job or the queue runs and it keeps going: Secopy leaves the Dock
and its icon in the menu bar shows the progress (`42%`, `2/3 · 42%`, `Paused`, then ✓ or ✗).
Click it for the details, **Pause**, **Open Secopy** and **Quit Secopy…** (it asks first
while a job runs):

![The menu bar panel](images/panel.png)

Turn this off in Settings to have closing the window ask to cancel the job instead.

## Export and import

**File › Export…** (or Settings) saves the settings, the copy presets and the mirror presets,
as you choose, in a `.secopy` file: for a new Mac, or to share presets. A single preset can
be exported from its own screen. The queue and recent destinations are never exported.

**File › Import…** (or opening a `.secopy` file from Finder) shows what the file holds before
anything changes: settings that differ, each preset with its paths, paths not on this Mac,
and presets that can't be imported, with why. A preset whose name you already have is added
as **Keep both** ("Name (2)") unless you choose **Replace yours**; replacing a mirror with
fewer archive days says what its next run removes. Nothing is imported while a job runs.

A file from another version of Secopy says which one made it. Settings this Secopy doesn't
know are left out, and settings the file lacks are set to their defaults; both are listed. A
preset with a setting this Secopy doesn't know can't be imported: update Secopy first.

![Import](images/import.png)

## Settings

**General**

- **Language**: **Automatic** (the Mac's language when Secopy has it, English otherwise) or a
  language of your choice, whatever the Mac's is. Secopy's words change when you press
  **Save**; macOS's own windows (Open, Save, About) at the next launch.
- **Notify when a job finishes**, when Secopy's window isn't in front (on by default; macOS
  asks for permission the first time).
- **Keep jobs running in the menu bar when the window is closed** (on by default).

**Copies**

- **Write the checksum file to the destination** (on by default).
- **Write ASC MHL** (off by default): see
  [The checksum file and the report](#the-checksum-file-and-the-report).
- **Show the count of ignored files** (on by default).
- **Save the report next to the checksum file** (off by default).
- **Always ignore when copying**: names never copied or mirrored, like `*.LRF` or `.gitkeep`
  (`*` is any characters, `?` one; case doesn't matter). A matching directory is left out with
  everything in it (and counted as one), and a mirror never removes these files from its
  destination. It starts with the files computers leave behind; **Restore defaults** puts those
  back. Each list holds up to 128 patterns of up to 255 characters, names only (no `/`). Copy
  and mirror presets have their own **Also ignore** list on top of it. In Terminal,
  `--ignore PATTERN` adds a pattern.

Changes apply to the next job when you press **Save**; **Cancel** (or Esc) drops them.

## Keyboard

| Keys | Does |
|---|---|
| ⌘O / ⌘D | Choose the source / the destination |
| ⌘↩ | Start |
| ⌘. | Cancel the running job (asks first) |
| Space | Pause and resume |
| ⌘1 / ⌘2 / ⌘3 | Copy / Mirror / Verify |
| ⌘4 | Queue |
| ⌘, | Settings |
| Esc | Back from Settings, presets and Import; closes dialogs |

## When something goes wrong

- **A file failed.** The summary and the report say why for each file. Fix the cause (a
  name, a permission, a drive) and press **Retry**. Files a cancelled or stopped copy never
  reached aren't failures: copy the source again (files already there are skipped).
- **The drive was pulled out or filled up.** The job stops when it notices and says so. Files
  already copied are complete and listed; Secopy removes the file in progress if it still can.
- **A `.secopy-partial` file at the destination** (or `.secopy-….partial`): a copy cut short
  by a crash or a power cut. It never has a file's final name; the next copy of the same files
  replaces it (Leftovers), or delete it.
- **"… in the origin couldn't be read"** on a mirror: nothing is removed from the backup that
  run, so an unreadable file never looks deleted.
- **A copy looks damaged months later.** Run **Verify** on it: it names every file that
  changed or is missing.
