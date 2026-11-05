// Fail if any npm production/dev dependency uses a license outside the allow-list (prompt §1).
import { execFileSync } from 'node:child_process';

const ALLOWED = [
  'MIT',
  'MIT-0',
  'ISC',
  'Apache-2.0',
  'BSD-2-Clause',
  'BSD-3-Clause',
  '0BSD',
  'MPL-2.0',
  'Zlib',
  'OFL-1.1',
  'CC0-1.0',
  'CC-BY-4.0',
  'Python-2.0',
  'BlueOak-1.0.0',
  'Unlicense',
];

const out = execFileSync('pnpm', ['licenses', 'list', '--json', '--long'], {
  encoding: 'utf8',
  maxBuffer: 64 * 1024 * 1024,
  shell: process.platform === 'win32',
});
const byLicense = JSON.parse(out);
const isAllowed = (expr) => {
  // SPDX expressions: accept "A OR B" if any side is allowed; "A AND B" if all are.
  const clean = expr.replace(/[()]/g, '').trim();
  if (clean.includes(' OR ')) return clean.split(' OR ').some((x) => isAllowed(x));
  if (clean.includes(' AND ')) return clean.split(' AND ').every((x) => isAllowed(x));
  return ALLOWED.includes(clean);
};
const bad = [];
for (const [license, pkgs] of Object.entries(byLicense)) {
  if (!isAllowed(license))
    for (const p of pkgs) bad.push(`${p.name}@${p.versions ?? p.version}: ${license}`);
}
if (bad.length) {
  console.error('Disallowed licenses:\n' + bad.join('\n'));
  process.exit(1);
}
console.log(`license check OK (${Object.keys(byLicense).length} license kinds)`);
