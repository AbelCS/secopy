# Job Queue and Sidebar — Design

On top of 0.6.0. Requirements live in [RFD 0001](../../rfd/0001-secopy.md) (§5.7, FR-39..FR-43);
decisions made here are also logged in RFD §14. Issue: #50. Target release: 0.7.0. The mirror
(#51, [design](2026-09-28-mirror-design.md)) comes next and adds mirror runs to this queue.

## Goal

Set up several jobs, leave the Mac working (overnight, for example), and find every result
ready the next morning. Anything that can be run by hand can be queued; nothing is special-cased.

**Success looks like:** three cards set up one after another with **Add to queue**, then
**Run queue**; the next morning a notification says "Queue done: 3 of 3 jobs complete", and
the queue summary opens each job's own summary.

## Decisions

| # | Decision |
|---|---|
| Q1 | **Same app, sections in a sidebar:** Copy · Mirror · Queue. One engine, one look, and the queue holds every kind of job (copies now, mirror runs in 0.8.0). A separate mirror app was rejected: a duplicated engine and no shared queue. |
| Q2 | **A queued job is a snapshot of a setup**, not a reference to the screen: source, destination, mode, file types, include-the-directory and what to do with existing files. It can be removed and added again, not edited. |
| Q3 | **Jobs are checked again when their turn comes:** the source is scanned and the destination checked then, so a job copies what is there at that moment, and a job whose card isn't inserted fails with that reason. |
| Q4 | **On failure: continue with the next job (default) or stop the queue**, one choice for the whole queue. A job fails when it can't start or ends with failed files. |
| Q5 | **Cancel stops the current job and the queue.** The jobs not run stay queued. (Skipping just one job is a later addition if needed.) |
| Q6 | **The queue is saved** (`queue.json`). Jobs that finished well leave it at the end of a run; failed and not-run jobs stay, with their reason, ready to run again. |
| Q7 | **One notification for the whole queue**, not one per job; the Mac stays awake for the whole run. |

## 1. Sidebar

- A `Sidebar` component in the design system: **Copy**, **Mirror** (from 0.8.0; hidden until
  then), **Queue** with the number of queued jobs. The selected section is highlighted.
- The sidebar shows on the section screens (New copy, a copy's Summary, Queue, the queue
  summary). It is hidden while jobs run (the running screen takes the whole window, as today)
  and on Settings and Profiles, which have their own Cancel / Back.
- Settings stays at the top right of each section. Profiles stays inside Copy.
- Keyboard: ⌘1 Copy, ⌘2 Mirror, ⌘3 Queue (in a new **View** menu).

## 2. Adding a copy job

- New copy's action bar: **Add to queue** (secondary) next to **Start copy**. Enabled exactly
  when Start copy is.
- It stores the snapshot (Q2) and says "Added to the queue (3 jobs)" in the status for a few
  seconds. The source is then cleared and the destination kept, like New copy after a job.
- The settings that apply when the job runs (checksum file, report next to it) are the ones
  in Settings at that time.

## 3. Queue screen

```
Queue                                                        [Settings]
QUEUE                                                           3 jobs
 1  Copy & Verify  /Volumes/CARD_A/…/CLIP → /Volumes/V001/Day01   [↑][↓][✕]
 2  Copy           /Volumes/CARD_B/DCIM   → /Volumes/V001/Day01   [↑][↓][✕]
 3  Copy & Verify  /Users/me/Desktop/A    → /Volumes/Media/A      ✗ A isn't there any more.
IF A JOB FAILS   (●) Continue with the next job   ( ) Stop the queue
[Clear queue…]                                                [Run queue]
```

- Each row: position, mode, source → destination (long paths keep their end visible), and
  the reason when it failed last time. ↑ / ↓ reorder; ✕ removes (no confirmation; it's one
  row). **Clear queue…** asks first.
- **Run queue** is the primary action; disabled with an empty queue. With no jobs, an empty
  state explains how to add one.
- The failure choice is saved with the queue.

## 4. Running

- The Copying screen is the one used today, with the queue's progress in the status
  ("Job 2 of 3 · 40 / 106 files").
- A job that can't start (source or destination missing, nothing to copy, destination
  blocked) gets its reason and counts as failed (Q4); the screen shows "Checking…" while
  it's checked.
- Pause pauses the current job. Cancel asks "Stop copying and stop the queue?" (Q5).
- Quitting during a run asks, as today; the current job is stopped and marked "Stopped",
  the rest stay queued.
- Between jobs nothing waits: the next one starts when the previous one's summary is saved.

## 5. Queue summary

```
Queue done: 2 of 3 jobs complete                              took 1:42:10
 ✓  Copy & Verify  CARD_A/…/CLIP → V001/Day01   All 212 files copied and verified   [Summary]
 ✗  Copy           CARD_B/DCIM   → V001/Day01   3 files failed                       [Summary]
 –  Copy & Verify  Desktop/A     → Media/A      Not run: the queue stopped
                                                                        [Done]
```

- One row per job run in this queue run: result icon and word, the job, its headline.
  **Summary** opens that job's Summary (with Retry failed, Show in Finder, Save report…);
  Back returns to the queue summary.
- A notification when the queue ends and the window isn't in front (the Settings option
  applies): "Queue done: 2 of 3 jobs complete".
- **Done** goes to the Queue screen, which now holds only the failed and not-run jobs (Q6).

## Architecture

| Unit | Purpose |
|---|---|
| `secopy-app` `queue` | The saved queue (`queue.json`): jobs, the failure choice. Add, remove, move, clear; results of the last run. |
| `secopy-app` `jobs` | Keeps each finished job's summary for the queue run (today it keeps only the last job). |
| `secopy-app` queue runner | Runs the jobs one after another on the existing job runner: builds each job's session from its snapshot (scan, plan, checks), starts it, waits, records the result; one keep-awake for the run. |
| commands | `queue`, `add_to_queue`, `remove_from_queue`, `move_in_queue`, `clear_queue`, `set_queue_on_failure`, `run_queue` (progress on a channel, with the job index), `queue_summary`, `queue_job_summary(index)`. |
| UI | `Sidebar`, the Queue screen, Add to queue on New copy, "Job n of m" in the Copying status, the queue summary. |

`queue.json` (version 1): `{ "version": 1, "onFailure": "continue", "jobs": [ { "kind": "copy",
"sources": ["/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP"], "includeFolder": true, "extensions":
["mp4"], "destination": "/Volumes/V001/Day01", "conflicts": "keepBoth", "verify": true,
"lastError": null } ] }`. Unknown `kind`s (written by a newer version) are kept and shown as
"Needs a newer Secopy", never dropped.

## Errors

- A job's source or destination missing: the job fails with the same message the Copy screen
  would show ("CARD_A isn't connected.").
- `queue.json` damaged: renamed aside and an empty queue used, with the usual one-time message
  (like the other saved files).
- A queue run can't start while a single job runs, and vice versa (one job at a time).

## Testing

- **Rust:** the queue file round trip, damaged file, unknown kind kept; add/move/remove/clear;
  the runner with fake jobs: continue vs stop on failure, a job that can't start, cancel stops
  the queue, finished jobs leave and failed ones stay with their reason; each job re-scanned
  at its turn (a file added to the source after queuing is copied).
- **UI (Vitest):** Sidebar (sections, count, hidden while running); Add to queue enabled like
  Start and clearing the source; Queue screen (reorder, remove, clear asks, failure choice,
  empty state); "Job n of m"; the queue summary and opening a job's summary; one notification.
- **Manual:** three real cards queued and run with the window in the background; a queued
  card pulled out before its turn; Cancel during job 2; quit during a run and reopen.

## Review focus

1. **A job copies what's there at its turn**, not what the screen showed when it was added.
2. **Stop on failure** really stops before the next job starts, including a job that can't start.
3. **Cancel or quit mid-queue** leaves no job lost from `queue.json`.
4. **Two runs at once** (Run queue while a single job runs, or the reverse) is impossible.
5. **Memory over a long queue:** every job's finished list is kept for the queue summary;
   many large jobs must stay within NFR-4 (bounded memory), e.g. by keeping only the current
   job's list in memory and reading earlier ones back from their saved reports.

## Changed after release

2026-09-28 (#69): "Job n of m" stays in the status line, not next to the title; a job that
can't start shows "Checking…" briefly instead of no screen; the queue summary has no
**Show queue** (Done opens the Queue). What was built works, so the spec follows it.
