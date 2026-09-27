# macOS app: manual checklist

Tauri's WebDriver doesn't support macOS, so the app gets this check by hand on a real Mac
with a real SD card (or any USB stick) before each release. Build it with
`npm run tauri build` from `ui/` (or install the release `.dmg`), then:

1. **Normal offload.** Insert a card with some video files. Drop its folder from Finder on
   FROM, choose an empty destination on TO. Expect: file count, size and hidden items
   skipped; "Files will go to" ends in the card folder's name. Start with Copy & Verify.
   Expect: both bars move, active files show, the finished list fills, the summary says
   "All N files copied and verified".
2. **Checksum file.** Summary → Open checksum file opens it; in Terminal,
   `cd <destination> && xxhsum -c secopy_*.xxh64` prints `OK` for every file.
3. **Run it again.** New copy, same card, same destination. Expect: the non-empty warning
   and "N identical files will be skipped (not checked)"; the job finishes at once and the
   summary says nothing had to be copied.
4. **Different files with the same name.** Change one file on the card (or copy another
   file over it), run again with Keep both. Expect: "1 file differs", and the copy lands as
   `name (1).ext`.
5. **Only what's inside, and the filter.** Choose "Copy only what's inside" and turn off an
   extension chip. Expect: the counts and "Files will go to" follow.
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

Record the macOS version, the card reader and anything odd in the release PR.
