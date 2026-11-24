// Where keyboard focus goes when the control that had it disappears during an upload: buttons unmount or hide as an upload starts, finishes or is cancelled, and focus must not fall back to the top of the page.

/** How an upload ended. */
export type UploadEnd = "done" | "failed" | "cancelled";

/**
 * In a one-photo field: a new photo needs alt text next, so focus goes there. Otherwise it goes
 * back to the button that picks a photo: Replace photo when one is shown, Choose a photo when not.
 */
export function focusAfterUpload(end: UploadEnd, hasPhoto: boolean): "alt" | "replace" | "choose" {
  if (end === "done") return "alt";
  return hasPhoto ? "replace" : "choose";
}

/** In a grid of tiles: the tile that took the place of one that went, or the last one left. */
export function tileAfter(position: number, tileCount: number): number | null {
  if (tileCount <= 0) return null;
  return Math.min(Math.max(position, 0), tileCount - 1);
}

/** True when keyboard focus is inside `element`. */
export function holdsFocus(element: Element | null | undefined): boolean {
  return element !== null && element !== undefined && element.contains(document.activeElement);
}

/**
 * True when focus sits nowhere useful: it fell back to the body because its element was
 * removed, or its element was hidden and the browser has not moved it yet.
 */
export function focusIsLost(): boolean {
  const active = document.activeElement;
  if (active === null || active === document.body) return true;
  return !active.isConnected || active.getClientRects().length === 0;
}
