import { describe, expect, it } from 'vitest';
import type { EditCommand, EditResult, EngineHost } from '@blueoffice/platform';
import { DocController, type EditView } from '../src/controller';
import { CopyInFlight } from '../src/clipboard/copyInFlight';
import { ClipboardHistory } from '../src/clipboard/history';

function result(over: Partial<EditResult> = {}): EditResult {
  return {
    info: { name: 'a.docx', pageCount: 1, pages: [{ width: 595, height: 842 }], wordCount: 0 },
    selection: { anchor: { p: 0, o: 0 }, focus: { p: 0, o: 0 } },
    caret: { page: 0, x: 85, y: 56, h: 15 },
    rects: [],
    dirty: [],
    format: {} as EditResult['format'],
    modified: true,
    ...over,
  };
}

function memoryStorage(): Storage {
  const m = new Map<string, string>();
  return {
    get length() {
      return m.size;
    },
    clear: () => m.clear(),
    getItem: (k) => m.get(k) ?? null,
    key: (i) => [...m.keys()][i] ?? null,
    removeItem: (k) => void m.delete(k),
    setItem: (k, v) => void m.set(k, v),
  };
}

/** A controller whose system clipboard write only finishes when `finishWrite` is called,
 * as a slow clipboard (permission prompt, desktop IPC) would. */
function racing() {
  const commands: EditCommand[] = [];
  let system = { text: 'old text', html: '' };
  let finishWrite: () => void = () => {};
  const engine = {
    command: async (c: EditCommand) => {
      commands.push(c);
      if (c.cmd === 'copy') {
        return result({
          clip: { text: 'new text', html: '<b>new</b> text', internal: '[{"n":1}]' },
        });
      }
      return result();
    },
  } as unknown as EngineHost;
  const view: EditView = { refresh: () => {}, setSelection: () => {} };
  const clipboard = {
    readText: async () => system.text,
    read: async () => ({ text: system.text, html: system.html }),
    writeText: async () => {},
    writeHtml: (html: string, text: string) =>
      new Promise<void>((resolve) => {
        finishWrite = () => {
          system = { text, html };
          resolve();
        };
      }),
  };
  const c = new DocController(
    engine,
    view,
    clipboard,
    () => {},
    (e) => {
      throw e;
    },
  );
  c.history = new ClipboardHistory(memoryStorage());
  const pastes = () => commands.filter((x) => x.cmd === 'paste');
  return { c, pastes, finish: () => finishWrite() };
}

const tick = () => new Promise((r) => setTimeout(r, 0));

describe('copy in flight (CORE-MISC-003)', () => {
  it('a paste event fired before the copy reached the clipboard pastes the copy', async () => {
    const { c, pastes, finish } = racing();
    const copying = c.copy();
    await tick();
    expect(c.copies.busy).toBe(true);
    // Ctrl+V right after Ctrl+C: the event still carries the old clipboard content.
    const pasting = c.pasteEvent('old text', '');
    await tick();
    // The paste waits for the copy instead of pasting the stale content.
    expect(pastes()).toHaveLength(0);
    finish();
    await copying;
    await pasting;
    expect(pastes()).toHaveLength(1);
    expect(pastes()[0]).toMatchObject({ text: 'new text', internal: '[{"n":1}]' });
    expect(c.copies.busy).toBe(false);
  });

  it('the paste button reads the clipboard only after the copy is done', async () => {
    const { c, pastes, finish } = racing();
    const copying = c.copy();
    const pasting = c.pasteFromClipboard();
    await tick();
    expect(pastes()).toHaveLength(0);
    finish();
    await Promise.all([copying, pasting]);
    expect(pastes()[0]).toMatchObject({ text: 'new text', internal: '[{"n":1}]' });
  });

  it('without a copy in flight the event content is pasted as is', async () => {
    const { c, pastes } = racing();
    await c.pasteEvent('from elsewhere', '<i>from elsewhere</i>');
    expect(pastes()[0]).toMatchObject({ text: 'from elsewhere', html: '<i>from elsewhere</i>' });
    expect(pastes()[0]).toMatchObject({ internal: '' });
  });
});

describe('CopyInFlight', () => {
  it('waits for the latest copy and forgets failed ones', async () => {
    const f = new CopyInFlight<string>();
    expect(await f.settle()).toEqual({ waited: false, result: null });
    let a: (v: string) => void = () => {};
    const first = f.track(new Promise<string>((r) => (a = r)));
    const settling = f.settle();
    // A second copy starts while a paste is waiting: the paste waits for it too.
    let b: (v: string) => void = () => {};
    const second = f.track(new Promise<string>((r) => (b = r)));
    a('one');
    await first;
    b('two');
    await second;
    expect(await settling).toEqual({ waited: true, result: 'two' });
    expect(f.busy).toBe(false);
    await f.track(Promise.reject(new Error('denied'))).catch(() => {});
    expect(f.busy).toBe(false);
    expect(await f.settle()).toEqual({ waited: false, result: null });
  });
});
