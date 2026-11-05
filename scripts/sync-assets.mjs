// Copy bundled fonts and corpus samples into an app's public/ directory.
// Usage: node scripts/sync-assets.mjs <public-dir>
import { copyFileSync, mkdirSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const target = resolve(process.argv[2] ?? join(root, 'apps', 'web', 'public'));
const fontsDir = join(root, 'assets', 'fonts');
const manifest = JSON.parse(readFileSync(join(fontsDir, 'manifest.json'), 'utf8'));

mkdirSync(join(target, 'fonts'), { recursive: true });
copyFileSync(join(fontsDir, 'manifest.json'), join(target, 'fonts', 'manifest.json'));
for (const f of manifest.fonts) copyFileSync(join(fontsDir, f), join(target, 'fonts', f));

mkdirSync(join(target, 'samples'), { recursive: true });
copyFileSync(
  join(root, 'tests', 'corpus', 'write', 'phase1-sample.docx'),
  join(target, 'samples', 'phase1-sample.docx'),
);
console.log(`assets → ${target}`);
