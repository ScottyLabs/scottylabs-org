// Accepting files dragged from the desktop onto an element. Used as an attachment, which keeps the drop handlers off the markup: the element is only a target, and every drop zone also has a button that opens the file picker, so keyboard and screen reader users never need to drag.
//   <div {@attach fileDrop({ onfiles, onactive: (on) => (active = on) })}>

import type { Attachment } from "svelte/attachments";

export interface FileDropOptions {
  onfiles: (files: File[]) => void;
  /** True while files are dragged over the element. */
  onactive?: (active: boolean) => void;
  disabled?: boolean;
}

function carriesFiles(event: DragEvent): boolean {
  return event.dataTransfer?.types.includes("Files") ?? false;
}

export function fileDrop(options: FileDropOptions): Attachment<HTMLElement> {
  const accepts = (event: DragEvent) => options.disabled !== true && carriesFiles(event);
  // dragenter and dragleave fire for every child, so count them.
  let depth = 0;

  const onDragEnter = (event: DragEvent) => {
    if (!accepts(event)) return;
    event.preventDefault();
    depth += 1;
    options.onactive?.(true);
  };
  const onDragOver = (event: DragEvent) => {
    if (!accepts(event)) return;
    event.preventDefault();
    if (event.dataTransfer !== null) event.dataTransfer.dropEffect = "copy";
  };
  const onDragLeave = (event: DragEvent) => {
    if (!carriesFiles(event)) return;
    depth = Math.max(0, depth - 1);
    if (depth === 0) options.onactive?.(false);
  };
  const onDrop = (event: DragEvent) => {
    if (!accepts(event)) return;
    event.preventDefault();
    depth = 0;
    options.onactive?.(false);
    const files = Array.from(event.dataTransfer?.files ?? []);
    if (files.length > 0) options.onfiles(files);
  };

  return (node) => {
    node.addEventListener("dragenter", onDragEnter);
    node.addEventListener("dragover", onDragOver);
    node.addEventListener("dragleave", onDragLeave);
    node.addEventListener("drop", onDrop);
    return () => {
      node.removeEventListener("dragenter", onDragEnter);
      node.removeEventListener("dragover", onDragOver);
      node.removeEventListener("dragleave", onDragLeave);
      node.removeEventListener("drop", onDrop);
    };
  };
}
