import { afterEach, describe, expect, it, vi } from 'vitest';
import { BrowserClipboard } from '../src/shared';
import {
  WebFileAccess,
  WebShell,
  fileSystemAccess,
  isExternalUrl,
  type FsFileHandle,
  type FsPickers,
} from '../src/web';

/** In-memory File System Access API: one directory of named files. */
function fakeFs(files: Record<string, Uint8Array>) {
  const written: string[] = [];
  const handle = (name: string, granted = true): FsFileHandle => ({
    name,
    getFile: async () => ({
      name,
      arrayBuffer: async () => (files[name] ?? new Uint8Array()).slice().buffer,
    }),
    createWritable: async () => {
      let data = new Uint8Array();
      return {
        write: async (b: Blob) => {
          data = new Uint8Array(await b.arrayBuffer());
        },
        close: async () => {
          files[name] = data;
          written.push(name);
        },
      };
    },
    queryPermission: async () => (granted ? 'granted' : 'prompt'),
    requestPermission: async () => (granted ? 'granted' : 'denied'),
  });
  const picks: (string | null)[] = [];
  const pickers: FsPickers = {
    showOpenFilePicker: async () => {
      const n = picks.shift();
      if (!n) throw Object.assign(new Error('cancel'), { name: 'AbortError' });
      return [handle(n)];
    },
    showSaveFilePicker: async (o) => {
      const n = picks.shift() ?? o.suggestedName;
      if (!n) throw Object.assign(new Error('cancel'), { name: 'AbortError' });
      return handle(n);
    },
  };
  return { pickers, picks, written, files };
}

describe('web file access (File System Access API)', () => {
  it('is used only when both pickers exist and the browser is not automated', () => {
    const p = { showOpenFilePicker: () => null, showSaveFilePicker: () => null };
    expect(fileSystemAccess({ ...p, navigator: {} })).not.toBeNull();
    expect(fileSystemAccess({ ...p, navigator: { webdriver: true } })).toBeNull();
    expect(fileSystemAccess({ showOpenFilePicker: () => null })).toBeNull();
  });

  it('opens with the picker and saves back into the same file', async () => {
    const fs = fakeFs({ 'Báo cáo.docx': new Uint8Array([1, 2, 3]) });
    const files = new WebFileAccess(fs.pickers);
    fs.picks.push('Báo cáo.docx');
    const f = await files.openFile(['.docx']);
    expect(f).toEqual({ name: 'Báo cáo.docx', bytes: new Uint8Array([1, 2, 3]) });
    // Same name: written in place, no picker.
    expect(await files.saveFile(new Uint8Array([9]), 'Báo cáo.docx', 'x')).toBe('Báo cáo.docx');
    expect(fs.files['Báo cáo.docx']).toEqual(new Uint8Array([9]));
    // Another name (export): the save picker chooses where.
    fs.picks.push('Báo cáo (bản in).pdf');
    expect(await files.saveFile(new Uint8Array([4]), 'Báo cáo.pdf', 'application/pdf')).toBe(
      'Báo cáo (bản in).pdf',
    );
    expect(fs.written).toEqual(['Báo cáo.docx', 'Báo cáo (bản in).pdf']);
  });

  it('treats a cancelled picker as cancel', async () => {
    const fs = fakeFs({});
    const files = new WebFileAccess(fs.pickers);
    fs.picks.push(null);
    expect(await files.openFile(['.docx'])).toBeNull();
    const cancelling: FsPickers = {
      ...fs.pickers,
      showSaveFilePicker: async () => {
        throw Object.assign(new Error('cancel'), { name: 'AbortError' });
      },
    };
    expect(await new WebFileAccess(cancelling).saveFile(new Uint8Array(), 'a.docx', '')).toBeNull();
  });
});

describe('rich web clipboard', () => {
  afterEach(() => vi.unstubAllGlobals());

  class Item {
    constructor(readonly data: Record<string, Blob>) {}
    get types() {
      return Object.keys(this.data);
    }
    async getType(t: string) {
      return this.data[t]!;
    }
  }

  function stub(withItem = true, refusePng = false) {
    const writes: Record<string, string>[] = [];
    const store: Item[] = [];
    vi.stubGlobal('navigator', {
      clipboard: {
        writeText: async (t: string) => {
          writes.push({ 'text/plain': t });
        },
        readText: async () => 'plain',
        write: async (items: Item[]) => {
          if (refusePng && items.some((i) => i.types.includes('image/png')))
            throw new Error('NotAllowed');
          const w: Record<string, string> = {};
          for (const i of items) for (const t of i.types) w[t] = await (await i.getType(t)).text();
          writes.push(w);
          store.splice(0, store.length, ...items);
        },
        read: async () => store,
      },
    });
    if (withItem) vi.stubGlobal('ClipboardItem', Item);
    else vi.stubGlobal('ClipboardItem', undefined);
    return writes;
  }

  it('writes HTML + text + PNG in one item and reads them back', async () => {
    const writes = stub();
    const c = new BrowserClipboard();
    await c.writeItems({
      text: 'Xin chào',
      html: '<b>Xin chào</b>',
      png: new Uint8Array([137, 80]),
    });
    expect(Object.keys(writes[0]!).sort()).toEqual(['image/png', 'text/html', 'text/plain']);
    const back = await c.read();
    expect(back.text).toBe('Xin chào');
    expect(back.html).toBe('<b>Xin chào</b>');
    expect(back.image).toEqual(new Uint8Array([137, 80]));
    expect(await c.readHtml()).toBe('<b>Xin chào</b>');
  });

  it('keeps HTML + text when the picture is refused, text only without ClipboardItem', async () => {
    let writes = stub(true, true);
    await new BrowserClipboard().writeItems({
      text: 't',
      html: '<i>t</i>',
      png: new Uint8Array([1]),
    });
    expect(writes.at(-1)).toEqual({ 'text/html': '<i>t</i>', 'text/plain': 't' });
    writes = stub(false);
    await new BrowserClipboard().writeHtml('<i>t</i>', 't');
    expect(writes).toEqual([{ 'text/plain': 't' }]);
  });
});

// source: docs/security.md — links followed from a slide show (SLIDE-ACTION-001).
describe('opening a link outside the app (web)', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('accepts web and mail addresses only', () => {
    for (const ok of ['https://example.vn/a?b=1', 'http://localhost:5173', 'mailto:a@example.vn']) {
      expect(isExternalUrl(ok), ok).toBe(true);
    }
    for (const bad of [
      'file:///C:/Windows/System32/calc.exe',
      'javascript:alert(1)',
      'data:text/html,x',
      'blob:https://example.vn/1',
      'bluedoc://open?file=x',
      'https:///x',
      'https://exa mple.vn',
      ' https://example.vn',
      'java\tscript:alert(1)',
      'mailto:',
      '',
    ]) {
      expect(isExternalUrl(bad), JSON.stringify(bad)).toBe(false);
    }
  });

  it('opens a new tab that cannot reach the app, and refuses everything else', async () => {
    const clicked: { href: string; target: string; rel: string }[] = [];
    const anchor = () => {
      const a = {
        href: '',
        target: '',
        rel: '',
        style: {} as Record<string, string>,
        click: () => clicked.push({ href: a.href, target: a.target, rel: a.rel }),
        remove: () => undefined,
      };
      return a;
    };
    vi.stubGlobal('document', {
      createElement: () => anchor(),
      body: { appendChild: () => undefined },
    });
    const shell = new WebShell();
    await shell.openExternal('https://example.vn/blueoffice');
    expect(clicked).toEqual([
      { href: 'https://example.vn/blueoffice', target: '_blank', rel: 'noopener noreferrer' },
    ]);
    for (const bad of ['file:///C:/Windows/System32/calc.exe', 'javascript:alert(1)']) {
      await expect(shell.openExternal(bad)).rejects.toThrow('openExternal.refused');
    }
    expect(clicked).toHaveLength(1);
  });
});
