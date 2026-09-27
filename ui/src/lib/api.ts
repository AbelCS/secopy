// Everything the UI asks of the app, in one place. Components take it from the Svelte
// context (`useApi`), so tests can pass a fake instead of talking to Tauri.

import { Channel } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { getContext, setContext } from "svelte";
import {
  commands,
  type ConflictPolicy,
  type DriveView,
  type FinishedRow,
  type Profile,
  type ProfileInput,
  type ProfilesView,
  type ProgressView,
  type SessionView,
  type Settings,
  type StartView,
  type SummaryView,
} from "./bindings";

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: string };

/** Turns a command's error result into a thrown `Error` with the app's message. */
export async function unwrap<T>(result: Promise<Result<T>>): Promise<T> {
  const r = await result;
  if (r.status === "error") throw new Error(r.error);
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
  resumeJob: (): Promise<void> => commands.resumeJob(),
  cancelJob: (): Promise<void> => commands.cancelJob(),
  jobRunning: (): Promise<boolean> => commands.jobRunning(),
  finishedPage: (offset: number, limit: number, failedOnly: boolean): Promise<FinishedRow[]> =>
    unwrap(commands.finishedPage(offset, limit, failedOnly)),
  jobSummary: (): Promise<SummaryView | null> => unwrap(commands.jobSummary()),
  saveReport: (path: string): Promise<null> => unwrap(commands.saveReport(path)),
  retryFailed: (): Promise<SessionView> => unwrap(commands.retryFailed()),
  eject: (mountPoint: string): Promise<null> => unwrap(commands.eject(mountPoint)),
  appStart: (): Promise<StartView> => unwrap(commands.appStart()),
  recentDestinations: (): Promise<string[]> => unwrap(commands.recentDestinations()),
  listDrives: (): Promise<DriveView[]> => unwrap(commands.listDrives()),
  selectProfile: (id: string | null): Promise<SessionView> => unwrap(commands.selectProfile(id)),
  updateProfile: (): Promise<ProfilesView> => unwrap(commands.updateProfile()),
  saveProfileAs: (name: string, folder: string): Promise<ProfilesView> =>
    unwrap(commands.saveProfileAs(name, folder)),
  createProfile: (input: ProfileInput): Promise<Profile[]> => unwrap(commands.createProfile(input)),
  editProfile: (id: string, input: ProfileInput): Promise<ProfilesView> =>
    unwrap(commands.editProfile(id, input)),
  deleteProfile: (id: string): Promise<ProfilesView> => unwrap(commands.deleteProfile(id)),
  setSettings: (settings: Settings): Promise<Settings> => unwrap(commands.setSettings(settings)),
  setMode: (verify: boolean): Promise<null> => unwrap(commands.setMode(verify)),
  /** Secopy → Settings… (⌘,). */
  onOpenSettings: (handler: () => void): Promise<() => void> => listen("open-settings", handler),
  /** File menu items (spec §3): "choose-source", "choose-destination", "start-copy", "cancel-copy". */
  onMenu: (handler: (item: string) => void): Promise<() => void> => listen<string>("menu", (e) => handler(e.payload)),
  /** Which File menu items apply. */
  setMenuState: (setup: boolean, canStart: boolean, copying: boolean): Promise<void> =>
    commands.setMenuState(setup, canStart, copying),

  /** FROM's Choose…: a folder or files, in one panel. */
  pickSource: (): Promise<string[] | null> => unwrap(commands.pickSource()),
  /** A folder on a card, for a profile's folder. */
  pickCardFolder: async (): Promise<string | null> =>
    asList(await open({ directory: true, multiple: false, title: "Directory on the card" }))?.[0] ?? null,
  pickDestination: async (): Promise<string | null> =>
    asList(await open({ directory: true, multiple: false, title: "Copy to" }))?.[0] ?? null,
  pickReportPath: (suggested: string): Promise<string | null> =>
    save({ defaultPath: suggested, filters: [{ name: "Text", extensions: ["txt"] }] }),
  confirm: (message: string, title: string, ok = "Stop copying", cancel = "Keep copying"): Promise<boolean> =>
    ask(message, { title, kind: "warning", okLabel: ok, cancelLabel: cancel }),
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
      const { x, y } = event.payload.position.toLogical(window.devicePixelRatio);
      handler(event.payload.paths, document.elementFromPoint(x, y));
    }),
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
