// Writes the smaller copies of the photos bundled in static/seed/, which src() and srcset() in src/lib/media/images.ts ask for (seedWidths there decides which exist). Run it from sites/web after adding or replacing a seed photo, and commit what it writes:
//   deno run -A scripts/seed-sizes.ts
// The type-aware linter cannot see Node's types, so calls on node:fs read as untyped here.
/* oxlint-disable typescript/no-unsafe-assignment, typescript/no-unsafe-call, typescript/no-unsafe-member-access, typescript/no-unsafe-argument, typescript/no-unsafe-return, typescript/strict-boolean-expressions */
import { readdir } from "node:fs/promises";
import { join } from "node:path";
import sharp from "sharp";
import { seedVariantPath, seedWidths } from "../src/lib/media/images.ts";

const ROOT = "static";
const VARIANT = /-\d+\.jpg$/u;

async function photos(dir: string): Promise<string[]> {
  const entries = await readdir(dir, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map((entry) => {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) return photos(path);
      return Promise.resolve(
        entry.name.endsWith(".jpg") && !VARIANT.test(entry.name) ? [path] : [],
      );
    }),
  );
  return nested.flat();
}

async function writeCopies(file: string): Promise<string[]> {
  const { width } = await sharp(file).metadata();
  const path = `/${file.slice(ROOT.length + 1)}`;
  const targets = seedWidths(path, width).slice(0, -1);
  return Promise.all(
    targets.map(async (target) => {
      const out = join(ROOT, seedVariantPath(path, target).slice(1));
      await sharp(file)
        .resize({ width: target })
        .jpeg({ quality: 80, progressive: true, mozjpeg: true })
        .toFile(out);
      return `${out} (${target.toString()} px)`;
    }),
  );
}

const written = await Promise.all(
  (await photos(join(ROOT, "seed"))).map((file) => writeCopies(file)),
);
for (const line of written.flat()) console.log(line);
