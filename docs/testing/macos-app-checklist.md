# macOS app: manual checklist

Tauri's WebDriver doesn't support macOS, so the app gets this check by hand on a real Mac
with a real SD card (or any USB stick) before each release. Install the release `.dmg` as the
README's Install section says (that also checks the first-launch steps), or build it with
`npm run tauri build` from `ui/`. Then:

## Copy

1. **Normal offload.** Insert a card with some video files. Drop its directory from Finder on
   From, choose an empty destination on To. Expect: the file count, the size and the ignored
   count; "Files go to" ends where the files will land. Start with Copy & Verify. Expect: both
   bars move, active files show, the finished list fills, the summary says "N files copied and
   verified" and lists every file as ✓ Verified with its 32-character checksum.
2. **Checksum file.** Summary → Open checksum file opens `secopy_….xxh128`; in Terminal,
   `cd <destination> && xxhsum -c secopy_*.xxh128` prints `OK` for every file.
3. **Run it again.** New copy, same card, same destination. Expect: the non-empty warning
   and "N files with the same size and date will be skipped (not checked)"; the job finishes at
   once and the summary says there was nothing to copy.
4. **Different files with the same name.** Change one file on the card (or copy another
   file over it), run again with Keep both. Expect: "1 file differs from what's there", and the
   copy lands as `name (1).ext`. Run again with Overwrite: Start's status says it replaces 1
   file, and the old file is replaced only after the new copy is verified.
5. **Choose…, only what's inside, and the filter.** From → Choose…: the panel accepts a
   directory or several files. Pick the card directory, tick "Include the “…” directory" and
   turn off an extension chip. Expect: the counts and "Files go to" follow.
6. **Ignore lists.** In Settings › Always ignore when copying, add `*.XML` and Save: the ignored
   count goes up and no XML file is copied. In New copy, Also ignore › Edit…, add `*.THM`: the
   count follows, and the preset shows "Changed for this run". Restore defaults in Settings
   puts the original list back.
7. **Pause and Cancel.** Start a large copy, Pause (the bars stop), Resume, then Cancel… and
   Cancel job. Expect: the summary says Cancelled, and no `.secopy-partial` or
   `.secopy-….partial` file is left in the destination (`ls -la`).
8. **Cancel and remove.** Start a copy into a directory with a file already in it; Cancel…:
   Continue (and Esc) keep copying. Cancel… again, tick "Also remove the files already copied",
   Cancel job: the destination is as before (the old file stays, no checksum file, no new
   directories) and the summary says "Cancelled: the destination is back as it was".
9. **Pull the card.** Start a large copy and eject or pull the card mid-way. Expect: a red
   "Stopped: The source is no longer available; was it disconnected?" and a summary that lists
   what finished.
10. **Retry.** Make a file unreadable (`chmod 000` on a copy of a card directory), copy, then
    Retry. Expect: only that file is offered again. Eject the card and press Retry: it says the
    source isn't there any more; nothing starts.
11. **Save report.** Summary → Save report… writes a `.txt` and a `.json` next to it.
12. **ASC MHL.** Turn on Settings › Write ASC MHL. Copy a card: before Start, "ASC MHL: new
    history"; after, an `ascmhl` directory next to the files and "ASC MHL written." Copy the
    same card to a second destination: it continues the source's history if the card has one.
    If you have Silverstack or Hedge, open the destination: the history is read and every file
    verifies.

## Presets, settings and the app

13. **A copy preset on a real card.** Choose the card's clip directory (e.g.
    `PRIVATE/M4ROOT/CLIP`), keep only the video types, and Save as… (only a name is asked).
    Choose None, then the preset again: the source and types come back. Eject the card and
    choose the preset: "<card> isn't connected", nothing copies. Quit with the card in and open
    again: the preset and its source are loaded. In Manage presets…, Choose… another directory
    as the source and Save.
14. **Changed for this run.** With a preset applied, turn a type off: "Changed for this run"
    appears. Copy without saving; after New copy the preset is as it was. Change it again and
    Update: the next card uses the change.
15. **Settings.** Change a setting and Cancel: nothing changed. Turn the checksum file off and
    Save: the next job writes none and the summary says so. Turn it on with "Save the report
    next to the checksum file": the report files appear next to the checksum file.
16. **Language.** Settings › General › Language › Español, Save: the window, the menus and the
    menu bar switch to Spanish at once, and New copy keeps what was set up. Quit and open again:
    File › Elegir origen… shows the Open panel in Spanish. Back to Automatic: the Mac's
    language at once, and the panels too after a relaunch. File › Start is greyed out (or not)
    as before each switch.
17. **Remembered, and not.** Choose Copy, resize the window, select a preset, run a job, quit
    and open again: Copy, the size and the preset are back; the destination is empty and the
    used one is in Recent….
18. **Notification.** Start a copy, switch to another app: when it ends, a notification says
    the result. With "Notify when a job finishes" off, none. With the window in front, none.
19. **Keyboard.** ⌘O, ⌘D, ⌘↩ from the File menu (greyed out when they don't apply); Space
    pauses and resumes; ⌘. asks to cancel; ⌘1–⌘4 switch tabs and open the Queue; Esc cancels
    Settings and leaves Copy presets.
20. **VoiceOver.** With VoiceOver on (⌘F5), do a whole copy with the keyboard: every control
    is read with its name, each new screen reads its title, and the end is announced.
21. **Drops.** Drop a card (or a directory on it) on From: Source shows it with its files
    and size. Drop a directory on To: it becomes the destination, not the source.
22. **Closing and quitting.** With no job running, the red button closes the window and the
    app quits; open it again and ⌘Q quits at once. With "Keep jobs running in the menu bar…"
    off, start a copy and close the window: it asks first; Continue keeps it open, Cancel job
    and quit quits and leaves no partial file. ⌘Q during a copy asks the same.

## Queue

23. **Queue.** Set up three copies (two cards, one directory) with Add to queue; reorder them;
    Start the queue with the window in the background: one notification at the end; the queue
    summary opens each job's summary; the queue is empty.
24. **Queue failures.** Queue a card, take it out, start the queue with "Continue with the next
    job": that job fails ("<card> isn't connected."), the others run, it stays in the queue.
    With "Stop the queue": nothing after it runs.
25. **Queue interrupted.** Cancel during job 2: the queue stops, jobs 2 and 3 stay queued.
    Run again and quit during a job: on reopening, the queue still has the jobs not finished.
    Cancel while a job is being checked (just after "Job 2 of 3" appears, before files move):
    "Stop the queue?"; job 2 doesn't start. ⌘Q between two jobs asks first.

## Mirror and Verify

26. **Mirror.** Create "SSD → NAS" (origin on an SSD, destination on the SMB share). Preview:
    counts match; Start. Change a file, add one, delete one in the origin; Preview shows them;
    Start: the NAS matches, the deleted file is in `.secopy-archive/<date time>/` (Finder,
    ⇧⌘. shows hidden files), and `.secopy-checksums.xxh128` is there. Run again: "Already in
    sync".
27. **Mirror safety.** Point a preset's origin at an empty directory: Preview warns, Start asks.
    Queue that preset: the job fails with the reason, nothing removed. Pull the destination
    mid-run: files fail and "Files deleted in the origin were left in the destination".
28. **Mirror in the queue.** Queue a copy and a mirror; start the queue; both complete; the
    queue summary opens the mirror's summary.
29. **Verify.** Copy a directory with Copy & Verify; Verify the destination: all intact. Change
    one byte in one file (a hex editor), delete another, add a third: Verify says 1 changed,
    1 missing, 1 not checked; the report lists them. Verify a whole drive with several copies.
30. **Verify a mirror.** Run a mirror to the NAS; Verify its destination: all intact. Queue a
    verify of it with another job.

## Export, import and the menu bar

31. **Export and import.** File › Export… with everything ticked; open the `.secopy` in a text
    editor: settings and presets, no ids. Export one copy preset and one mirror from their
    screens. In another macOS user account, File › Import… the first file: every preset and
    the settings come in; the queue and recent destinations don't. Import it again: each
    preset offers Keep both ("Name (2)") or Replace yours; replace a mirror that is queued,
    then run the queue: the queued job runs the replaced mirror. A preset whose card isn't in
    says it isn't connected, and still imports.
32. **Import from Finder.** Double-click a `.secopy` with Secopy closed, then with it open: the
    Import screen shows it. Start a copy and double-click one: "Import it when the current job
    has finished.", nothing changes; File › Import… is greyed out while it runs, also during a
    mirror's archiving or deleting. Back on the Import screen changes nothing.
33. **Menu bar.** Start a copy that lasts a few minutes and close the window: an icon with a
    percentage appears in the menu bar and Secopy leaves the Dock and ⌘Tab; the percentage
    moves. Clicking it opens a panel under it (the job, From/To, a bar, files, speed, time
    left) that stays open while the figures update; clicking elsewhere closes it. Pause pauses
    (the title says Paused), Resume carries on. Open Secopy shows the progress screen and the
    icon goes. Close again and let it finish: ✓ and the notification; Open Secopy shows the
    summary. Check light and dark menu bars.
34. **Menu bar, the rest.** A copy with a failed file finishes hidden: ✗, "Finished with
    problems". ⌘Q while hidden and running: the window shows and asks. Open Secopy from
    Spotlight while hidden: the window shows. A queue shows `2/3 · …`. With the setting off,
    closing during a copy asks to cancel it; closing with nothing running quits.

Record the macOS version, the card reader and anything odd in the release PR.
