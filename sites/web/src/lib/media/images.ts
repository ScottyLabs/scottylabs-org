// URLs and sizes for ImageRef. Uploaded images (`/media/<uuid>`) come in widths of 480, 960 and
// 1920 pixels, never wider than the original, which is also kept up to 1920 pixels wide. Images bundled with the site (`/seed/...`) have one size and are used as they are.

import type { ImageRef } from "../api/types.ts";

/** The widths the site service makes for every upload. */
export const MEDIA_WIDTHS = [480, 960, 1920] as const;

/** The widest variant the service keeps. */
export const MAX_MEDIA_WIDTH = 1920;

const UPLOADED = /^\/media\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu;

type SizedImage = Pick<ImageRef, "src" | "width">;

/** True for images uploaded through the portal, which can be requested at several widths. */
export function isUploaded(path: string): boolean {
  return UPLOADED.test(path);
}

/** The widths stored for an upload: the standard ones below its width, plus its own width. */
export function storedWidths(image: SizedImage): number[] {
  const original = image.width;
  if (original === null || original <= 0) return [...MEDIA_WIDTHS];
  const capped = Math.min(original, MAX_MEDIA_WIDTH);
  return [...MEDIA_WIDTHS.filter((width) => width < capped), capped];
}

/**
 * The URL for showing an image at least `width` CSS pixels wide (times the device's pixel ratio
 * if you want sharpness). Uploads snap to a stored width so browsers and caches share files.
 */
export function src(image: SizedImage, width: number): string {
  if (!isUploaded(image.src)) return image.src;
  const widths = storedWidths(image);
  const chosen = widths.find((candidate) => candidate >= width) ?? widths.at(-1) ?? MAX_MEDIA_WIDTH;
  return `${image.src}?w=${chosen}`;
}

/** A `srcset` with every stored width of an upload, or undefined for bundled images. */
export function srcset(image: SizedImage): string | undefined {
  if (!isUploaded(image.src)) return undefined;
  return storedWidths(image)
    .map((width) => `${image.src}?w=${width} ${width}w`)
    .join(", ");
}

/** Width divided by height, or `fallback` when the size is unknown. */
export function aspectRatio(image: Pick<ImageRef, "width" | "height">, fallback = 4 / 3): number {
  const { width, height } = image;
  if (width === null || height === null || width <= 0 || height <= 0) return fallback;
  return width / height;
}
