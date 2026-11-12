// Compare two real desktop capture sets made at the same viewport and DPR.
import { readFileSync, mkdirSync, writeFileSync, existsSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { PNG } from 'pngjs';
import pixelmatch from 'pixelmatch';

const args = process.argv.slice(2);
const option = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};
const reference = resolve(option('baseline', 'docs/ui/screens/baseline'));
const current = resolve(option('current', 'out/ui-screens/current'));
const output = resolve(option('out', 'out/ui-screens/diff'));
const limit = Number(option('max-ratio', '0.001'));
const manifest = (dir) => JSON.parse(readFileSync(join(dir, 'manifest.json'), 'utf8'));
if (!existsSync(join(reference, 'manifest.json'))) {
  console.log(
    `::warning::No desktop visual baseline at ${reference}; capture is reported but not compared.`,
  );
  process.exit(0);
}
const before = manifest(reference);
const after = manifest(current);
if (
  !Array.isArray(before.captures) ||
  !before.captures.length ||
  !Array.isArray(after.captures) ||
  !after.captures.length
) {
  throw new Error('Desktop visual manifests must contain captures');
}
if (before.windowsScalePercent !== after.windowsScalePercent) {
  throw new Error('Desktop visual captures use different Windows scaling settings');
}
const byName = new Map(after.captures.map((entry) => [entry.file, entry]));
mkdirSync(output, { recursive: true });
let failed = false;
for (const previous of before.captures) {
  const now = byName.get(previous.file);
  if (
    !now ||
    previous.dpr !== now.dpr ||
    previous.width !== now.width ||
    previous.height !== now.height
  ) {
    console.error(`::error::Missing or incompatible viewport/DPR for ${previous.file}`);
    failed = true;
    continue;
  }
  const a = PNG.sync.read(readFileSync(join(reference, previous.file)));
  const b = PNG.sync.read(readFileSync(join(current, now.file)));
  if (a.width !== b.width || a.height !== b.height) {
    console.error(`::error::Pixel dimensions changed for ${previous.file}`);
    failed = true;
    continue;
  }
  const diff = new PNG({ width: a.width, height: a.height });
  const pixels = pixelmatch(a.data, b.data, diff.data, a.width, a.height, { threshold: 0.1 });
  const ratio = pixels / (a.width * a.height);
  console.log(`${previous.file}: ${(ratio * 100).toFixed(3)}% changed`);
  if (ratio > limit) {
    writeFileSync(join(output, previous.file), PNG.sync.write(diff));
    console.error(`::error::${previous.file} exceeds ${(limit * 100).toFixed(2)}%`);
    failed = true;
  }
}
for (const warning of after.warnings ?? []) console.log(`::warning::Capture coverage: ${warning}`);
if (failed) process.exitCode = 1;
