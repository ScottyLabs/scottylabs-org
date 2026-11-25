// URLs and sizes for ImageRef. Uploaded images (`/media/<uuid>`) come in widths of 480, 960 and
// 1920 pixels, never wider than the original, which is also kept up to 1920 pixels wide. Photos bundled with the site (`/seed/<path>.jpg`) also come smaller, as `<path>-480.jpg` and `<path>-960.jpg` where that is at most 80% of their width (scripts/seed-sizes.ts writes them).

import type { ImageRef } from "../api/types.ts";

/** The widths the site service makes for every upload. */
export const MEDIA_WIDTHS = [480, 960, 1920] as const;

/** The widest variant the service keeps. */
export const MAX_MEDIA_WIDTH = 1920;

const UPLOADED = /^\/media\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu;
const SEED_PHOTO = /^\/seed\/[\w/-]+\.jpg$/u;

/** The smaller widths bundled photos come in. */
export const SEED_WIDTHS = [480, 960] as const;

/** True for a photo bundled with the site that has smaller copies. */
export function isSeedPhoto(path: string): boolean {
  return SEED_PHOTO.test(path) && !path.includes("//");
}

/**
 * The widths a bundled photo exists in, narrowest first, ending with its own; just its own width
 * when it has no smaller copies. A copy is made only where it is at most 80% as wide, so a
 * 1000 px photo has a 480 px copy and a 1440 px one has 480 and 960 px copies.
 */
export function seedWidths(path: string, width: number | null | undefined): number[] {
  if (width === null || width === undefined || width <= 0) return [];
  if (!isSeedPhoto(path)) return [width];
  return [...SEED_WIDTHS.filter((candidate) => candidate <= width * 0.8), width];
}

/** "/seed/photos/c10-480.jpg" for the 480 px copy of /seed/photos/c10.jpg. */
export function seedVariantPath(path: string, width: number): string {
  return path.replace(/\.jpg$/u, `-${width.toString()}.jpg`);
}

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
  if (isSeedPhoto(image.src)) {
    const widths = seedWidths(image.src, image.width);
    const chosen = widths.find((candidate) => candidate >= width) ?? widths.at(-1);
    return chosen === undefined || chosen === widths.at(-1)
      ? image.src
      : seedVariantPath(image.src, chosen);
  }
  if (!isUploaded(image.src)) return image.src;
  const widths = storedWidths(image);
  const chosen = widths.find((candidate) => candidate >= width) ?? widths.at(-1) ?? MAX_MEDIA_WIDTH;
  return `${image.src}?w=${chosen}`;
}

/** A `srcset` with every stored width of an upload or bundled photo, or undefined when there is one size. */
export function srcset(image: SizedImage): string | undefined {
  if (isSeedPhoto(image.src)) {
    const widths = seedWidths(image.src, image.width);
    if (widths.length < 2) return undefined;
    const last = widths.length - 1;
    return widths
      .map(
        (width, index) =>
          `${index === last ? image.src : seedVariantPath(image.src, width)} ${width.toString()}w`,
      )
      .join(", ");
  }
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
