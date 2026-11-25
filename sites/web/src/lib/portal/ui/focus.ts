// Where keyboard focus goes when the element that had it is gone, so keyboard and screen reader users carry on from the page heading instead of the top of the document.

/** Enough of an element to decide where focus goes; tests pass stand-ins. */
export interface FocusCandidate {
  readonly isConnected: boolean;
}

/**
 * Where a closing dialog sends focus: back to the element that had it when the dialog opened,
 * while that is still on the page. A dialog opened on arrival saved the body, and one opened by a
 * control that went away saved a removed element, so then focus goes to the page heading, but only
 * when focus is lost: a page that moved focus somewhere itself keeps it there.
 */
export function focusAfterDialog(
  opener: FocusCandidate | null,
  body: FocusCandidate,
  focusLost: boolean,
): "opener" | "heading" | null {
  if (opener !== null && opener !== body && opener.isConnected) return "opener";
  return focusLost ? "heading" : null;
}

/** True when focus is on the body, on a removed element, or inside `closed`, which is closing. */
export function focusIsLost(closed?: Element): boolean {
  const active = document.activeElement;
  if (active === null || active === document.body || !active.isConnected) return true;
  return closed?.contains(active) === true;
}

/** Focuses the portal page's heading, or its main area when it has none. */
export function focusPageHeading(options?: FocusOptions): void {
  const main = document.querySelector<HTMLElement>("#portal-main");
  const target = main?.querySelector<HTMLElement>("h1") ?? main;
  if (target === null || target === undefined) return;
  if (!target.hasAttribute("tabindex")) target.setAttribute("tabindex", "-1");
  target.focus(options);
}
