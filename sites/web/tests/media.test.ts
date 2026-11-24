/// <reference lib="dom" />
// Tests for image URLs, sizes and upload checks. Run from sites/web with `deno test tests/`.
// The DOM library reference types the browser code under test; the tests themselves never use it.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access */

import assert from "node:assert/strict";
import type { ImageRef } from "../src/lib/api/types.ts";
import { focusAfterUpload, tileAfter } from "../src/lib/media/focus.ts";
import { aspectRatio, isUploaded, src, srcset, storedWidths } from "../src/lib/media/images.ts";
import { checkImageFile, formatBytes } from "../src/lib/media/upload.ts";

const ID = "0193a5b0-7c1e-7d2a-9f00-1234567890ab";

function image(width: number | null, height: number | null, path = `/media/${ID}`): ImageRef {
  return { src: path, alt: "", width, height };
}

Deno.test("uploads are recognized by their path", () => {
  assert.ok(isUploaded(`/media/${ID}`));
  assert.ok(!isUploaded(`/media/${ID}?w=480`));
  assert.ok(!isUploaded("/seed/photos/c01.jpg"));
  assert.ok(!isUploaded("/media/not-a-uuid"));
  assert.ok(!isUploaded(`https://evil.example/media/${ID}`));
});

Deno.test("stored widths follow the service: standard widths below the original, plus the original", () => {
  assert.deepEqual(storedWidths(image(1920, 1080)), [480, 960, 1920]);
  assert.deepEqual(storedWidths(image(1200, 900)), [480, 960, 1200]);
  assert.deepEqual(storedWidths(image(400, 300)), [400]);
  assert.deepEqual(storedWidths(image(960, 720)), [480, 960]);
  assert.deepEqual(storedWidths(image(null, null)), [480, 960, 1920]);
});

Deno.test("srcset lists every stored width for uploads only", () => {
  assert.equal(
    srcset(image(1920, 1080)),
    `/media/${ID}?w=480 480w, /media/${ID}?w=960 960w, /media/${ID}?w=1920 1920w`,
  );
  assert.equal(
    srcset(image(1200, 900)),
    `/media/${ID}?w=480 480w, /media/${ID}?w=960 960w, /media/${ID}?w=1200 1200w`,
  );
  assert.equal(srcset(image(1600, 1200, "/seed/projects/maps.jpg")), undefined);
});

Deno.test("src picks the smallest stored width that is wide enough", () => {
  assert.equal(src(image(1920, 1080), 300), `/media/${ID}?w=480`);
  assert.equal(src(image(1920, 1080), 481), `/media/${ID}?w=960`);
  assert.equal(src(image(1920, 1080), 4000), `/media/${ID}?w=1920`);
  assert.equal(src(image(1200, 900), 1500), `/media/${ID}?w=1200`);
  assert.equal(src(image(null, null), 960), `/media/${ID}?w=960`);
  assert.equal(src(image(800, 600, "/seed/photos/c01.jpg"), 480), "/seed/photos/c01.jpg");
});

Deno.test("aspect ratio falls back when the size is unknown", () => {
  assert.equal(aspectRatio(image(1600, 900)), 16 / 9);
  assert.equal(aspectRatio(image(null, 900)), 4 / 3);
  assert.equal(aspectRatio(image(0, 0), 1), 1);
});

Deno.test("files are checked before uploading", () => {
  const MB = 1024 * 1024;
  assert.equal(checkImageFile({ name: "cover.jpg", type: "image/jpeg", size: 2 * MB }), null);
  assert.equal(checkImageFile({ name: "cover.webp", type: "image/webp", size: 5 * MB }), null);
  assert.equal(checkImageFile({ name: "photo.PNG", type: "", size: MB }), null);
  assert.match(
    checkImageFile({ name: "anim.gif", type: "image/gif", size: MB }) ?? "",
    /not a JPG, PNG or WebP/u,
  );
  assert.match(
    checkImageFile({ name: "logo.svg", type: "image/svg+xml", size: 10 }) ?? "",
    /not a JPG/u,
  );
  assert.equal(
    checkImageFile({ name: "huge.jpg", type: "image/jpeg", size: 7.2 * MB }),
    "huge.jpg is 7.2 MB. Choose a photo of 5 MB or less.",
  );
  assert.match(
    checkImageFile({ name: "empty.png", type: "image/png", size: 0 }) ?? "",
    /is empty/u,
  );
  assert.equal(formatBytes(820 * 1024), "820 KB");
  assert.equal(formatBytes(12 * MB), "12 MB");
});

Deno.test("after an upload, focus goes to the alt text or back to the button that picks a photo", () => {
  assert.equal(focusAfterUpload("done", true), "alt");
  assert.equal(focusAfterUpload("failed", true), "replace");
  assert.equal(focusAfterUpload("cancelled", true), "replace");
  assert.equal(focusAfterUpload("failed", false), "choose");
  assert.equal(focusAfterUpload("cancelled", false), "choose");
});

Deno.test("when a tile goes, focus moves to the tile that took its place or the last one", () => {
  assert.equal(tileAfter(1, 4), 1);
  assert.equal(tileAfter(3, 3), 2);
  assert.equal(tileAfter(0, 1), 0);
  assert.equal(tileAfter(-1, 2), 0);
  assert.equal(tileAfter(0, 0), null);
});
