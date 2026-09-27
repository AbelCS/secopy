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
12. **Drives.** Insert a card and a USB drive: both appear in FROM within 2 s; eject one and
    it goes away. The Mac's own disk and the drive holding the destination aren't listed.
13. **A profile on a real card.** Save as new… on the card's clip folder (e.g.
    `PRIVATE/M4ROOT/CLIP`), with only the video types on. Eject, insert another card of the
    same camera, click its drive: the profile's folder and types apply. Insert a card of
    another camera: "<card> has no <folder>", nothing copies. In Profile → Manage profiles…,
    Choose… a folder on the card: the folder fills in relative to the card; change a type
    and Save.
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
19. **Eject.** After copying from a card, "Eject <card>" ejects it and says so. Open a file
    from the card in QuickTime and try again: the reason is shown, the card stays mounted.
    With a removable destination, "Safe to eject <drive>" shows.
20. **Keyboard.** ⌘O, ⌘D, ⌘↩ from the File menu (greyed out when they don't apply); Space
    pauses and resumes; ⌘. asks to cancel; Esc cancels Settings and leaves Profiles.
21. **VoiceOver.** With VoiceOver on (⌘F5), do a whole copy with the keyboard: every control
    is read with its name, each new screen reads its title, and the end is announced.

Record the macOS version, the card reader and anything odd in the release PR.
