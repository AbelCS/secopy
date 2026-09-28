/** The explanation a `Hint` gives `el` (its term or mark): what VoiceOver reads after it. */
export function hintOf(el: HTMLElement): string | null {
  const id = el.closest("[aria-describedby]")?.getAttribute("aria-describedby");
  return id ? (document.getElementById(id)?.textContent?.trim() ?? null) : null;
}

/** What `el` shows, without the explanations its hints hold for hover. */
export function shown(el: Element): string {
  const copy = el.cloneNode(true) as Element;
  copy.querySelectorAll("[role='tooltip']").forEach((t) => t.remove());
  return (copy.textContent ?? "").replace(/\s+/g, " ").trim();
}

/** Finds the element that shows exactly `text` once hints are left out. */
export const showing = (text: string) => (_: string, el: Element | null) =>
  !!el && shown(el) === text && [...el.children].every((c) => shown(c) !== text);

/** The help a `Button` gives (its tip): what VoiceOver reads after its name; null without one. */
export function helpOf(button: HTMLElement): string | null {
  const id = button.getAttribute("aria-describedby");
  return id ? (document.getElementById(id)?.textContent?.trim() ?? null) : null;
}
