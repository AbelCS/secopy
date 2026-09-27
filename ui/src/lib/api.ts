// Everything the UI asks of the app, in one place. Components take it from the Svelte
// context (`useApi`), so tests can pass a fake instead of talking to Tauri.

import { Channel } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { getContext, setContext } from "svelte";
import {
  commands,
  type ConflictPolicy,
  type FinishedRow,
  type ProgressView,
  type SessionView,
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

  /** FROM's Choose…: a folder or files, in one panel. */
  pickSource: (): Promise<string[] | null> => unwrap(commands.pickSource()),
  pickDestination: async (): Promise<string | null> =>
    asList(await open({ directory: true, multiple: false, title: "Copy to" }))?.[0] ?? null,
  pickReportPath: (suggested: string): Promise<string | null> =>
    save({ defaultPath: suggested, filters: [{ name: "Text", extensions: ["txt"] }] }),
  confirm: (message: string, title: string): Promise<boolean> =>
    ask(message, { title, kind: "warning", okLabel: "Stop copying", cancelLabel: "Keep copying" }),
  reveal: (path: string): Promise<void> => revealItemInDir(path),
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
