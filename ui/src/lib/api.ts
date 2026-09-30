// Everything the UI asks of the app, in one place. Components take it from the Svelte
// context (`useApi`), so tests can pass a fake instead of talking to Tauri.
import { t } from "./i18n";

import { Channel } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { getContext, setContext } from "svelte";
import { ask } from "./confirm.svelte";
import { dropPoint, isMacOS } from "./drop";
import {
  commands,
  type ConflictPolicy,
  type FinishedRow,
  type CheckView,
  type ComparedView,
  type CopyPreset,
  type CopyPresetInput,
  type CopyPresetsView,
  type ExportWhat,
  type ImportChoices,
  type ImportDone,
  type ImportView,
  type PanelView,
  type MirrorPreset,
  type MirrorPresetInput,
  type ArchiveView,
  type ArchiveDeletedView,
  type MirrorPreviewView,
  type OnFailure,
  type PreviewKind,
  type PreviewRow,
  type ProgressView,
  type QueueEvent,
  type QueueView,
  type SessionView,
  type Settings,
  type StartView,
  type SummaryView,
  type Message,
} from "./bindings";
import { AppError, say } from "./message";

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: Message };

/** Turns a command's error result into a thrown `AppError`: the app's message, in words. */
export async function unwrap<T>(result: Promise<Result<T>>): Promise<T> {
  const r = await result;
  if (r.status === "error") throw new AppError(r.error);
  return r.data;
}

function asList(picked: string | string[] | null): string[] | null {
  if (picked === null) return null;
  return Array.isArray(picked) ? picked : [picked];
}

export const tauriApi = {
  scanSource: (paths: string[]): Promise<SessionView> => unwrap(commands.scanSource(paths)),
  setIncludeFolder: (include: boolean): Promise<SessionView> =>
    unwrap(commands.setIncludeFolder(include)),
  clearSource: (): Promise<SessionView> => unwrap(commands.clearSource()),
  setFilter: (selected: (string | null)[] | null): Promise<SessionView> =>
    unwrap(commands.setFilter(selected)),
  setDestination: (path: string | null): Promise<SessionView> =>
    unwrap(commands.setDestination(path)),
  setConflicts: (policy: ConflictPolicy): Promise<SessionView> =>
    unwrap(commands.setConflicts(policy)),
  sessionView: (): Promise<SessionView> => unwrap(commands.sessionView()),

  startJob: (verify: boolean, onProgress: (p: ProgressView) => void): Promise<null> => {
    const channel = new Channel<ProgressView>();
    channel.onmessage = onProgress;
    return unwrap(commands.startJob(verify, channel));
  },
  pauseJob: (): Promise<void> => commands.pauseJob(),
  /** The menu bar panel (#80): what it shows now, its updates, and its buttons. */
  menubarView: (): Promise<PanelView | null> => commands.menubarView(),
  onPanelView: (cb: (view: PanelView) => void): Promise<() => void> =>
    listen<PanelView>("menubar-view", (event) => cb(event.payload)),
  openMainWindow: (): Promise<void> => commands.openMainWindow(),
  quitApp: (): Promise<void> => commands.quitApp(),
  resumeJob: (): Promise<void> => commands.resumeJob(),
  /** Stops the job; `removeCopied` also removes the files it already copied (#54). */
  cancelJob: (removeCopied: boolean): Promise<void> => commands.cancelJob(removeCopied),
  jobRunning: (): Promise<boolean> => commands.jobRunning(),
  finishedPage: (offset: number, limit: number, failedOnly: boolean): Promise<FinishedRow[]> =>
    unwrap(commands.finishedPage(offset, limit, failedOnly)),
  jobSummary: (): Promise<SummaryView | null> => unwrap(commands.jobSummary()),
  saveReport: (path: string): Promise<null> => unwrap(commands.saveReport(path)),
  retryFailed: (): Promise<SessionView> => unwrap(commands.retryFailed()),
  queue: (): Promise<QueueView> => unwrap(commands.queue()),
  addToQueue: (verify: boolean): Promise<QueueView> => unwrap(commands.addToQueue(verify)),
  removeFromQueue: (index: number): Promise<QueueView> => unwrap(commands.removeFromQueue(index)),
  moveInQueue: (from: number, to: number): Promise<QueueView> => unwrap(commands.moveInQueue(from, to)),
  clearQueue: (): Promise<QueueView> => unwrap(commands.clearQueue()),
  setQueueOnFailure: (onFailure: OnFailure): Promise<QueueView> => unwrap(commands.setQueueOnFailure(onFailure)),
  runQueue: (onEvent: (e: QueueEvent) => void): Promise<null> => {
    const channel = new Channel<QueueEvent>();
    channel.onmessage = onEvent;
    return unwrap(commands.runQueue(channel));
  },
  queueFinishedPage: (index: number, offset: number, limit: number, failedOnly: boolean): Promise<FinishedRow[]> =>
    unwrap(commands.queueFinishedPage(index, offset, limit, failedOnly)),
  queueSaveReport: (index: number, path: string): Promise<null> => unwrap(commands.queueSaveReport(index, path)),
  mirrorPresets: (): Promise<MirrorPreset[]> => unwrap(commands.mirrorPresets()),
  createMirrorPreset: (input: MirrorPresetInput): Promise<MirrorPreset[]> =>
    unwrap(commands.createMirrorPreset(input)),
  editMirrorPreset: (id: string, input: MirrorPresetInput): Promise<MirrorPreset[]> =>
    unwrap(commands.editMirrorPreset(id, input)),
  deleteMirrorPreset: (id: string): Promise<MirrorPreset[]> => unwrap(commands.deleteMirrorPreset(id)),
  /** What a mirror's archive holds (#101), asked before switching it to Delete. */
  mirrorArchive: (id: string): Promise<ArchiveView> => unwrap(commands.mirrorArchive(id)),
  /** `destination`: the one whose archive was shown; the app refuses if the mirror's changed. */
  deleteMirrorArchive: (id: string, destination: string): Promise<ArchiveDeletedView> =>
    unwrap(commands.deleteMirrorArchive(id, destination)),
  clearMirrorArchiveNextRun: (id: string): Promise<MirrorPreset[]> => unwrap(commands.clearMirrorArchiveNextRun(id)),
  /** What a preset would do now; the preview's Start then runs exactly this (FR-47). */
  previewMirror: (id: string, onCompared: (c: ComparedView) => void): Promise<MirrorPreviewView> => {
    const channel = new Channel<ComparedView>();
    channel.onmessage = onCompared;
    return unwrap(commands.previewMirror(id, channel));
  },
  /** Stops a preview's deep check; the preview then fails with `errors.mirror.previewCancelled`. */
  cancelMirrorPreview: (): Promise<void> => commands.cancelMirrorPreview(),
  mirrorPreviewPage: (kind: PreviewKind | null, offset: number, limit: number): Promise<PreviewRow[]> =>
    unwrap(commands.mirrorPreviewPage(kind, offset, limit)),
  /** Runs preset `id`'s preview, once; a preset changed since fails. */
  runMirror: (id: string, onProgress: (p: ProgressView) => void): Promise<null> => {
    const channel = new Channel<ProgressView>();
    channel.onmessage = onProgress;
    return unwrap(commands.runMirror(id, channel));
  },
  addMirrorToQueue: (id: string): Promise<QueueView> => unwrap(commands.addMirrorToQueue(id)),
  /** Verify's Choose…: what the directory's checksum files list (plan 8). */
  checkDirectory: (path: string): Promise<CheckView> => unwrap(commands.checkDirectory(path)),
  /** Verify's Start: checks the directory chosen last, once. */
  startCheck: (path: string, onProgress: (p: ProgressView) => void): Promise<null> => {
    const channel = new Channel<ProgressView>();
    channel.onmessage = onProgress;
    return unwrap(commands.startCheck(path, channel));
  },
  addCheckToQueue: (path: string): Promise<QueueView> => unwrap(commands.addCheckToQueue(path)),
  appStart: (): Promise<StartView> => unwrap(commands.appStart()),
  recentDestinations: (): Promise<string[]> => unwrap(commands.recentDestinations()),
  selectCopyPreset: (id: string | null): Promise<SessionView> => unwrap(commands.selectCopyPreset(id)),
  updateCopyPreset: (): Promise<CopyPresetsView> => unwrap(commands.updateCopyPreset()),
  saveCopyPresetAs: (name: string): Promise<CopyPresetsView> => unwrap(commands.saveCopyPresetAs(name)),
  createCopyPreset: (input: CopyPresetInput): Promise<CopyPreset[]> => unwrap(commands.createCopyPreset(input)),
  editCopyPreset: (id: string, input: CopyPresetInput): Promise<CopyPresetsView> =>
    unwrap(commands.editCopyPreset(id, input)),
  deleteCopyPreset: (id: string): Promise<CopyPresetsView> => unwrap(commands.deleteCopyPreset(id)),
  setSettings: (settings: Settings): Promise<Settings> => unwrap(commands.setSettings(settings)),
  setMode: (verify: boolean): Promise<null> => unwrap(commands.setMode(verify)),
  /** Secopy → Settings… (⌘,). */
  onOpenSettings: (handler: () => void): Promise<() => void> => listen("open-settings", handler),
  /** Menu items the window handles: "choose-source", "choose-destination", "start-copy",
   * "cancel-copy", "export-file", "import-file" (File), "show-copy", "show-mirror",
   * "show-verify", "show-queue" (View). */
  onMenu: (handler: (item: string) => void): Promise<() => void> => listen<string>("menu", (e) => handler(e.payload)),
  /** Which File menu items apply. */
  setMenuState: (setup: boolean, canStart: boolean, copying: boolean, busy: boolean): Promise<void> =>
    commands.setMenuState(setup, canStart, copying, busy),

  /** FROM's Choose…: a folder or files, in one panel. */
  pickSource: (): Promise<string[] | null> => unwrap(commands.pickSource()),
  /** One directory: a copy preset's source, a mirror's origin or destination. */
  pickDirectory: async (title = t("dialog.chooseDirectory")): Promise<string | null> =>
    asList(await open({ directory: true, multiple: false, title }))?.[0] ?? null,
  pickDestination: async (): Promise<string | null> =>
    asList(await open({ directory: true, multiple: false, title: t("dialog.copyTo") }))?.[0] ?? null,
  /** What was exported, in words ("Exported 2 copy presets."). */
  exportAll: (path: string, what: ExportWhat): Promise<string> => unwrap(commands.exportAll(path, what)).then(say),
  exportCopyPreset: (id: string, path: string): Promise<string> =>
    unwrap(commands.exportCopyPreset(id, path)).then(say),
  exportMirrorPreset: (id: string, path: string): Promise<string> =>
    unwrap(commands.exportMirrorPreset(id, path)).then(say),
  /** A .secopy file to import. */
  pickImportFile: async (): Promise<string | null> =>
    asList(
      await open({ multiple: false, directory: false, filters: [{ name: t("dialog.secopyFiles"), extensions: ["secopy"] }] }),
    )?.[0] ?? null,
  openImport: (path: string): Promise<ImportView> => unwrap(commands.openImport(path)),
  applyImport: (choices: ImportChoices): Promise<ImportDone> => unwrap(commands.applyImport(choices)),
  /** A .secopy file opened from Finder, once. */
  takeOpenedFile: (): Promise<string | null> => commands.takeOpenedFile(),
  onOpenFile: (cb: () => void): Promise<() => void> => listen("open-file", () => cb()),
  /** Where to save a .secopy file. */
  pickExportPath: (suggested: string): Promise<string | null> =>
    save({ defaultPath: suggested, filters: [{ name: t("dialog.secopyFiles"), extensions: ["secopy"] }] }),
  pickReportPath: (suggested: string): Promise<string | null> =>
    save({ defaultPath: suggested, filters: [{ name: t("dialog.textFiles"), extensions: ["txt"] }] }),
  confirm: (message: string, title: string, ok: string, cancel: string): Promise<boolean> =>
    ask(message, title, ok, cancel),
  reveal: (path: string): Promise<void> => revealItemInDir(path),
  /** The window is in front; notifications are only for when it isn't. */
  windowFocused: (): boolean => document.hasFocus(),
  /** A macOS notification. Asks for permission once; does nothing if it's refused. */
  notify: async (title: string, body: string): Promise<void> => {
    let granted = await isPermissionGranted();
    if (!granted) granted = (await requestPermission()) === "granted";
    if (granted) sendNotification({ title, body });
  },
  openFile: (path: string): Promise<void> => openPath(path),

  /** Finder drops: the paths and the element under the pointer. */
  onDrop: (handler: (paths: string[], target: Element | null) => void): Promise<() => void> =>
    getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type !== "drop") return;
      const { x, y } = dropPoint(event.payload.position, window.devicePixelRatio, isMacOS());
      handler(event.payload.paths, document.elementFromPoint(x, y));
    }),
  /** The window is closing: Rust hides it behind the menu bar icon when that applies (#80). */
  hideToMenuBar: (): Promise<boolean> => commands.hideToMenuBar(),
  /** Closing the window; call `prevent()` to keep it open. */
  onCloseRequested: (handler: (prevent: () => void) => Promise<void>): Promise<() => void> =>
    getCurrentWindow().onCloseRequested((event) => handler(() => event.preventDefault())),
};

export type Api = typeof tauriApi;

const KEY = Symbol("api");

export function provideApi(api: Api): void {
  setContext(KEY, api);
}

export function useApi(): Api {
  return getContext<Api>(KEY);
}

/** The context to render a component with a given `Api` (tests). */
export function apiContext(api: Api): Map<symbol, Api> {
  return new Map([[KEY, api]]);
}
