// Reordering a vertical list by dragging a handle, with a mouse, a pen or a finger. The list moves as the pointer passes the middle of the next item. Items carry `data-sortable-item`.
//   <li data-sortable-item><span {@attach sortableHandle({ index: () => i, onmove })}>...</span></li>
// Moving an item re-inserts its element, which ends pointer capture, so the drag listens on the window instead.

export interface SortableOptions {
  /** The item's current position. */
  index: () => number;
  /** Move the item at `from` to `to`; the list re-renders with it there. */
  onmove: (from: number, to: number) => void;
  onstart?: () => void;
  /** Called when the pointer lets go. `moved` is false when the item ended where it started, such as after a click on the handle. */
  onend?: (moved: boolean) => void;
}

function middle(element: Element): number {
  const rect = element.getBoundingClientRect();
  return rect.top + rect.height / 2;
}

/** An attachment for the handle. Typed as a plain function so tests can load it without Svelte. */
export function sortableHandle(options: SortableOptions): (handle: HTMLElement) => () => void {
  return (handle) => {
    let current = -1;
    let started = -1;
    let pointer: number | null = null;
    let list: HTMLElement | null = null;

    function onmove(event: PointerEvent) {
      if (pointer !== event.pointerId || list === null) return;
      event.preventDefault();
      const all = [...list.querySelectorAll(":scope > [data-sortable-item]")];
      const above = all[current - 1];
      const below = all[current + 1];
      let to = current;
      if (above !== undefined && event.clientY < middle(above)) to = current - 1;
      else if (below !== undefined && event.clientY > middle(below)) to = current + 1;
      if (to === current) return;
      options.onmove(current, to);
      current = to;
    }

    function stop(event: PointerEvent) {
      if (pointer !== event.pointerId) return;
      pointer = null;
      list = null;
      window.removeEventListener("pointermove", onmove);
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("pointercancel", stop);
      options.onend?.(current !== started);
    }

    function start(event: PointerEvent) {
      if (event.button !== 0 || pointer !== null) return;
      list = handle.closest("[data-sortable-item]")?.parentElement ?? null;
      if (list === null) return;
      event.preventDefault();
      pointer = event.pointerId;
      current = options.index();
      started = current;
      window.addEventListener("pointermove", onmove);
      window.addEventListener("pointerup", stop);
      window.addEventListener("pointercancel", stop);
      options.onstart?.();
    }

    handle.addEventListener("pointerdown", start);
    return () => {
      handle.removeEventListener("pointerdown", start);
      window.removeEventListener("pointermove", onmove);
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("pointercancel", stop);
    };
  };
}
