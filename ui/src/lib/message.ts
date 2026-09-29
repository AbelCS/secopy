// Texts from the app (#84): Rust sends a catalog key and its values (`Message`), and the UI
// says them in its language.
import type { Arg, Message } from "./bindings";
import { formatBytes } from "./format";
import { t, type Key, type Values } from "./i18n";

function value(arg: Arg): string | number {
  if (typeof arg === "number" || typeof arg === "string") return arg;
  if (Array.isArray(arg)) return arg.map(say).join(t("format.comma"));
  if ("bytes" in arg) return formatBytes(arg.bytes);
  return say(arg);
}

/** A message from the app, in words. */
export function say(m: Message): string {
  const values: Values = {};
  for (const [name, arg] of Object.entries(m.args)) if (arg !== undefined) values[name] = value(arg);
  return t(m.key as Key, values);
}

/** A command's failure: its words as the Error's message, and the message itself. */
export class AppError extends Error {
  readonly m: Message;
  constructor(m: Message) {
    super(say(m));
    this.name = "AppError";
    this.m = m;
  }
}

/** The form field an error is about ("errors.field.<field>.…"), or `null`. */
export function fieldOf(e: unknown): string | null {
  if (!(e instanceof AppError)) return null;
  return /^errors\.field\.(\w+)\./.exec(e.m.key)?.[1] ?? null;
}
