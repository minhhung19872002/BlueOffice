import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { DOC_QUERY_COMMANDS } from '../src/queryCommands';

const packages = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) return sources(p);
    return /\.tsx?$/.test(name) ? [p] : [];
  });
}

describe('DOC_QUERY_COMMANDS', () => {
  it('knows every query the BlueDoc editor and its text layer send', () => {
    const known = new Set(DOC_QUERY_COMMANDS);
    const sent = new Set<string>();
    // `controller.query({ cmd: '…' })` in the editor, and the engine queries of the
    // accessibility text layer in canvas-host.
    for (const f of sources(join(packages, 'editor', 'src'))) {
      const text = readFileSync(f, 'utf8');
      for (const m of text.matchAll(/\.query\(\{\s*cmd:\s*'([A-Za-z0-9_]+)'/g)) sent.add(m[1]!);
    }
    const a11y = readFileSync(join(packages, 'canvas-host', 'src', 'a11y.ts'), 'utf8');
    for (const m of a11y.matchAll(/cmd:\s*'([A-Za-z0-9_]+)'/g)) sent.add(m[1]!);
    expect(sent.size).toBeGreaterThan(5);
    expect([...sent].filter((c) => !known.has(c)).sort()).toEqual([]);
  });

  it('is what hosted sessions, the recorder and "repeat" treat as read-only', () => {
    for (const rel of [
      ['hosted', 'src', 'readonly.ts'],
      ['script', 'src', 'recorder.ts'],
      ['editor', 'src', 'repeat.ts'],
    ]) {
      const text = readFileSync(join(packages, ...rel), 'utf8');
      expect(text, rel.join('/')).toContain('...DOC_QUERY_COMMANDS');
    }
  });

  it('has no duplicates', () => {
    expect(new Set(DOC_QUERY_COMMANDS).size).toBe(DOC_QUERY_COMMANDS.length);
  });
});
