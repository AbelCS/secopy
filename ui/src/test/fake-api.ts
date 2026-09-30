// A fake `Api` for component tests: every call is recorded, and answers come from the
// views the test sets.

import { vi } from "vitest";
import type { Api } from "../lib/api";
import type {
  CheckView,
  ComparedView,
  CopyPreset,
  CopyPresetInput,
  CopyPresetsView,
  DestinationView,
  FinishedRow,
  MirrorPreset,
  MirrorPresetInput,
  MirrorPreviewView,
  OnFailure,
  PreviewKind,
  PreviewRow,
  ProgressView,
  QueuedJobView,
  QueueEvent,
  QueueView,
  SessionView,
  Settings,
  SourceView,
  StartView,
  SummaryView,
  ImportChoices,
  ImportDone,
  ImportView,
  PanelView,
  Message,
  ArchiveView,
  ArchiveDeletedView,
} from "../lib/bindings";

export function sourceView(over: Partial<SourceView> = {}): SourceView {
  return {
    label: raw("/Volumes/CARD/DCIM"),
    isFolder: true,
    contentsOnly: false,
    folder: "/Volumes/CARD/DCIM",
    isRetry: false,
    rootDir: "DCIM",
    files: 1284,
    bytes: 212_400_000_000,
    extensions: [
      { key: "mov", files: 1020, bytes: 208_000_000_000 },
      { key: "wav", files: 240, bytes: 4_100_000_000 },
      { key: "xml", files: 24, bytes: 2_000_000 },
    ],
    selectedExtensions: null,
    skippedSystem: 37,
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
    fsKind: "apfs",
    fsName: null,
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
    presetId: null,
    presetChanged: false,
    pickProblem: null,
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
    plan: { filesToWrite: 1284, bytesToWrite: 212_400_000_000, blocker: null, purgeable: null },
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
    removing: 0,
    archiving: false,
    undoing: false,
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
    unread: 0,
    durabilityError: null,
    dirErrors: 0,
    notStarted: 0,
    bytesWritten: 212_400_000_000,
    millis: 252_000,
    failures: [],
    copyRoot: "/Volumes/RAID/Day01/DCIM",
    checksumFile: "/Volumes/RAID/Day01/secopy_2026-09-27_140302.xxh64",
    checksumError: null,
    checksumOff: false,
    reportFile: "/Users/me/Library/Application Support/com.latecommits.secopy/reports/r.txt",
    reportErrors: [],
    mirror: null,
    undone: null,
    check: null,
    finished: 1284,
    ...over,
  };
}

export function copyPreset(over: Partial<CopyPreset> = {}): CopyPreset {
  return {
    id: "fx3",
    name: "Sony FX3",
    source: "/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP",
    includeFolder: true,
    extensions: ["mp4"],
    ...over,
  };
}

export function settingsView(over: Partial<Settings> = {}): Settings {
  return { writeChecksumFile: true, showSystemCount: true, reportNextToChecksum: false, notifyWhenDone: true, keepInMenuBar: true, ...over };
}

export function startView(over: Partial<StartView> = {}): StartView {
  return {
    session: sessionView(),
    settings: settingsView(),
    copyPresets: [],
    verify: true,
    recentDestinations: [],
    warnings: [],
    lastPreset: null,
    ...over,
  };
}

export function queuedJob(over: Partial<QueuedJobView> = {}): QueuedJobView {
  return {
    kind: "copy",
    verify: true,
    source: raw("/Volumes/CARD/DCIM"),
    destination: "/Volumes/RAID/Day01",
    lastError: null,
    supported: true,
    name: null,
    ...over,
  };
}

export function mirrorPreset(over: Partial<MirrorPreset> = {}): MirrorPreset {
  return {
    id: "m1",
    name: "Footage → NAS",
    origin: "/Volumes/SSD/Footage",
    destination: "/Volumes/Media/Footage",
    deleted: { mode: "archive", days: 30 },
    deepCheck: false,
    ...over,
  };
}

export function checkView(over: Partial<CheckView> = {}): CheckView {
  return {
    directory: "/Volumes/Backup/Day01",
    checksumFiles: 3,
    files: 1284,
    bytes: 212_400_000_000,
    notChecked: 12,
    problems: [],
    ...over,
  };
}

export function mirrorPreview(over: Partial<MirrorPreviewView> = {}): MirrorPreviewView {
  return {
    presetId: "m1",
    name: "Footage → NAS",
    origin: "/Volumes/SSD/Footage",
    destination: "/Volumes/Media/Footage",
    newFiles: 2,
    newBytes: 4_000_000_000,
    changedFiles: 1,
    changedBytes: 2_000_000_000,
    removedFiles: 1,
    archiveDays: 30,
    failing: 0,
    unchanged: 100,
    guard: null,
    ...over,
  };
}

export function queueView(over: Partial<QueueView> = {}): QueueView {
  return { jobs: [], onFailure: "continue", ...over };
}

/** Every method is a spy; `session` is what the session commands answer. */
/** A .secopy file's Import screen: a clash, a path not connected, a preset that can't come in. */
export function importView(over: Partial<ImportView> = {}): ImportView {
  return {
    fileName: "Team presets.secopy",
    settings: { changes: [raw("Write the checksum file: on → off")], problem: null },
    copyPresets: [
      { name: "Sony FX3", paths: ["/Volumes/CARD_A/CLIP"], clash: "Sony FX3", newName: "Sony FX3 (2)", missing: [], problem: null, section: false },
      { name: "DJI", paths: ["/Volumes/DJI/DCIM"], clash: null, newName: "DJI", missing: ["/Volumes/DJI/DCIM"], problem: null, section: false },
      { name: "Bad", paths: [], clash: null, newName: "Bad", missing: [], problem: raw("Its details can't be read (…)."), section: false },
    ],
    mirrorPresets: [],
    ...over,
  };
}

export function fakeApi(session: SessionView = sessionView()) {
  const state = {
    session,
    progress: null as ((p: ProgressView) => void) | null,
    drop: null as ((paths: string[], target: Element | null) => void) | null,
    close: null as ((prevent: () => void) => Promise<void>) | null,
    start: startView({ session }),
    openSettings: null as (() => void) | null,
    menu: null as ((item: string) => void) | null,
    /** The handler `onPanelView` was given: the menu bar panel's updates. */
    panelView: null as ((view: PanelView) => void) | null,
    /** The handler `onOpenFile` was given: a .secopy file opened from Finder. */
    openFile: null as (() => void) | null,
    queue: queueView(),
    /** The handler `runQueue` was given: the test sends queue events through it. */
    queueEvent: null as ((e: QueueEvent) => void) | null,
  };
  const queueAnswer = () => Promise.resolve(state.queue);
  const answer = () => Promise.resolve(state.session);
  const presetsAnswer = () =>
    Promise.resolve({ presets: state.start.copyPresets, session: state.session } as CopyPresetsView);
  const api = {
    scanSource: vi.fn(answer),
    setIncludeFolder: vi.fn((_include: boolean) => answer()),
    clearSource: vi.fn(answer),
    sessionView: vi.fn(answer),
    setFilter: vi.fn(answer),
    setDestination: vi.fn(answer),
    setConflicts: vi.fn(answer),
    startJob: vi.fn((_verify: boolean, onProgress: (p: ProgressView) => void) => {
      state.progress = onProgress;
      return Promise.resolve(null);
    }),
    pauseJob: vi.fn(() => Promise.resolve()),
    resumeJob: vi.fn(() => Promise.resolve()),
    menubarView: vi.fn(() => Promise.resolve(null as PanelView | null)),
    onPanelView: vi.fn((handler: (view: PanelView) => void) => {
      state.panelView = handler;
      return Promise.resolve(() => {});
    }),
    openMainWindow: vi.fn(() => Promise.resolve()),
    quitApp: vi.fn(() => Promise.resolve()),
    cancelJob: vi.fn((_removeCopied: boolean) => Promise.resolve()),
    jobRunning: vi.fn(() => Promise.resolve(false)),
    finishedPage: vi.fn((_o: number, _l: number, _f: boolean) => Promise.resolve([] as Awaited<ReturnType<Api["finishedPage"]>>)),
    jobSummary: vi.fn(() => Promise.resolve(summaryView() as SummaryView | null)),
    saveReport: vi.fn((_p: string) => Promise.resolve(null)),
    retryFailed: vi.fn(answer),
    queue: vi.fn(queueAnswer),
    addToQueue: vi.fn((_verify: boolean) => queueAnswer()),
    removeFromQueue: vi.fn((_index: number) => queueAnswer()),
    moveInQueue: vi.fn((_from: number, _to: number) => queueAnswer()),
    clearQueue: vi.fn(queueAnswer),
    setQueueOnFailure: vi.fn((_onFailure: OnFailure) => queueAnswer()),
    runQueue: vi.fn((onEvent: (e: QueueEvent) => void) => {
      state.queueEvent = onEvent;
      return Promise.resolve(null);
    }),
    queueFinishedPage: vi.fn((_i: number, _o: number, _l: number, _f: boolean) => Promise.resolve([] as FinishedRow[])),
    queueSaveReport: vi.fn((_i: number, _p: string) => Promise.resolve(null)),
    mirrorPresets: vi.fn(() => Promise.resolve([mirrorPreset()])),
    createMirrorPreset: vi.fn((_input: MirrorPresetInput) => Promise.resolve([mirrorPreset()])),
    editMirrorPreset: vi.fn((_id: string, _input: MirrorPresetInput) => Promise.resolve([mirrorPreset()])),
    deleteMirrorPreset: vi.fn((_id: string) => Promise.resolve([] as MirrorPreset[])),
    mirrorArchive: vi.fn((_id: string) =>
      Promise.resolve({ files: 0, bytes: 0, oldest: null, connected: true, busy: false } as ArchiveView),
    ),
    deleteMirrorArchive: vi.fn((_id: string) => Promise.resolve({ removed: 0, notDeleted: null } as ArchiveDeletedView)),
    clearMirrorArchiveNextRun: vi.fn((_id: string) => Promise.resolve([mirrorPreset()])),
    previewMirror: vi.fn((_id: string, _onCompared: (c: ComparedView) => void) => Promise.resolve(mirrorPreview())),
    cancelMirrorPreview: vi.fn(() => Promise.resolve()),
    mirrorPreviewPage: vi.fn((_kind: PreviewKind | null, _o: number, _l: number) => Promise.resolve([] as PreviewRow[])),
    runMirror: vi.fn((_id: string, onProgress: (p: ProgressView) => void) => {
      state.progress = onProgress;
      return Promise.resolve(null);
    }),
    addMirrorToQueue: vi.fn((_id: string) => queueAnswer()),
    checkDirectory: vi.fn((_path: string) => Promise.resolve(checkView())),
    startCheck: vi.fn((_path: string, onProgress: (p: ProgressView) => void) => {
      state.progress = onProgress;
      return Promise.resolve(null);
    }),
    addCheckToQueue: vi.fn((_path: string) => queueAnswer()),
    appStart: vi.fn(() => Promise.resolve(state.start)),
    recentDestinations: vi.fn(() => Promise.resolve(state.start.recentDestinations)),
    selectCopyPreset: vi.fn((_id: string | null) => answer()),
    updateCopyPreset: vi.fn(presetsAnswer),
    saveCopyPresetAs: vi.fn((_name: string) => presetsAnswer()),
    createCopyPreset: vi.fn((_input: CopyPresetInput) => Promise.resolve(state.start.copyPresets)),
    editCopyPreset: vi.fn((_id: string, _input: CopyPresetInput) => presetsAnswer()),
    deleteCopyPreset: vi.fn((_id: string) => presetsAnswer()),
    setSettings: vi.fn((s: Settings) => Promise.resolve(s)),
    setMode: vi.fn((_verify: boolean) => Promise.resolve(null)),
    onOpenSettings: vi.fn((handler: () => void) => {
      state.openSettings = handler;
      return Promise.resolve(() => {});
    }),
    onMenu: vi.fn((handler: (item: string) => void) => {
      state.menu = handler;
      return Promise.resolve(() => {});
    }),
    onOpenFile: vi.fn((handler: () => void) => {
      state.openFile = handler;
      return Promise.resolve(() => {});
    }),
    takeOpenedFile: vi.fn(() => Promise.resolve(null as string | null)),
    hideToMenuBar: vi.fn(() => Promise.resolve(false)),
    pickImportFile: vi.fn(() => Promise.resolve(null as string | null)),
    openImport: vi.fn((_p: string) => Promise.resolve(importView())),
    applyImport: vi.fn((_c: ImportChoices) =>
      Promise.resolve({ message: raw("Imported."), failed: false, settings: settingsView(), copyPresets: [], mirrorPresets: [] } as ImportDone),
    ),
    setMenuState: vi.fn((_setup: boolean, _canStart: boolean, _copying: boolean, _busy: boolean) => Promise.resolve()),
    pickSource: vi.fn(() => Promise.resolve(["/Volumes/CARD/DCIM"] as string[] | null)),
    pickDirectory: vi.fn(() => Promise.resolve("/Volumes/CARD_A/PRIVATE/M4ROOT/CLIP" as string | null)),
    pickDestination: vi.fn(() => Promise.resolve("/Volumes/RAID/Day01" as string | null)),
    pickReportPath: vi.fn((_s: string) => Promise.resolve("/tmp/report.txt" as string | null)),
    pickExportPath: vi.fn((_s: string) => Promise.resolve(null as string | null)),
    exportAll: vi.fn((_p: string, _w: unknown) => Promise.resolve("Exported.")),
    exportCopyPreset: vi.fn((_id: string, _p: string) => Promise.resolve("Exported.")),
    exportMirrorPreset: vi.fn((_id: string, _p: string) => Promise.resolve("Exported.")),
    confirm: vi.fn((_m: string, _t: string, _ok?: string, _cancel?: string) => Promise.resolve(true)),
    reveal: vi.fn((_p: string) => Promise.resolve()),
    windowFocused: vi.fn(() => true),
    notify: vi.fn((_t: string, _b: string) => Promise.resolve()),
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

/** A message the UI shows as it is, for fixtures that stand for what the app sends. */
export function raw(text: string): Message {
  return { key: "format.raw", args: { text } };
}
