/**
 * BlueScript sandbox (CORE-SCRIPT-001): no way out of QuickJS except the API
 * bridge, limits on time / memory / calls, cancellation.
 */
import { describe, expect, it } from 'vitest';
import { loadQuickJS, runSandboxed, type ScriptCall } from '../src/sandbox';
import { API_METHODS, callToSource, PRELUDE } from '../src/prelude';
import { BLUESCRIPT_DTS, apiNames, complete } from '../src/completion';
import { inlineRunner } from '../src/runner';
import type { ScriptHost } from '../src/host';
import { scriptStore } from '../src/store';

const noCalls: ScriptCall = async (m) => {
  throw new Error(`unexpected ${m}`);
};

async function run(code: string, opts: Partial<Parameters<typeof runSandboxed>[2]> = {}) {
  const module = await loadQuickJS();
  const logs: string[] = [];
  const r = await runSandboxed(module, code, {
    app: 'doc',
    call: noCalls,
    log: (level, text) => logs.push(`${level}:${text}`),
    ...opts,
  });
  return { ...r, logs };
}

describe('sandbox isolation', () => {
  it('has no browser, worker or node globals', async () => {
    const r = await run(`
      return ['fetch', 'XMLHttpRequest', 'WebSocket', 'postMessage', 'self', 'window', 'document',
        'importScripts', 'localStorage', 'indexedDB', 'setTimeout', 'setInterval', 'require',
        'process', 'Deno', 'Worker', 'navigator', 'location', '__bo_call', '__bo_log', 'std', 'os']
        .filter((n) => typeof globalThis[n] !== 'undefined');
    `);
    expect(r.error).toBeUndefined();
    expect(r.value).toEqual([]);
  });

  it('cannot escape through constructors or eval', async () => {
    const r = await run(`
      const viaFn = Function('return typeof fetch + typeof postMessage')();
      const viaEval = (0, eval)('typeof self');
      const viaCtor = (async () => {}).constructor('return typeof globalThis.postMessage')();
      return [viaFn, viaEval, await viaCtor];
    `);
    expect(r.value).toEqual(['undefinedundefined', 'undefined', 'undefined']);
  });

  it('a postMessage call is an error inside the script', async () => {
    const r = await run(`postMessage({ steal: 1 }, '*');`);
    expect(r.ok).toBe(false);
    expect(r.error?.kind).toBe('error');
    expect(r.error?.message).toMatch(/postMessage/);
  });

  it('a fetch call is an error inside the script', async () => {
    const r = await run(`await fetch('https://example.com');`);
    expect(r.ok).toBe(false);
    expect(r.error?.message).toMatch(/fetch/);
  });

  it('only allow-listed methods reach the host, with JSON copies of the arguments', async () => {
    const seen: [string, unknown[]][] = [];
    const r = await run(
      `
      const out = await doc.find('x', { matchCase: true });
      return out;
    `,
      {
        call: async (m, args) => {
          seen.push([m, args]);
          return [{ paragraph: 0, start: 1, endParagraph: 0, end: 2 }];
        },
      },
    );
    expect(r.error).toBeUndefined();
    expect(seen).toEqual([['doc.find', ['x', { matchCase: true }]]]);
    expect(r.value).toEqual([{ paragraph: 0, start: 1, endParagraph: 0, end: 2 }]);
  });

  it('host errors come back as exceptions the script can catch', async () => {
    const r = await run(
      `try { await doc.insertText('a'); } catch (e) { return 'caught ' + e.message; }`,
      {
        call: async () => {
          throw new Error('protected');
        },
      },
    );
    expect(r.value).toBe('caught protected');
  });

  it('console output reaches the host', async () => {
    const r = await run(
      `console.log('a', 1, { b: 2 }); console.warn('w'); console.error(undefined);`,
    );
    expect(r.ok).toBe(true);
    expect(r.logs).toEqual(['log:a 1 {"b":2}', 'warn:w', 'error:undefined']);
  });

  it('reports syntax errors', async () => {
    const r = await run('const = 1;');
    expect(r.ok).toBe(false);
    expect(r.error?.kind).toBe('syntax');
  });

  it('defines only the open application’s globals', async () => {
    const r = await run(
      'return [typeof doc, typeof workbook, typeof sheet, typeof presentation];',
      { app: 'sheet' },
    );
    expect(r.value).toEqual(['undefined', 'object', 'object', 'undefined']);
  });
});

describe('sandbox limits', () => {
  it('kills an infinite loop at the time limit', async () => {
    const t0 = Date.now();
    const r = await run('while (true) {}', { timeoutMs: 300 });
    expect(r.ok).toBe(false);
    expect(r.error?.kind).toBe('timeout');
    expect(Date.now() - t0).toBeLessThan(5_000);
  });

  it('kills an endless microtask loop', async () => {
    const r = await run('for (;;) await null;', { timeoutMs: 300 });
    expect(r.error?.kind).toBe('timeout');
  });

  it('stops a script waiting on a host call that never answers', async () => {
    const r = await run('await doc.insertText("x");', {
      timeoutMs: 200,
      call: () => new Promise(() => {}),
    });
    expect(r.error?.kind).toBe('timeout');
  });

  it('caps memory', async () => {
    const r = await run(`const a = []; for (;;) a.push(new Array(100000).fill(1.5));`, {
      memoryBytes: 16 * 1024 * 1024,
    });
    expect(r.ok).toBe(false);
    expect(r.error?.kind).toBe('memory');
  });

  it('caps the stack', async () => {
    const r = await run('const f = (n) => f(n + 1) + 1; f(0);');
    expect(r.ok).toBe(false);
    expect(r.error?.message).toMatch(/stack/i);
  });

  it('caps API calls', async () => {
    const r = await run('for (;;) await doc.insertText("x");', {
      maxCalls: 50,
      call: async () => null,
    });
    expect(r.ok).toBe(false);
    expect(r.error?.kind).toBe('limit');
    expect(r.calls).toBeLessThanOrEqual(51);
  });

  it('cancels', async () => {
    const ac = new AbortController();
    // On one thread a busy loop starves the timer that would abort it (the worker runner
    // terminates the worker instead); cancel a script that waits on the host.
    const p = run('for (;;) await doc.insertText("x");', {
      signal: ac.signal,
      timeoutMs: 10_000,
      call: () => new Promise((r) => setTimeout(r, 5)),
    });
    setTimeout(() => ac.abort(), 100);
    const r = await p;
    expect(r.error?.kind).toBe('cancelled');
  });

  it('runs again normally after a killed run', async () => {
    await run('while (true) {}', { timeoutMs: 100 });
    const r = await run('return 6 * 7;');
    expect(r.value).toBe(42);
  });
});

describe('runner', () => {
  it('brackets a run with begin / end and answers calls in order', async () => {
    const order: string[] = [];
    const host: ScriptHost = {
      app: 'doc',
      begin: async () => void order.push('begin'),
      end: async () => void order.push('end'),
      call: async (m, args) => {
        order.push(`${m}:${String(args[0])}`);
        await new Promise((r) => setTimeout(r, args[0] === 'a' ? 20 : 0));
        return null;
      },
    };
    const r = inlineRunner().run(
      `await Promise.all([doc.insertText('a'), doc.insertText('b')]);`,
      host,
    );
    const res = await r.done;
    expect(res.ok).toBe(true);
    expect(order).toEqual(['begin', 'doc.insertText:a', 'doc.insertText:b', 'end']);
  });

  it('Stop ends a run and still calls end()', async () => {
    let ended = false;
    const host: ScriptHost = {
      app: 'doc',
      end: async () => void (ended = true),
      call: () => new Promise(() => {}),
    };
    const r = inlineRunner().run('await doc.insertText("x");', host);
    setTimeout(() => r.stop(), 50);
    const res = await r.done;
    expect(res.error?.kind).toBe('cancelled');
    expect(ended).toBe(true);
  });
});

describe('typings and completion', () => {
  it('every prelude method is typed in bluescript.d.ts', () => {
    const typed = new Set(['doc', 'sheet', 'slide'].flatMap((a) => apiNames(a as 'doc')));
    // Spot checks of the public surface named in the backlog item.
    for (const n of [
      'BlueDocument.paragraphs',
      'BlueDocument.insertText',
      'BlueDocument.find',
      'BlueDocument.replace',
      'Paragraph.getStyle',
      'Range.values',
      'Range.formulas',
      'Range.format',
      'Sheet.addChart',
      'Presentation.slides',
      'Slide.shapes',
      'Shape.text',
      'Shape.setText',
    ]) {
      expect(typed.has(n), n).toBe(true);
    }
    // Every host method is reachable from the prelude source.
    for (const list of Object.values(API_METHODS)) {
      for (const m of list) expect(PRELUDE.includes(`'${m}'`), m).toBe(true);
    }
    expect(BLUESCRIPT_DTS).toContain('declare const doc: BlueDocument;');
  });

  it('completes members after a dot and globals while typing', () => {
    const c = complete('doc', 'await doc.ins', 13);
    expect(c?.items.map((m) => m.name)).toEqual(['insertPageBreak', 'insertTable', 'insertText']);
    expect(c?.from).toBe(10);
    const g = complete('sheet', 'wor', 3);
    expect(g?.items.map((m) => m.name)).toContain('workbook');
    const r = complete('sheet', 'range.', 6);
    expect(r?.items.map((m) => m.name)).toContain('setValues');
    expect(complete('doc', '', 0)).toBeNull();
  });

  it('turns recorded calls into source', () => {
    expect(callToSource({ method: 'doc.insertText', args: ['Xin chào'] })).toBe(
      'await doc.insertText("Xin chào");',
    );
    expect(callToSource({ method: 'sheet.setValues', args: [0, 'A1', [[1]]] })).toBe(
      'await (await workbook.sheet(0)).range("A1").setValues([[1]]);',
    );
  });
});

describe('saved scripts', () => {
  it('saves, lists and removes per application', () => {
    const m = new Map<string, string>();
    const storage = {
      getItem: (k: string) => m.get(k) ?? null,
      setItem: (k: string, v: string) => void m.set(k, v),
    };
    const docs = scriptStore('doc', storage);
    const a = docs.save({ name: 'B', code: '1' });
    docs.save({ name: 'A', code: '2' });
    expect(docs.list().map((s) => s.name)).toEqual(['A', 'B']);
    docs.save({ id: a.id, name: 'B2', code: '3' });
    expect(docs.list().find((s) => s.id === a.id)?.code).toBe('3');
    expect(scriptStore('sheet', storage).list()).toEqual([]);
    docs.remove(a.id);
    expect(docs.list().map((s) => s.name)).toEqual(['A']);
    m.set('blueoffice.scripts.doc', 'not json');
    expect(docs.list()).toEqual([]);
  });
});
