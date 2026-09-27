// A fake `Api` for component tests: every call is recorded, and answers come from the
// views the test sets.

import { vi } from "vitest";
import type { Api } from "../lib/api";
import type {
  DestinationView,
  DriveView,
  Profile,
  ProfileInput,
  ProfilesView,
  ProgressView,
  SessionView,
  Settings,
  SourceView,
  StartView,
  SummaryView,
} from "../lib/bindings";

export function sourceView(over: Partial<SourceView> = {}): SourceView {
  return {
    label: "/Volumes/CARD/DCIM",
    isFolder: true,
    contentsOnly: false,
    folder: "/Volumes/CARD/DCIM",
    isRetry: false,
    rootDir: "DCIM",
    files: 1284,
    bytes: 212_400_000_000,
    extensions: [
      { key: "mov", label: ".mov", files: 1020, bytes: 208_000_000_000 },
      { key: "wav", label: ".wav", files: 240, bytes: 4_100_000_000 },
      { key: "xml", label: ".xml", files: 24, bytes: 2_000_000 },
    ],
    selectedExtensions: null,
    skippedHidden: 37,
    skippedSymlinks: 0,
    problems: [],
    problemCount: 0,
    ...over,
  };
}

export function destinationView(over: Partial<DestinationView> = {}): DestinationView {
  return {
    path: "/Volumes/RAID/Day01",
    copyRoot: "/Volumes/RAID/Day01/DCIM",
    blocker: null,
    freeBytes: 1_800_000_000_000,
    fsKind: "APFS",
    existingItems: null,
    problems: [],
    problemCount: 0,
    identical: 0,
    differs: 0,
    stalePartials: 0,
    ...over,
  };
}

export function sessionView(over: Partial<SessionView> = {}): SessionView {
  return {
    source: null,
    selectedFiles: 0,
    selectedBytes: 0,
    destination: null,
    conflicts: "keepBoth",
    plan: null,
    profileId: null,
    profileChanged: false,
    pickProblem: null,
    suggestedFolder: "",
    stale: false,
    ...over,
  };
}

/** A session with a source and a destination, ready to start. */
export function readyView(over: Partial<SessionView> = {}): SessionView {
  return sessionView({
    source: sourceView(),
    selectedFiles: 1284,
    selectedBytes: 212_400_000_000,
    destination: destinationView(),
    plan: { filesToWrite: 1284, bytesToWrite: 212_400_000_000, blocker: null },
    ...over,
  });
}

export function progressView(over: Partial<ProgressView> = {}): ProgressView {
  return {
    phase: "copying",
    elapsedMs: 0,
    paused: false,
    verify: true,
    totalFiles: 1284,
    totalBytes: 212_400_000_000,
    copiedBytes: 0,
    verifiedBytes: 0,
    filesDone: 0,
    filesSkipped: 0,
    filesFailed: 0,
    active: [],
    smallFiles: null,
    fatal: null,
    ...over,
  };
}

export function summaryView(over: Partial<SummaryView> = {}): SummaryView {
  return {
    outcome: "complete",
    stoppedBecause: null,
    verify: true,
    files: 1284,
    copied: 0,
    verified: 1284,
    skippedIdentical: 0,
    skippedDifferent: 0,
    failed: 0,
    notStarted: 0,
    bytesWritten: 212_400_000_000,
    millis: 252_000,
    failures: [],
    copyRoot: "/Volumes/RAID/Day01/DCIM",
    checksumFile: "/Volumes/RAID/Day01/secopy_2026-09-27_140302.xxh64",
    checksumError: null,
    checksumOff: false,
    reportFile: "/Users/me/Library/Application Support/com.latecommits.secopy/reports/r.txt",
    reportError: null,
    finished: 1284,
    ...over,
  };
}

export function profile(over: Partial<Profile> = {}): Profile {
  return {
    id: "fx3",
    name: "Sony FX3",
    folder: "PRIVATE/M4ROOT/CLIP",
    includeFolder: true,
    extensions: ["mp4"],
    ...over,
  };
}

export function settingsView(over: Partial<Settings> = {}): Settings {
  return { writeChecksumFile: true, showHiddenCount: true, reportNextToChecksum: false, ...over };
}

export function drive(over: Partial<DriveView> = {}): DriveView {
  return { name: "CARD_A", path: "/Volumes/CARD_A", totalBytes: 64_000_000_000, freeBytes: 20_000_000_000, ...over };
}

export function startView(over: Partial<StartView> = {}): StartView {
  return {
    session: sessionView(),
    settings: settingsView(),
    profiles: [],
    verify: true,
    recentDestinations: [],
    warnings: [],
    ...over,
  };
}

/** Every method is a spy; `session` is what the session commands answer. */
export function fakeApi(session: SessionView = sessionView()) {
  const state = {
    session,
    progress: null as ((p: ProgressView) => void) | null,
    drop: null as ((paths: string[], target: Element | null) => void) | null,
    close: null as ((prevent: () => void) => Promise<void>) | null,
    start: startView({ session }),
    openSettings: null as (() => void) | null,
  };
  const answer = () => Promise.resolve(state.session);
  const profilesAnswer = () =>
    Promise.resolve({ profiles: state.start.profiles, session: state.session } as ProfilesView);
  const api = {
    scanSource: vi.fn(answer),
    setIncludeFolder: vi.fn((_include: boolean) => answer()),
    clearSource: vi.fn(answer),
    setFilter: vi.fn(answer),
    setDestination: vi.fn(answer),
    setConflicts: vi.fn(answer),
    sessionView: vi.fn(answer),
    startJob: vi.fn((_verify: boolean, onProgress: (p: ProgressView) => void) => {
      state.progress = onProgress;
      return Promise.resolve(null);
    }),
    pauseJob: vi.fn(() => Promise.resolve()),
    resumeJob: vi.fn(() => Promise.resolve()),
    cancelJob: vi.fn(() => Promise.resolve()),
    jobRunning: vi.fn(() => Promise.resolve(false)),
    finishedPage: vi.fn((_o: number, _l: number, _f: boolean) => Promise.resolve([] as Awaited<ReturnType<Api["finishedPage"]>>)),
    jobSummary: vi.fn(() => Promise.resolve(summaryView() as SummaryView | null)),
    saveReport: vi.fn((_p: string) => Promise.resolve(null)),
    retryFailed: vi.fn(answer),
    appStart: vi.fn(() => Promise.resolve(state.start)),
    recentDestinations: vi.fn(() => Promise.resolve(state.start.recentDestinations)),
    listDrives: vi.fn(() => Promise.resolve([drive()] as DriveView[])),
    selectProfile: vi.fn((_id: string | null) => answer()),
    updateProfile: vi.fn(profilesAnswer),
    saveProfileAs: vi.fn((_name: string, _folder: string) => profilesAnswer()),
    createProfile: vi.fn((_input: ProfileInput) => Promise.resolve(state.start.profiles)),
    editProfile: vi.fn((_id: string, _input: ProfileInput) => profilesAnswer()),
    deleteProfile: vi.fn((_id: string) => profilesAnswer()),
    setSettings: vi.fn((s: Settings) => Promise.resolve(s)),
    setMode: vi.fn((_verify: boolean) => Promise.resolve(null)),
    onOpenSettings: vi.fn((handler: () => void) => {
      state.openSettings = handler;
      return Promise.resolve(() => {});
    }),
    pickSource: vi.fn(() => Promise.resolve(["/Volumes/CARD/DCIM"] as string[] | null)),
    pickDestination: vi.fn(() => Promise.resolve("/Volumes/RAID/Day01" as string | null)),
    pickReportPath: vi.fn((_s: string) => Promise.resolve("/tmp/report.txt" as string | null)),
    confirm: vi.fn((_m: string, _t: string, _ok?: string, _cancel?: string) => Promise.resolve(true)),
    reveal: vi.fn((_p: string) => Promise.resolve()),
    openFile: vi.fn((_p: string) => Promise.resolve()),
    onDrop: vi.fn((handler: (paths: string[], target: Element | null) => void) => {
      state.drop = handler;
      return Promise.resolve(() => {});
    }),
    onCloseRequested: vi.fn((handler: (prevent: () => void) => Promise<void>) => {
      state.close = handler;
      return Promise.resolve(() => {});
    }),
  } satisfies Api;
  return { api, state };
}
