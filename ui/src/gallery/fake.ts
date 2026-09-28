// Fake data and a fake Api for the gallery: every screen without Tauri. Dev only.
import type { Api } from "../lib/api";
import type { FinishedRow, MirrorPreset, MirrorPreviewView, PreviewRow, Profile, ProgressView, QueueSummaryView, QueueView, SessionView, Settings, SummaryView } from "../lib/bindings";

export const profiles: Profile[] = [
  { id: "fx3", name: "Sony FX3", source: "/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP", includeFolder: true, extensions: ["mp4"] },
  { id: "dji", name: "DJI Mini 4", source: "/Volumes/DJI/DCIM", includeFolder: false, extensions: null },
];

export const settings: Settings = { writeChecksumFile: true, showSystemCount: true, reportNextToChecksum: false, notifyWhenDone: true };

export const session: SessionView = {
  source: {
    label: "/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP",
    isFolder: true,
    contentsOnly: false,
    folder: "/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP",
    isRetry: false,
    rootDir: "CLIP",
    files: 212,
    bytes: 180_400_000_000,
    extensions: [
      { key: "mp4", label: ".mp4", files: 106, bytes: 180_000_000_000 },
      { key: "xml", label: ".xml", files: 106, bytes: 400_000 },
    ],
    selectedExtensions: ["mp4"],
    skippedSystem: 4,
    skippedSymlinks: 0,
    problems: [],
    problemCount: 0,
  },
  selectedFiles: 106,
  selectedBytes: 180_000_000_000,
  destination: {
    path: "/Volumes/V001/Day01",
    copyRoot: "/Volumes/V001/Day01/CLIP",
    blocker: null,
    freeBytes: 1_800_000_000_000,
    fsKind: "APFS",
    existingItems: 12,
    problems: [],
    problemCount: 0,
    identical: 0,
    differs: 2,
    stalePartials: 0,
  },
  conflicts: "keepBoth",
  plan: { filesToWrite: 106, bytesToWrite: 180_000_000_000, blocker: null },
  profileId: "fx3",
  profileChanged: true,
  pickProblem: null,
  stale: false,
};

export const progress: ProgressView = {
  phase: "copying",
  elapsedMs: 60_000,
  paused: false,
  verify: true,
  totalFiles: 106,
  totalBytes: 180_000_000_000,
  copiedBytes: 72_000_000_000,
  verifiedBytes: 60_000_000_000,
  filesDone: 40,
  filesSkipped: 0,
  filesFailed: 1,
  active: [
    { id: 41, name: "C0041.MP4", path: "CLIP/C0041.MP4", verifying: false, size: 2_300_000_000, bytesDone: 1_100_000_000 },
    { id: 39, name: "C0039.MP4", path: "CLIP/C0039.MP4", verifying: true, size: 2_100_000_000, bytesDone: 900_000_000 },
  ],
  smallFiles: null,
  fatal: null,
  removing: 0,
  archiving: false,
};

function row(i: number, status: FinishedRow["status"] = "verified"): FinishedRow {
  const name = `CLIP/DJI_20260919113404_00${String(i).padStart(2, "0")}_D_LITOX1.MP4`;
  return {
    id: i,
    path: name,
    finalPath: name,
    size: 2_300_000_000,
    millis: 2_100,
    hash: `d78a9dd8afc9649${i % 10}`,
    status,
    reason: status === "failed" ? "Cannot read source: permission denied" : null,
  };
}

export const summary: SummaryView = {
  outcome: "failures",
  stoppedBecause: null,
  verify: true,
  files: 106,
  copied: 0,
  verified: 105,
  skippedIdentical: 0,
  skippedDifferent: 0,
  failed: 1,
  notStarted: 0,
  bytesWritten: 180_000_000_000,
  millis: 252_000,
  failures: [row(17, "failed")],
  finished: 40,
  copyRoot: "/Volumes/V001/Day01/CLIP",
  checksumFile: "/Volumes/V001/Day01/secopy_2026-09-27_140302.xxh64",
  checksumError: null,
  checksumOff: false,
  reportFile: "/x/r.txt",
  reportError: null,
  mirror: null,
  undone: null,
};

export const mirrors: MirrorPreset[] = [
  { id: "m1", name: "Footage → NAS", origin: "/Volumes/SSD/Footage", destination: "/Volumes/Media/Footage", deleted: { mode: "archive", days: 30 }, deepCheck: false },
  { id: "m2", name: "Photos → Backup", origin: "/Users/me/Pictures/Photos", destination: "/Volumes/Backup/Photos", deleted: { mode: "delete", days: 30 }, deepCheck: true },
];

export const mirrorPreview: MirrorPreviewView = {
  presetId: "m1",
  name: "Footage → NAS",
  origin: "/Volumes/SSD/Footage",
  destination: "/Volumes/Media/Footage",
  newFiles: 12,
  newBytes: 38_200_000_000,
  changedFiles: 3,
  changedBytes: 9_400_000_000,
  removedFiles: 5,
  archiveDays: 30,
  unchanged: 2410,
  guard: null,
};

const previewRows: PreviewRow[] = [
  { path: "2026/09/A001_C001.mov", size: 4_100_000_000, kind: "new", reason: "New" },
  { path: "2026/09/A001_C002.mov", size: 3_900_000_000, kind: "new", reason: "New" },
  { path: "2026/08/Edit_v3.prproj", size: 48_000_000, kind: "changed", reason: "Newer in the origin" },
  { path: "2026/07/B002_C010.mov", size: 2_200_000_000, kind: "removed", reason: "Deleted in the origin" },
];

export const mirrorSummary: SummaryView = {
  ...summary,
  outcome: "complete",
  failed: 0,
  failures: [],
  files: 15,
  verified: 15,
  checksumFile: null,
  checksumOff: true,
  copyRoot: "/Volumes/Media/Footage",
  mirror: { new: 12, updated: 3, removed: 5, archived: true, removalFailures: [], nothingRemoved: null },
};

export const queue: QueueView = {
  onFailure: "continue",
  running: false,
  jobs: [
    { kind: "copy", verify: true, source: "/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP", destination: "/Volumes/V001/Day01", lastError: null, supported: true, name: null },
    { kind: "copy", verify: true, source: "/Volumes/CARD_B/PRIVATE/M4ROOT/CLIP", destination: "/Volumes/V001/Day01", lastError: "CARD_B isn't connected.", supported: true, name: null },
    { kind: "copy", verify: false, source: "/Users/me/Desktop/Stills", destination: "/Volumes/Media/Stills", lastError: null, supported: true, name: null },
  ],
};

export const queueSummary: QueueSummaryView = {
  complete: 1,
  count: 3,
  millis: 6_130_000,
  results: [
    { job: queue.jobs[0], result: "complete", reason: null, summary: { ...summary, outcome: "complete", failed: 0, failures: [] } },
    { job: queue.jobs[1], result: "failed", reason: "CARD_B isn't connected.", summary: null },
    { job: queue.jobs[2], result: "notRun", reason: "Not run: the queue stopped.", summary: null },
  ],
};

const ok =
  <T>(value: T) =>
  () =>
    Promise.resolve(value);

/** Answers every call with the fake data above. */
export function fakeApi(start: Partial<{ profiles: Profile[] }> = {}): Api {
  const table: Record<string, unknown> = {
    appStart: ok({
      session,
      settings,
      profiles: start.profiles ?? profiles,
      verify: true,
      recentDestinations: ["/Volumes/V001/Day01", "/Volumes/V001/Day00"],
      warnings: [],
      lastProfile: null,
    }),
    finishedPage: (offset: number, limit: number) =>
      Promise.resolve(Array.from({ length: Math.max(0, Math.min(limit, 40 - offset)) }, (_, i) => row(offset + i + 1))),
    onDrop: ok(() => {}),
    onCloseRequested: ok(() => {}),
    onOpenSettings: ok(() => {}),
    jobRunning: ok(false),
    recentDestinations: ok([]),
    confirm: ok(true),
    queue: ok(queue),
    mirrorPresets: ok(mirrors),
    mirrorPreviewPage: (kind: string | null) => Promise.resolve(previewRows.filter((r) => kind === null || r.kind === kind)),
  };
  return new Proxy({} as Api, { get: (_target, key: string) => table[key] ?? ok(session) });
}
