/**
 * Web/desktop parity: the WASM engine (web) and the native engine (desktop,
 * exercised here through `bo-cli dump`, which calls the same `bo-engine`) must
 * produce byte-identical display lists for the same file. Because the canvas
 * renderer is shared code, identical display lists ⇒ identical rendering.
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import { initSync, WasmEngine } from '@blueoffice/engine-wasm';

const root = join(__dirname, '..', '..', '..');
const sample = join(root, 'tests', 'corpus', 'doc', 'phase1-sample.docx');
const exe = process.platform === 'win32' ? 'bo-cli.exe' : 'bo-cli';

/** The most recently built bo-cli (a stale release build must not win). */
function cli(): string {
  const built = ['release', 'debug']
    .map((p) => join(root, 'target', p, exe))
    .filter((f) => existsSync(f))
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  if (built[0]) return built[0];
  execFileSync('cargo', ['build', '-q', '-p', 'bo-cli'], { cwd: root, stdio: 'inherit' });
  return join(root, 'target', 'debug', exe);
}

let engine: WasmEngine;

beforeAll(() => {
  initSync({
    module: readFileSync(join(root, 'packages', 'engine-wasm', 'pkg', 'bo_wasm_bg.wasm')),
  });
  engine = new WasmEngine();
  const fontsDir = join(root, 'assets', 'fonts');
  const manifest = JSON.parse(readFileSync(join(fontsDir, 'manifest.json'), 'utf8')) as {
    fonts: string[];
  };
  for (const f of manifest.fonts) engine.addFont(readFileSync(join(fontsDir, f)));
});

describe('web (WASM) vs desktop (native) engine parity', () => {
  it('produces identical display lists for every page of the sample', () => {
    const info = JSON.parse(engine.open(readFileSync(sample), 'phase1-sample.docx')) as {
      pageCount: number;
    };
    expect(info.pageCount).toBeGreaterThanOrEqual(3);
    const bin = cli();
    for (let i = 0; i < info.pageCount; i++) {
      const native = execFileSync(bin, ['dump', sample, '--page', String(i)], {
        encoding: 'utf8',
        maxBuffer: 64 << 20,
      }).trim();
      const wasm = engine.pageJson(i);
      expect(wasm.length).toBeGreaterThan(1000);
      expect(wasm === native, `page ${i + 1} differs`).toBe(true);
    }
  }, 120_000);

  it('exports a PDF from WASM', () => {
    const pdf = engine.exportPdf();
    expect(new TextDecoder().decode(pdf.slice(0, 5))).toBe('%PDF-');
  });
});
