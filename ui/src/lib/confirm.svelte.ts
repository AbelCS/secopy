// Questions before something that can't be undone (#113): shown in the window by
// `ConfirmHost`, the safe answer first and focused, so Return and Esc never do the risky thing.
// A system alert put the risky answer first, where Return pressed it.

export type Question = {
  message: string;
  title: string;
  /** The risky answer: "Delete", "Stop", "Discard". */
  ok: string;
  /** The safe answer. */
  cancel: string;
  answer: (yes: boolean) => void;
};

export const asking: { current: Question | null } = $state({ current: null });

/** Asks `message`; true only for the risky answer. A new question answers an open one no. */
export function ask(message: string, title: string, ok: string, cancel: string): Promise<boolean> {
  asking.current?.answer(false);
  return new Promise((resolve) => {
    asking.current = {
      message,
      title,
      ok,
      cancel,
      answer: (yes) => {
        asking.current = null;
        resolve(yes);
      },
    };
  });
}
