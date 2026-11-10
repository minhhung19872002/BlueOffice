// Copy bundled fonts and corpus samples into an app's public/ directory.
// Usage: node scripts/sync-assets.mjs <public-dir>
import { copyFileSync, mkdirSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const target = resolve(process.argv[2] ?? join(root, 'apps', 'web', 'public'));
const fontsDir = join(root, 'assets', 'fonts');
const manifest = JSON.parse(readFileSync(join(fontsDir, 'manifest.json'), 'utf8'));

mkdirSync(join(target, 'fonts'), { recursive: true });
copyFileSync(join(fontsDir, 'manifest.json'), join(target, 'fonts', 'manifest.json'));
for (const f of manifest.fonts) copyFileSync(join(fontsDir, f), join(target, 'fonts', f));

// Spelling dictionaries (assets/dictionaries/<lang>/*.aff, *.dic, README).
const dicts = join(root, 'assets', 'dictionaries');
for (const lang of readdirSync(dicts)) {
  mkdirSync(join(target, 'dictionaries', lang), { recursive: true });
  for (const f of readdirSync(join(dicts, lang)))
    copyFileSync(join(dicts, lang, f), join(target, 'dictionaries', lang, f));
}

// Thesaurus data (assets/thesaurus/<lang>/*.json.gz, README; CORE-THES-001).
const thes = join(root, 'assets', 'thesaurus');
for (const lang of readdirSync(thes, { withFileTypes: true })) {
  if (!lang.isDirectory()) continue;
  mkdirSync(join(target, 'thesaurus', lang.name), { recursive: true });
  for (const f of readdirSync(join(thes, lang.name)))
    copyFileSync(join(thes, lang.name, f), join(target, 'thesaurus', lang.name, f));
}

mkdirSync(join(target, 'samples'), { recursive: true });
copyFileSync(
  join(root, 'tests', 'corpus', 'doc', 'phase1-sample.docx'),
  join(target, 'samples', 'phase1-sample.docx'),
);
console.log(`assets → ${target}`);
