# Wording review — 2026-10-03

Issue #172. Every string in `ui/src/locales/en.json` (910) reviewed twice, independently (a
Claude agent and GPT-6.1 Sol), against the house rules: precise terms and few words (detail
only in help), one term per thing, consistent form, true to what the app does, translatable.
Merged and checked against the code by Claude. Nothing is changed until approved.

## A. Wrong today (fix)

| key | now | proposed |
|---|---|---|
| copy.ignoredHint | Left out by Settings › Always ignore when copying: {patterns} | Ignored by name: {patterns} |
| settings.systemCount.help | Shows how many files and directories Always ignore when copying left out. Every other file is copied, hidden or not. | How many files and directories “Always ignore when copying” and “Also ignore” left out. Every other file is copied, hidden or not. |
| presets.empty.what | A copy preset saves a source and its settings (whether that directory itself is copied, and which file types), so a copy you do often is set up in one step: choose the preset in the main window. | A copy preset saves a source and its options (the directory itself or only what's in it, file types, Also ignore): choose it in New copy to set up a frequent copy in one step. |
| presets.empty.how | Create one with “+ New preset”, or with “Save as…” in the main window. | Create one with “+ New preset…”, or with “Save as…” in New copy. |
| verify.nothing | No checksum files here: there's nothing to verify. Secopy writes one with every copy and every mirror. | No checksum files here: nothing to verify. Copies write one (unless it's off in Settings); mirrors always do. |
| mirror.empty.what | …Nothing is ever written to the origin, and every run shows a preview first. | A mirror keeps a destination identical to its origin: new and changed files are copied and verified; files deleted in the origin are archived (or deleted) in the destination. Nothing is written to the origin. |
| mirror.editor.paranoidHelp | Byte-for-byte comparison of both copies. | Compares the checksums of both copies. |
| errors.file.nameClash | Another file in this copy has the same name (names are compared ignoring case) | Another file in this copy has the same name (on this destination, names that differ only in case are the same) |
| summary.headline.undoneRemoved | Cancelled: the copied files were removed | Cancelled: not everything could be put back |
| progress.stop.alsoRemoveHelp | The destination goes back to how it was. Files this job replaced come back only from a mirror's archive. | Removes what this job copied. Files it replaced come back only from a mirror's archive. |
| settings.notify.label / import.setting.notify | Notify when a copy finishes | Notify when a job finishes |
| settings.menuBar.label / import.setting.menuBar | Keep copying in the menu bar (when the window is closed) | Keep jobs running in the menu bar when the window is closed |
| errors.job.alreadyRunning / errors.queue.busy / errors.report.running | A copy is already running. / A copy or the queue… / The copy is still running. | A job is already running. / A job or the queue is already running. / The job is still running. |
| menu.file.cancel | Cancel Copy | Cancel Job… |
| summary.notRestored.one | …couldn't be brought back: their new versions stay. | {count} file this job replaced couldn't be restored: its new version stays. |
| summary.headline.allVerified.one / allCopied.one / check.intact.one | All {count} file … | {count} file copied and verified / {count} file copied / {count} file intact |
| copy.queued.one/other | Added to the queue ({count} job). | Added to the queue (now {count} job). |
| verify.startHelp.one | …and compares each with its checksum… | Reads the {count} listed file ({size}) and compares it with its checksum; nothing is written. |
| copy.preflight.identical (+ notEmpty, identicalHint) | Identical / "Identical files will be skipped." | Same size and date / "Files with the same size and date are skipped." (contents aren't compared) |

## B. One term per thing

| term | not | where it changes |
|---|---|---|
| deleted in the origin | gone from / removed from the origin | mirror.preview.startHelp.*, progress.quit.finishing.*, import.replaceDeletes |
| verify / verifying | check (as the action) | progress.stop.left.check ("Nothing was changed: verifying only reads files."), summary.headline.checkCannotContinue ("verifying couldn't continue") |
| choose / chosen | pick / picked | copy.status.noSource ("Choose a source."), copy.pick.nothing ("Nothing was chosen."), copy.pick.mixed ("Choose one directory or only files, not both."), presets.editor.includePicked ("Include the chosen directory itself"), errors.preset.needsDirectory |
| checksum | hash | errors.file.hashMismatch ("Checksum mismatch (source {expected}, copy {actual})") |
| symlink | link | errors.file.isLink ("Is a symlink, not checked (symlinks aren't followed)"), errors.scan.loop |
| available | free | copy.preflight.purgeable ("…{free} available now") |
| options (a copy's) | settings, choices | presets.updateHelp / saveAsHelp ("…this source and these options…") |
| directory | folder | settings.mhl.help |
| destination | backup | settings.ignore.help ("…never removes them from its destination.") |
| New copy | main window | presets.empty.* |
| queue (lowercase) | Queue mid-sentence | copy/verify/mirror.addToQueueHelp |
| days to keep (mirror) | the preset's days | mirror.archiveNotCleaned.* |
| example paths | CARD_A | presets.editor.sourcePlaceholder, errors.field.source.notFull ("/Volumes/Untitled/…") |

## C. Shorter, more precise

| key | proposed |
|---|---|
| copy.addToQueueHelp | Adds this copy, as set up now, to the queue. |
| verify.addToQueueHelp | Adds this directory to the queue. |
| mirror.addToQueueHelp | Adds this mirror to the queue; what to copy and remove is worked out again when it runs. |
| copy.aboutVerifyingText | Copy & Verify reads each file back from the destination and compares it with the source's checksum; a mismatch is copied again once. Copy is faster but checks nothing. |
| copy.preflight.identicalHint | Same name, size and modification date (within 2 s) as a file in the destination: not copied or read. |
| copy.preflight.existingHint | Same name as a file in the destination, different size or date. Keep both: copied with a number added. Overwrite: replaces it. Skip: leaves it. |
| progress.smallFiles.help | Files under 8 MB, copied several at once and counted together. |
| progress.lookingAtSource | Checking the source and the destination… |
| progress.quit.finishing.undo | Secopy first finishes putting the destination back, then quits. |
| summary.stats.notCheckedHint | In no checksum file: nothing to compare them with. |
| summary.stats.notStartedHint | Not reached: the job was cancelled or stopped. |
| settings.mhl.help | ASC Media Hash List, the media industry's proof of copy (Silverstack, Hedge). Written to an ascmhl directory with the files; an existing history is continued and lists files already there too. |
| settings.report.label (+ import.setting.report) | Save the report next to the checksum file |
| import.setting.checksumFile | Write the checksum file to the destination (same as Settings) |
| verify.about | A copy's directory or a whole drive: every file its checksum files list is read and compared with its checksum. Nothing is written. |
| queue.empty | Nothing queued. Set up a copy, mirror or verify and press “Add to queue”. |
| queue.name.mirror | Mirror · {name} (as queue.mode.mirror) |
| errors.mhl.overwrites | Overwriting files the destination's ASC MHL history lists would break it. … |
| errors.mirror.archiveNotDir | …Move it away, or choose “Delete them” for deleted files. |
| mirror.switch.deleteNextRun | Delete them at the next run (as “Delete them now”) |

## D. Form (mechanical, everywhere)

- **Contractions**: "can't / couldn't / isn't" throughout (the UI mostly does; errors.file.*,
  summary.checksumError, reportError, copyCannotContinue don't yet), with articles where
  missing ("Can't read the source: {why}", "Can't write to the destination: {why}").
- **Curly quotes and apostrophes**: “ ” and ’ in every string (errors.name.invalidChar,
  errors.check.* still use straight quotes; apostrophes are mostly straight).
- **Ellipsis on what opens a panel or an editor**: Cancel…, + New preset…, + New mirror…,
  Edit… (Also ignore), Cancel Job… (menu).
- **Percent**: "{value}%" (no space, as the menu bar already does).
- **UI names quoted in prose**: “Add to queue”, “Keep both”, “Overwrite”, “Skip” where text
  refers to a button (queue.empty, errors.mhl.*, errors.import.twoReplace).

## E. Translatable structure (small code changes)

- `mirror.preview.removed` + `deleted` / `archived`, joined in markup with an arrow: two full
  strings ("{count} deleted in the origin → deleted" / "… → archived, kept {days} days").
- `{days}` isn't a plural: "1 days" shows when a mirror keeps 1 day (mirror.preview.archived,
  archivedHelp, archiveNotDeleted*, archiveNotDeletedAll). Make `days` select the form.
- `summary.mirrorChecksum` is nested in `summary.checksumError` ("The checksum file couldn't
  be written: the mirror's checksum file: …"): one full string.
- `summary.headline.someVerified/someCopied/nothingCopied` embed a `{kept}` clause, and
  `mirror.preview.startHelp.*` embed counted noun phrases: complete strings per case.

## F. Decisions for you

1. **Cancel or Stop?** The button says "Cancel", its panel asks "Stop copying?" with a "Stop"
   button, and the summary says "Cancelled". Proposed: Cancel everywhere for the user's
   action ("Cancel…", panel "Cancel this job?", buttons "Continue" / "Cancel job"); "Stopped"
   stays for a job that couldn't go on.
2. **Settings section "Every copy"** holds copy options and app options (notifications, menu
   bar). Proposed: two sections, "Copies" and "General".
3. **Menus in Title Case** (Choose Source…, Start Copy) follow macOS: proposed to keep, with
   precise words ("Cancel Job…", "Start" for any job).
4. **"Identical" → "Same size and date"** (A, last row): precise (contents aren't compared),
   one word longer. Proposed: yes.

## Not changed

Reviewers' proposals left out because they lose meaning or break conventions: dropping
"Asks first" from Cancel's help (it tells what happens), "Settings…" with an ellipsis (it
opens a screen, not a panel), shortening labels to "Write checksum file" / "Save report
beside checksum file" (articles dropped read as telegrams), lowercase menus (macOS uses Title
Case).
