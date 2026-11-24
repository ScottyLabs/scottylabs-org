// Stops the page behind a modal from scrolling, without the content shifting when the scrollbar goes away. Counted, so nested modals unlock only when the last one closes.

let locks = 0;
let saved: { overflow: string; paddingRight: string } | null = null;

export function lockScroll(): () => void {
  const root = document.documentElement;
  if (locks === 0) {
    const scrollbar = window.innerWidth - root.clientWidth;
    saved = { overflow: root.style.overflow, paddingRight: root.style.paddingRight };
    root.style.overflow = "hidden";
    if (scrollbar > 0) root.style.paddingRight = `${scrollbar}px`;
  }
  locks += 1;

  let released = false;
  return () => {
    if (released) return;
    released = true;
    locks -= 1;
    if (locks === 0 && saved !== null) {
      root.style.overflow = saved.overflow;
      root.style.paddingRight = saved.paddingRight;
      saved = null;
    }
  };
}
