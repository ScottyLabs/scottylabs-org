// Where the floating format bar sits: centered above the selection, or below it when there is no room above, and always inside the viewport.

export interface Box {
  top: number;
  left: number;
  width: number;
  height: number;
}

export interface FloatingPosition {
  top: number;
  left: number;
  placement: "above" | "below";
}

const GAP = 8;
const MARGIN = 8;

export function floatingPosition(
  anchor: Box,
  floating: { width: number; height: number },
  viewport: { width: number; height: number },
): FloatingPosition {
  const above = anchor.top - GAP - floating.height;
  const placement = above >= MARGIN ? "above" : "below";
  const top = placement === "above" ? above : anchor.top + anchor.height + GAP;
  const centered = anchor.left + anchor.width / 2 - floating.width / 2;
  const maxLeft = Math.max(MARGIN, viewport.width - floating.width - MARGIN);
  return {
    top: Math.round(top),
    left: Math.round(Math.min(Math.max(centered, MARGIN), maxLeft)),
    placement,
  };
}
