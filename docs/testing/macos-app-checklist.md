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
8. **Retry failed.** Make a file unreadable (`chmod 000` on a copy of a card folder),
   copy, then Retry failed. Expect: only that file is offered again.
9. **Closing and quitting during a copy.** Start a copy, close the window (red button, then
   ⌘W): it asks first; Keep copying keeps it open, Stop copying quits and leaves no partial
   file. Start another copy and press ⌘Q: the same question, the same result.
10. **Save report.** Summary → Save report… writes a `.txt` and a `.json` next to it.
11. **Closing and quitting when idle.** With no copy running, the red button closes the
    window and the app quits; open it again and ⌘Q quits at once, without asking.
12. **Drops.** Drop a card (or a directory on it) on FROM: Source shows it with its files
    and size. Drop a directory on TO: it becomes the destination, not the source.
13. **A profile on a real card.** Choose the card's clip folder (e.g.
    `PRIVATE/M4ROOT/CLIP`), keep only the video types, and Save as new… (only a name is
    asked). Choose None, then the profile again: the source and types come back. Eject the
    card and choose the profile: "<card> isn't connected", nothing copies. Quit with the
    card in and open again: the profile and its source are loaded. In Profile → Manage
    profiles…, Choose… another directory as the source and Save.
14. **Changed for this run.** With a profile applied, turn a type off: "changed for this
    run" appears. Copy without saving; after New copy the profile is as it was. Change it
    again and Update profile: the next card uses the change.
15. **Settings.** Change a setting and Cancel: nothing changed. Turn the checksum file off
    and Save: the next job writes none and the summary says so. Turn it on with "report next to it": the report files appear next to the checksum
    file.
16. **Remembered, and not.** Choose Copy, resize the window, select a profile, run a job,
    quit and open again: Copy, the size and the profile are back; the destination is empty
    and the used one is in Recent ▾.
17. **The identifier move.** After updating from 0.2.0, `~/Library/Application Support/
    com.latecommits.secopy/reports/` holds the old reports.
18. **Notification.** Start a copy, switch to another app: when it ends, a notification says
    the result. With "Notify when a copy finishes" off, none. With the window in front, none.
19. **Card gone before Retry.** After a copy with a failed file, eject the card in Finder
    and press Retry failed: it says the source isn't there any more; nothing starts.
20. **Keyboard.** ⌘O, ⌘D, ⌘↩ from the File menu (greyed out when they don't apply); Space
    pauses and resumes; ⌘. asks to cancel; Esc cancels Settings and leaves Profiles.
21. **VoiceOver.** With VoiceOver on (⌘F5), do a whole copy with the keyboard: every control
    is read with its name, each new screen reads its title, and the end is announced.
22. **Queue.** Set up three copies (two cards, one directory) with Add to queue; reorder them;
    Run queue with the window in the background: one notification at the end; the queue
    summary opens each job's summary; the queue is empty.
23. **Queue failures.** Queue a card, take it out, Run queue with "Continue": that job fails
    ("<card> isn't connected."), the others run, it stays in the queue. With "Stop the queue":
    nothing after it runs.
24. **Queue interrupted.** Cancel during job 2: the queue stops, jobs 2 and 3 stay queued.
    Run again and quit during a job: on reopening, the queue still has the jobs not finished.
    Cancel while a job is being checked (just after "Job 2 of 3" appears, before files
    move): job 2 doesn't start. ⌘Q between two jobs asks first.

Record the macOS version, the card reader and anything odd in the release PR.
