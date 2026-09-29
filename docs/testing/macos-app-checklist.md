# macOS app: manual checklist

Tauri's WebDriver doesn't support macOS, so the app gets this check by hand on a real Mac
with a real SD card (or any USB stick) before each release. Build it with
`npm run tauri build` from `ui/` (or install the release `.dmg` as the README's Install
section says, which also checks those first-launch steps), then:

1. **Normal offload.** Insert a card with some video files. Drop its folder from Finder on
   FROM, choose an empty destination on TO. Expect: file count, size and system files
   skipped; "Files will go to" ends in the card folder's name. Start with Copy & Verify.
   Expect: both bars move, active files show, the finished list fills, the summary says
   "All N files copied and verified" and lists every file as ✓ Verified with its checksum.
2. **Checksum file.** Summary → Open checksum file opens it; in Terminal,
   `cd <destination> && xxhsum -c secopy_*.xxh64` prints `OK` for every file.
3. **Run it again.** New copy, same card, same destination. Expect: the non-empty warning
   and "N identical files will be skipped (not checked)"; the job finishes at once and the
   summary says nothing had to be copied.
4. **Different files with the same name.** Change one file on the card (or copy another
   file over it), run again with Keep both. Expect: "1 file differs", and the copy lands as
   `name (1).ext`.
5. **Choose…, only what's inside, and the filter.** FROM → Choose…: the panel accepts a
   folder or several files. Pick the card folder, untick "Include the “…” folder" and turn
   off an extension chip. Expect: the counts and "Files will go to" follow.
6. **Pause and Cancel.** Start a large copy, Pause (the bars stop), Resume, then Cancel and
   confirm. Expect: the summary says Cancelled, and no `.secopy-partial` file is left in
   the destination (`ls -la`).
7. **Pull the card.** Start a large copy and eject/pull the card mid-way. Expect: a red
   "Stopped: the source is no longer available" and a summary that lists what finished.
8. **Retry.** Make a file unreadable (`chmod 000` on a copy of a card folder),
   copy, then Retry. Expect: only that file is offered again.
9. **Closing and quitting during a copy.** Start a copy, close the window (red button, then
   ⌘W): it asks first; Keep copying keeps it open, Stop copying quits and leaves no partial
   file. Start another copy and press ⌘Q: the same question, the same result.
10. **Save report.** Summary → Save report… writes a `.txt` and a `.json` next to it.
11. **Closing and quitting when idle.** With no copy running, the red button closes the
    window and the app quits; open it again and ⌘Q quits at once, without asking.
12. **Drops.** Drop a card (or a directory on it) on FROM: Source shows it with its files
    and size. Drop a directory on TO: it becomes the destination, not the source.
13. **A copy preset on a real card.** Choose the card's clip folder (e.g.
    `PRIVATE/M4ROOT/CLIP`), keep only the video types, and Save as… (only a name is
    asked). Choose None, then the preset again: the source and types come back. Eject the
    card and choose the preset: "<card> isn't connected", nothing copies. Quit with the
    card in and open again: the preset and its source are loaded. In Preset → Manage
    presets…, Choose… another directory as the source and Save.
14. **Changed for this run.** With a preset applied, turn a type off: "changed for this
    run" appears. Copy without saving; after New copy the preset is as it was. Change it
    again and Update: the next card uses the change.
15. **Settings.** Change a setting and Cancel: nothing changed. Turn the checksum file off
    and Save: the next job writes none and the summary says so. Turn it on with "report next to it": the report files appear next to the checksum
    file.
16. **Remembered, and not.** Choose Copy, resize the window, select a preset, run a job,
    quit and open again: Copy, the size and the preset are back; the destination is empty
    and the used one is in Recent ▾. After updating from 0.10, the profiles saved then are
    the copy presets, and the last one used is selected.
17. **The identifier move.** After updating from 0.2.0, `~/Library/Application Support/
    com.latecommits.secopy/reports/` holds the old reports.
18. **Notification.** Start a copy, switch to another app: when it ends, a notification says
    the result. With "Notify when a copy finishes" off, none. With the window in front, none.
19. **Card gone before Retry.** After a copy with a failed file, eject the card in Finder
    and press Retry: it says the source isn't there any more; nothing starts.
20. **Keyboard.** ⌘O, ⌘D, ⌘↩ from the File menu (greyed out when they don't apply); Space
    pauses and resumes; ⌘. asks to cancel; Esc cancels Settings and leaves Copy presets.
21. **VoiceOver.** With VoiceOver on (⌘F5), do a whole copy with the keyboard: every control
    is read with its name, each new screen reads its title, and the end is announced.
22. **Queue.** Set up three copies (two cards, one directory) with Add to queue; reorder them;
    Start the queue with the window in the background: one notification at the end; the queue
    summary opens each job's summary; the queue is empty.
23. **Queue failures.** Queue a card, take it out, start the queue with "Continue": that job fails
    ("<card> isn't connected."), the others run, it stays in the queue. With "Stop the queue":
    nothing after it runs.
24. **Queue interrupted.** Cancel during job 2: the queue stops, jobs 2 and 3 stay queued.
    Run again and quit during a job: on reopening, the queue still has the jobs not finished.
    Cancel while a job is being checked (just after "Job 2 of 3" appears, before files
    move): job 2 doesn't start. ⌘Q between two jobs asks first.
25. **Mirror.** Create "SSD → NAS" (origin on an SSD, destination on the SMB share). Preview:
    counts match; Start. Change a file, add one, delete one in the origin; Preview shows
    them; Run: the NAS matches, the deleted file is in `.secopy-archive/<date>/` (Finder,
    ⇧⌘. shows hidden files). Run again: "Already in sync".
26. **Mirror safety.** Point a preset's origin at an empty directory: Preview warns, Run asks.
    Queue that preset: the job fails with the reason, nothing removed. Pull the destination
    mid-run: files fail and "Files deleted in the origin were left in the destination".
27. **Mirror in the queue.** Queue a copy and a mirror; start the queue; both complete; the queue
    summary opens the mirror's summary.
28. **Cancel and remove.** Start a copy into a directory with a file already in it; Cancel:
    Continue (and Esc) keep copying. Cancel again, tick "Also remove the files already
    copied", Stop: the destination is as before (the old file stays, no checksum file, no new
    directories) and the summary says "Cancelled: the destination is back as it was". Cancel
    without ticking: the copied files stay.
29. **Verify.** Copy a directory with Copy & Verify; Verify the destination: all intact. Change
    one byte in one file (a hex editor), delete another, add a third: Verify says 1 changed,
    1 missing, 1 not checked; the report lists them. Verify a whole drive with several copies.
30. **Verify a mirror.** Run a mirror to the NAS; Verify its destination: all intact, and
    `.secopy-checksums.xxh64` is there (⇧⌘. in Finder). Queue a verify of it with another job.
31. **Export and import.** File › Export… with everything ticked; open the `.secopy` in a text
    editor: settings and presets, no ids. Export one copy preset and one mirror from their
    screens. In another macOS user account, File › Import… the first file: every preset and
    the settings come in; the queue and recent destinations don't. Import it again: each
    preset offers Keep both ("Name (2)") or Replace; Replace a mirror that is queued, then
    run the queue: the queued job runs the replaced mirror. A preset whose card isn't in says
    it isn't connected, and still imports.
32. **Import from Finder.** Double-click a `.secopy` with Secopy closed, then with it open: the
    Import screen shows it. Start a copy and double-click one: "Import it when the current
    job has finished.", nothing changes; File › Import… is greyed out while it runs, also
    during a mirror's archiving or deleting. Back on the
    Import screen changes nothing.
33. **Menu bar.** Start a copy that lasts a few minutes and close the window: an icon with a
    percentage appears in the menu bar and Secopy leaves the Dock and ⌘Tab; the percentage
    moves. Clicking it opens a panel under it (the job, From/To, a bar, files, speed, time
    left) that stays open while the figures update; clicking elsewhere closes it. Pause pauses
    (the title says Paused), Resume carries on. Open Secopy shows the progress screen and the
    icon goes. Close
    again and let it finish: ✓ and the notification; Open Secopy shows the summary. Check light
    and dark menu bars.
34. **Menu bar, the rest.** A copy with a failed file finishes hidden: ✗, "Finished with
    problems". ⌘Q while hidden and running: the window shows and asks. Open Secopy from
    Spotlight while hidden: the window shows. A queue shows `2/3 · …`. With the setting off,
    closing during a copy asks to stop it, as before; closing with nothing running quits.

Record the macOS version, the card reader and anything odd in the release PR.
