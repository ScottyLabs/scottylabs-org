/// <reference lib="dom" />
// Tests for image URLs, sizes, upload checks and video links. Run from sites/web with `deno test tests/`.
// The DOM library reference types the browser code under test; the tests themselves never use it.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access */

import assert from "node:assert/strict";
import type { ImageRef } from "../src/lib/api/types.ts";
import { focusAfterUpload, tileAfter } from "../src/lib/media/focus.ts";
import {
  aspectRatio,
  isSeedPhoto,
  isUploaded,
  seedVariantPath,
  seedWidths,
  src,
  srcset,
  storedWidths,
} from "../src/lib/media/images.ts";
import { checkImageFile, formatBytes } from "../src/lib/media/upload.ts";
import { videoEmbed } from "../src/lib/media/video.ts";

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

Deno.test("srcset lists every stored width of an upload", () => {
  assert.equal(
    srcset(image(1920, 1080)),
    `/media/${ID}?w=480 480w, /media/${ID}?w=960 960w, /media/${ID}?w=1920 1920w`,
  );
  assert.equal(
    srcset(image(1200, 900)),
    `/media/${ID}?w=480 480w, /media/${ID}?w=960 960w, /media/${ID}?w=1200 1200w`,
  );
  assert.equal(srcset(image(1600, 1200, "/seed/logos/scottylabs.png")), undefined);
});

Deno.test("src picks the smallest stored width that is wide enough", () => {
  assert.equal(src(image(1920, 1080), 300), `/media/${ID}?w=480`);
  assert.equal(src(image(1920, 1080), 481), `/media/${ID}?w=960`);
  assert.equal(src(image(1920, 1080), 4000), `/media/${ID}?w=1920`);
  assert.equal(src(image(1200, 900), 1500), `/media/${ID}?w=1200`);
  assert.equal(src(image(null, null), 960), `/media/${ID}?w=960`);
  assert.equal(
    src(image(800, 600, "/seed/logos/scottylabs.png"), 480),
    "/seed/logos/scottylabs.png",
  );
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

Deno.test("bundled photos come in smaller copies at most 80% as wide", () => {
  assert.equal(isSeedPhoto("/seed/photos/c10.jpg"), true);
  assert.equal(isSeedPhoto("/seed/../etc/passwd.jpg"), false);
  assert.equal(isSeedPhoto("/seed//photos/c10.jpg"), false);
  assert.equal(isSeedPhoto("/seed/photos/c10.png"), false);
  assert.deepEqual(seedWidths("/seed/projects/maps.jpg", 1440), [480, 960, 1440]);
  assert.deepEqual(seedWidths("/seed/photos/c10.jpg", 1000), [480, 1000]);
  assert.deepEqual(seedWidths("/seed/photos/small.jpg", 500), [500]);
  assert.deepEqual(seedWidths("/seed/photos/c10.jpg", null), []);
  assert.equal(seedVariantPath("/seed/photos/c10.jpg", 480), "/seed/photos/c10-480.jpg");
});

Deno.test("bundled photos get a srcset and a src that fits", () => {
  const maps = { src: "/seed/projects/maps.jpg", width: 1440 };
  assert.equal(
    srcset(maps),
    "/seed/projects/maps-480.jpg 480w, /seed/projects/maps-960.jpg 960w, /seed/projects/maps.jpg 1440w",
  );
  assert.equal(src(maps, 300), "/seed/projects/maps-480.jpg");
  assert.equal(src(maps, 700), "/seed/projects/maps-960.jpg");
  assert.equal(src(maps, 1920), "/seed/projects/maps.jpg");
  const photo = { src: "/seed/photos/c10.jpg", width: 1000 };
  assert.equal(srcset(photo), "/seed/photos/c10-480.jpg 480w, /seed/photos/c10.jpg 1000w");
  assert.equal(src(photo, 960), "/seed/photos/c10.jpg");
  // Without a known width there is nothing to choose from.
  assert.equal(srcset({ src: "/seed/photos/c10.jpg", width: null }), undefined);
  assert.equal(src({ src: "/seed/photos/c10.jpg", width: null }, 480), "/seed/photos/c10.jpg");
});

Deno.test("videos embed from YouTube and Vimeo without tracking", () => {
  assert.deepEqual(videoEmbed("https://www.youtube.com/watch?v=dQw4w9WgXcQ"), {
    provider: "youtube",
    providerName: "YouTube",
    embedUrl: "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?autoplay=1&rel=0",
  });
  assert.equal(
    videoEmbed("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1m30s")?.embedUrl,
    "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?autoplay=1&rel=0&start=90",
  );
  assert.equal(videoEmbed("https://youtu.be/dQw4w9WgXcQ")?.provider, "youtube");
  assert.equal(videoEmbed("https://youtube.com/shorts/dQw4w9WgXcQ")?.provider, "youtube");
  assert.equal(videoEmbed("https://m.youtube.com/live/dQw4w9WgXcQ")?.provider, "youtube");
  assert.deepEqual(videoEmbed("https://vimeo.com/76979871"), {
    provider: "vimeo",
    providerName: "Vimeo",
    embedUrl: "https://player.vimeo.com/video/76979871?autoplay=1&dnt=1",
  });
  assert.equal(
    videoEmbed("https://vimeo.com/76979871/abcdef1234")?.embedUrl,
    "https://player.vimeo.com/video/76979871?autoplay=1&dnt=1&h=abcdef1234",
  );
  assert.equal(
    videoEmbed("https://vimeo.com/channels/staffpicks/76979871")?.embedUrl,
    "https://player.vimeo.com/video/76979871?autoplay=1&dnt=1",
  );
  assert.equal(videoEmbed("https://player.vimeo.com/video/76979871")?.provider, "vimeo");
  for (const bad of [
    "",
    "not a link",
    "javascript:alert(1)",
    "http://youtube.com/watch?v=dQw4w9WgXcQ",
    "https://evil.example/watch?v=dQw4w9WgXcQ",
    "https://youtube.com/watch?v=<script>",
    "https://www.youtube.com/watch?v=bad",
    "https://www.youtube.com/@scottylabs",
    "https://www.youtube.com/playlist?list=PL1234567890",
    "https://vimeo.com/channels/staffpicks",
    "https://vimeo.com/12",
  ]) {
    assert.equal(videoEmbed(bad), null, bad);
  }
});

Deno.test("committee and project pages play the same links", () => {
  // These used to play on one kind of page and not the other.
  assert.notEqual(videoEmbed("https://youtu.be/abcdefg"), null);
  assert.equal(videoEmbed("https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ"), null);
});
