// Build crates/bo-wasm for the browser and generate JS glue into packages/engine-wasm/pkg.
// Usage: node scripts/build-wasm.mjs [--debug]
import { execFileSync } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const debug = process.argv.includes('--debug');
const profile = debug ? 'debug' : 'release';
const run = (cmd, args) => execFileSync(cmd, args, { cwd: root, stdio: 'inherit' });

run('cargo', [
  'build',
  '-p',
  'bo-wasm',
  '--target',
  'wasm32-unknown-unknown',
  ...(debug ? [] : ['--release']),
]);
const out = join(root, 'packages', 'engine-wasm', 'pkg');
mkdirSync(out, { recursive: true });
run('wasm-bindgen', [
  join(root, 'target', 'wasm32-unknown-unknown', profile, 'bo_wasm.wasm'),
  '--target',
  'web',
  '--out-dir',
  out,
]);
console.log(`bo-wasm (${profile}) → ${out}`);
