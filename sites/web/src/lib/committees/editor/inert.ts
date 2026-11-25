// The editor covers the portal shell, so the shell leaves the tab order and the accessibility tree while the editor is open. Everything beside the editor's ancestors goes inert, up to the app's root element, except live regions such as the portal's notifications. The editor's own dialogs, which sit beside it inside the same parent, stay usable. Nothing outside the app is touched.
//   <div {@attach inertOutside}>...</div>

import type { Attachment } from "svelte/attachments";

/** Live regions keep announcing, so saves and errors are still heard. */
const KEEP = ".p-toasts, [aria-live]";

export const inertOutside: Attachment<HTMLElement> = (element) => {
  const made: HTMLElement[] = [];
  for (let node = element.parentElement; node !== null; node = node.parentElement) {
    const parent = node.parentElement;
    if (parent === null || parent === document.body) break;
    for (const sibling of parent.children) {
      if (sibling === node || !(sibling instanceof HTMLElement)) continue;
      if (sibling.inert || sibling.matches(KEEP)) continue;
      sibling.inert = true;
      made.push(sibling);
    }
  }
  return () => {
    for (const sibling of made) sibling.inert = false;
  };
};
