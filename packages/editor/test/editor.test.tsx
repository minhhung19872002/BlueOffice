// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { DocInfo, EditCommand, EditResult, Platform } from '@blueoffice/platform';
import { BlueEditor } from '../src/BlueEditor';

afterEach(cleanup);

const info = (name: string): DocInfo => ({
  name,
  pageCount: 1,
  pages: [{ width: 595, height: 842 }],
  wordCount: 0,
});

function result(name: string, bold: boolean, modified: boolean): EditResult {
  return {
    info: info(name),
    selection: { anchor: { p: 0, o: 0 }, focus: { p: 0, o: 0 } },
    caret: { page: 0, x: 85, y: 56, h: 15 },
    rects: [],
    dirty: [0],
    format: {
      bold,
      italic: false,
      underline: false,
      strike: false,
      superscript: false,
      subscript: false,
      font: null,
      sizeHalfPoints: 26,
      color: null,
      highlight: null,
      align: 'Left',
      style: null,
      bullets: false,
      numbering: false,
      canUndo: modified,
      canRedo: false,
    },
    modified,
  };
}

function fakePlatform(): Platform & { opened: string[]; commands: EditCommand[]; saved: string[] } {
  const opened: string[] = [];
  const commands: EditCommand[] = [];
  const saved: string[] = [];
  let bold = false;
  let modified = false;
  let current = '';
  return {
    opened,
    commands,
    saved,
    kind: 'web',
    files: { openFile: async () => null, saveFile: async (_b, n) => n },
    storage: {
      id: 'local',
      open: async () => ({
        ref: { provider: 'local', id: 'a.docx', name: 'a.docx' },
        bytes: new Uint8Array([1]),
        permissions: { edit: true, download: true, share: false },
      }),
      save: async (d) => d,
      saveAs: async (name, bytes) => {
        saved.push(name);
        return {
          ref: { provider: 'local', id: name, name },
          bytes,
          permissions: { edit: true, download: true, share: false },
        };
      },
    },
    clipboard: { readText: async () => '', writeText: async () => {}, writeHtml: async () => {} },
    fonts: { list: async () => [], load: async () => new Uint8Array() },
    print: { printPdf: async () => {} },
    engine: {
      backend: 'wasm',
      init: async () => {},
      open: async (_b, name) => {
        opened.push(name);
        current = name;
        return {
          name,
          pageCount: 3,
          pages: [
            { width: 595, height: 842 },
            { width: 595, height: 842 },
            { width: 595, height: 842 },
          ],
          wordCount: 42,
        };
      },
      getPage: async () => ({ width: 595, height: 842, commands: [] }),
      getGlyphOutlines: async () => [],
      exportPdf: async () => new Uint8Array([37]),
      newDocument: async (name) => {
        current = name;
        return info(name);
      },
      command: async ({ now: _now, ...cmd }) => {
        commands.push(cmd);
        if (cmd.cmd === 'insertText') modified = true;
        if (cmd.cmd === 'formatText') bold = !bold;
        return result(current, bold, modified);
      },
      insertImage: async () => result('x', bold, modified),
      getMedia: async () => new Uint8Array(),
      saveDocx: async () => new Uint8Array([80, 75]),
      getSlideNotes: async () => '',
      savePptx: async () => new Uint8Array([80, 75]),
      slideCommand: async () => {
        throw new Error('not a presentation');
      },
      slideInsertPicture: async () => {
        throw new Error('not a presentation');
      },
      markSaved: async () => {
        modified = false;
      },
      dispose: () => {},
    },
    auth: {
      user: null,
      getAccessToken: async () => null,
      signIn: async () => {},
      signOut: async () => {},
    },
    window: { setTitle: vi.fn(), openNew: async () => {} },
    offline: { get: async () => null, put: async () => {}, delete: async () => {} },
    shell: {
      recentFiles: async () => [],
      addRecentFile: async () => {},
      onOpenRequest: () => () => {},
    },
  };
}

describe('BlueEditor', () => {
  it('shows the Vietnamese shell and opens a document via the storage provider', async () => {
    const p = fakePlatform();
    render(<BlueEditor platform={p} />);
    expect(screen.getByRole('tab', { name: 'Trang đầu' })).toBeTruthy();
    expect(screen.getByText('Chào mừng đến với BlueDoc')).toBeTruthy();
    expect(screen.getByTestId('status-engine').textContent).toBe('Lõi: WASM');
    fireEvent.click(screen.getByText('Mở tệp từ máy'));
    await waitFor(() => expect(screen.getByTestId('status-page').textContent).toBe('Trang 1/3'));
    expect(p.opened).toEqual(['a.docx']);
    expect(screen.getByTestId('doc-name').textContent).toBe('a.docx');
    expect(p.window.setTitle).toHaveBeenCalledWith('a.docx — BlueDoc');
  });

  it('tells about TCVN3/VNI text converted on open; the notice undoes it', async () => {
    const p = fakePlatform();
    const open = p.engine.open;
    p.engine.open = async (b, name) => ({ ...(await open(b, name)), legacyConverted: 2 });
    const command = p.engine.command;
    // The conversion is the first undo step.
    p.engine.command = async (c) => {
      const r = await command(c);
      return { ...r, format: { ...r.format, canUndo: true } };
    };
    render(<BlueEditor platform={p} />);
    fireEvent.click(screen.getByText('Mở tệp từ máy'));
    const bar = await screen.findByTestId('legacy-notice');
    expect(bar.textContent).toContain('Đã chuyển 2 đoạn chữ');
    fireEvent.click(screen.getByText('Hoàn tác chuyển đổi'));
    await waitFor(() => expect(p.commands).toContainEqual({ cmd: 'undo' }));
    expect(screen.queryByTestId('legacy-notice')).toBeNull();
  });

  it('switches language and theme through commands', async () => {
    render(<BlueEditor platform={fakePlatform()} />);
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Xem' }));
    fireEvent.click(screen.getByText('English'));
    await waitFor(() => expect(screen.getByRole('tab', { name: 'View' })).toBeTruthy());
    const root = document.querySelector('.bo-root')!;
    expect(root.getAttribute('data-theme')).toBe('light');
    fireEvent.click(screen.getByText('Light / Dark / High contrast'));
    await waitFor(() => expect(root.getAttribute('data-theme')).toBe('dark'));
    fireEvent.click(screen.getByText('Light / Dark / High contrast'));
    await waitFor(() => expect(root.getAttribute('data-theme')).toBe('hc'));
    fireEvent.click(screen.getByText('Light / Dark / High contrast'));
    await waitFor(() => expect(root.getAttribute('data-theme')).toBe('light'));
    fireEvent.click(screen.getByText('Density: Normal'));
    await waitFor(() =>
      expect(document.documentElement.getAttribute('data-density')).toBe('compact'),
    );
    expect(screen.getByText('Density: Compact')).toBeTruthy();
    fireEvent.click(screen.getByText('Density: Compact'));
    fireEvent.click(screen.getByText('Density: Touch'));
    await waitFor(() => expect(document.documentElement.hasAttribute('data-density')).toBe(false));
  });

  it('honours host configuration', () => {
    const onClose = vi.fn();
    render(
      <BlueEditor
        platform={fakePlatform()}
        host={{ brandName: 'BlueDrive', showShare: true, showClose: true, onClose }}
      />,
    );
    expect(screen.getByText(/BlueDrive · BlueDoc/)).toBeTruthy();
    expect(screen.getByText('Chia sẻ')).toBeTruthy();
    fireEvent.click(screen.getByLabelText('Đóng'));
    expect(onClose).toHaveBeenCalled();
  });

  it('creates a blank document, types, formats via the ribbon and saves', async () => {
    const p = fakePlatform();
    render(<BlueEditor platform={p} autosaveMs={0} />);
    fireEvent.click(screen.getByTestId('new-document'));
    await waitFor(() =>
      expect(screen.getByTestId('doc-name').textContent).toBe('Tài liệu mới.docx'),
    );
    const ime = document.querySelector('textarea')!;
    ime.value = 'Việt';
    fireEvent.input(ime);
    await waitFor(() => expect(p.commands).toContainEqual({ cmd: 'insertText', text: 'Việt' }));
    await waitFor(() => expect(screen.getByTestId('status-modified')).toBeTruthy());
    fireEvent.keyDown(ime, { key: 'Backspace' });
    await waitFor(() => expect(p.commands).toContainEqual({ cmd: 'deleteBackward' }));
    const bold = document.querySelector('[data-command="format.bold"]')!;
    fireEvent.click(bold);
    await waitFor(() => expect(bold.getAttribute('aria-pressed')).toBe('true'));
    fireEvent.keyDown(window, { key: 's', ctrlKey: true });
    await waitFor(() => expect(p.saved).toEqual(['Tài liệu mới.docx']));
    await waitFor(() => expect(screen.queryByTestId('status-modified')).toBeNull());
  });

  it('offers paste options after a paste and opens the clipboard history', async () => {
    const p = fakePlatform();
    render(<BlueEditor platform={p} autosaveMs={0} />);
    fireEvent.click(screen.getByTestId('new-document'));
    await waitFor(() =>
      expect(screen.getByTestId('doc-name').textContent).toBe('Tài liệu mới.docx'),
    );
    const ime = document.querySelector('textarea')!;
    const data: Record<string, string> = { 'text/html': '<b>đậm</b>', 'text/plain': 'đậm' };
    const e = new Event('paste', { bubbles: true, cancelable: true }) as Event & {
      clipboardData: unknown;
    };
    e.clipboardData = { getData: (t: string) => data[t] ?? '', items: [] };
    act(() => {
      ime.dispatchEvent(e);
    });
    await waitFor(() =>
      expect(p.commands).toContainEqual(expect.objectContaining({ cmd: 'paste', mode: 'keep' })),
    );
    fireEvent.click(await screen.findByTestId('paste-options'));
    fireEvent.click(screen.getByRole('menuitemradio', { name: /Chỉ văn bản thuần/ }));
    await waitFor(() =>
      expect(p.commands).toContainEqual(expect.objectContaining({ cmd: 'paste', mode: 'text' })),
    );
    const at = p.commands.findIndex((c) => c.cmd === 'paste' && c.mode === 'text');
    expect(p.commands[at - 1]?.cmd).toBe('undo');
    // Home › Clipboard launcher: the history panel with the default paste settings.
    fireEvent.click(document.querySelector('[data-command="edit.clipboardPanel"]')!);
    expect(await screen.findByTestId('clipboard-panel')).toBeTruthy();
    expect(screen.getByTestId('paste-defaults')).toBeTruthy();
  });

  it('inserts a symbol found by its Vietnamese keyword from the Insert tab', async () => {
    const p = fakePlatform();
    render(<BlueEditor platform={p} autosaveMs={0} />);
    fireEvent.click(screen.getByTestId('new-document'));
    await waitFor(() =>
      expect(screen.getByTestId('doc-name').textContent).toBe('Tài liệu mới.docx'),
    );
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Chèn' }));
    fireEvent.click(await waitFor(() => document.querySelector('[data-command="insert.symbol"]')!));
    const search = await screen.findByTestId('symbol-search');
    fireEvent.change(search, { target: { value: 'đồng' } });
    fireEvent.keyDown(search, { key: 'Enter' });
    await waitFor(() => expect(p.commands).toContainEqual({ cmd: 'insertText', text: '₫' }));
    fireEvent.keyDown(screen.getByTestId('symbol-panel'), { key: 'Escape' });
    expect(screen.queryByTestId('symbol-panel')).toBeNull();
  });

  it(
    'opens the icon library from the Insert tab and searches it without accents',
    { timeout: 40_000 },
    async () => {
      const p = fakePlatform();
      render(<BlueEditor platform={p} autosaveMs={0} />);
      fireEvent.click(screen.getByTestId('new-document'));
      await waitFor(() =>
        expect(screen.getByTestId('doc-name').textContent).toBe('Tài liệu mới.docx'),
      );
      fireEvent.mouseDown(screen.getByRole('tab', { name: 'Chèn' }));
      fireEvent.click(await waitFor(() => document.querySelector('[data-command="insert.icon"]')!));
      const panel = await screen.findByRole('dialog', { name: 'Chèn biểu tượng' });
      fireEvent.change(await screen.findByTestId('icon-search'), { target: { value: 'con dau' } });
      expect(
        await screen.findByRole('gridcell', { name: 'Con dấu tròn' }, { timeout: 20_000 }),
      ).toBeTruthy();
      fireEvent.keyDown(panel, { key: 'Escape' });
      expect(screen.queryByTestId('icon-panel')).toBeNull();
    },
  );

  it('opens the find panel and offers crash recovery', async () => {
    const p = fakePlatform();
    p.offline.get = async (k) =>
      k.endsWith('name') ? new TextEncoder().encode('báo cáo.docx') : new Uint8Array([1]);
    render(<BlueEditor platform={p} autosaveMs={0} />);
    await waitFor(() =>
      expect(screen.getByTestId('recovery').textContent).toContain('báo cáo.docx'),
    );
    fireEvent.click(screen.getByText('Khôi phục'));
    await waitFor(() => expect(p.opened).toEqual(['báo cáo.docx']));
    fireEvent.keyDown(window, { key: 'h', ctrlKey: true });
    const q = await screen.findByTestId('find-query');
    fireEvent.change(q, { target: { value: 'abc' } });
    fireEvent.click(screen.getByText('Thay tất cả'));
    await waitFor(() =>
      expect(p.commands).toContainEqual({ cmd: 'replaceAll', query: 'abc', options: {}, with: '' }),
    );
  });

  it('shows placeholders for sheet and slide apps', () => {
    render(<BlueEditor platform={fakePlatform()} app="sheet" />);
    expect(screen.getByText(/Phase 3/)).toBeTruthy();
  });
});

describe('BlueEditor digital signatures', () => {
  it('signs a copy through the file.sign command and shows the status of a signed file', async () => {
    const p = fakePlatform();
    const sign = vi.fn(async () => new Uint8Array([9, 9]));
    const savedFiles: string[] = [];
    const signedFile = new TextEncoder().encode('PK\x03\x04 _xmlsignatures/sig1.xml');
    const platform: Platform = {
      ...p,
      files: {
        openFile: async () => null,
        saveFile: async (_b, n) => {
          savedFiles.push(n);
          return n;
        },
      },
      storage: {
        ...p.storage,
        open: async () => ({
          ref: { provider: 'local', id: 'a.docx', name: 'a.docx' },
          bytes: signedFile,
          permissions: { edit: true, download: true, share: false },
        }),
      },
      signing: {
        kinds: ['remote'],
        defaults: async () => ({ remoteUrl: 'https://sign.example.vn' }),
        sign,
        verify: async () => ({
          kind: 'ooxml',
          error: null,
          signatures: [
            {
              signer: 'CN=A',
              signingTime: null,
              coversWholeDocument: true,
              timestamp: null,
              chain: { trusted: false, path: [], anchor: null, problems: [] },
              validatedAt: '',
            },
          ],
        }),
      },
    };
    render(<BlueEditor platform={platform} autosaveMs={0} />);
    fireEvent.click(screen.getByText('Mở tệp từ máy'));
    await waitFor(() =>
      expect(screen.getByTestId('signature-bar').dataset.state).toBe('untrusted'),
    );
    fireEvent.click(screen.getByTestId('file-tab'));
    fireEvent.click(document.querySelector('[data-command="file.sign"]')!);
    await waitFor(() => expect(screen.getByTestId('sign-dialog')).toBeTruthy());
    fireEvent.click(screen.getByTestId('sign-submit'));
    await waitFor(() => expect(savedFiles).toEqual(['a-signed.docx']));
    expect(sign).toHaveBeenCalledTimes(1);
    const [bytes, request] = sign.mock.calls[0] as unknown as [Uint8Array, unknown];
    expect(bytes).toEqual(new Uint8Array([80, 75]));
    expect(request).toMatchObject({
      signer: { kind: 'remote', url: 'https://sign.example.vn' },
      documentName: 'a-signed.docx',
    });
  });

  it('shows the table / picture / header-footer contextual tabs with the selection (CORE-UI-001)', async () => {
    const p = fakePlatform();
    let at: 'text' | 'table' | 'picture' | 'header' = 'text';
    const command = p.engine.command;
    p.engine.command = async (c) => {
      const r = await command(c);
      const format = {
        ...r.format,
        table:
          at === 'table'
            ? {
                rows: 2,
                cols: 2,
                row: 0,
                col: 0,
                headerRow: false,
                shading: null,
                align: 'Left' as const,
                widthPct: null,
                width: 0,
                multiCell: false,
                merged: false,
                direction: null,
                wrap: false,
                fixedLayout: false,
              }
            : null,
        pictureTools: at === 'picture',
      };
      const story =
        at === 'header'
          ? {
              kind: 'header' as const,
              section: 0,
              variant: 'default' as const,
              page: 0,
              titlePage: false,
              evenAndOdd: false,
              linked: false,
              canLink: false,
            }
          : undefined;
      return { ...r, format, story };
    };
    render(<BlueEditor platform={p} autosaveMs={0} />);
    fireEvent.click(screen.getByTestId('new-document'));
    await waitFor(() =>
      expect(screen.getByTestId('doc-name').textContent).toBe('Tài liệu mới.docx'),
    );
    const ime = document.querySelector('textarea')!;
    const type = (text: string) => {
      ime.value = text;
      fireEvent.input(ime);
    };
    const active = () =>
      screen.getAllByRole('tab').find((t) => t.getAttribute('aria-selected') === 'true')
        ?.textContent;
    expect(screen.queryByRole('tab', { name: 'Bảng' })).toBeNull();

    at = 'table';
    type('a');
    // The table tab appears under "Công cụ bảng"; typing in a cell keeps the Home tab.
    await waitFor(() => expect(screen.getByRole('tab', { name: 'Bảng' })).toBeTruthy());
    const band = document.querySelector('[data-context-group="table"]')!;
    expect(band.textContent).toContain('Công cụ bảng');
    expect(active()).toBe('Trang đầu');
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Bảng' }));
    expect(document.querySelector('[data-command="table.insertRowAbove"]')).not.toBeNull();

    at = 'picture';
    type('b');
    await waitFor(() => expect(active()).toBe('Định dạng ảnh'));
    expect(screen.queryByRole('tab', { name: 'Bảng' })).toBeNull();

    at = 'header';
    type('c');
    await waitFor(() => expect(active()).toBe('Đầu & chân trang'));
    expect(document.querySelector('[data-command="hf.close"]')).not.toBeNull();

    at = 'text';
    type('d');
    await waitFor(() => expect(active()).toBe('Trang đầu'));
    expect(document.querySelector('[data-context-group]')).toBeNull();
  });
});

describe('BlueDoc status bar, word count and split view (CORE-UI-007)', { timeout: 30_000 }, () => {
  async function open() {
    const p = fakePlatform();
    const command = p.engine.command;
    p.engine.command = async (cmd) => {
      const r = await command(cmd);
      return cmd.cmd === 'wordCount'
        ? {
            ...r,
            wordCount: {
              pages: 3,
              notes: cmd.notes ?? false,
              document: {
                words: cmd.notes ? 50 : 42,
                chars: 200,
                charsWithSpaces: 241,
                paragraphs: 7,
              },
            },
          }
        : r;
    };
    localStorage.clear();
    render(<BlueEditor platform={p} language="en-US" />);
    fireEvent.click(screen.getByText('Open a file'));
    await waitFor(() => expect(screen.getByTestId('status-page').textContent).toBe('Page 1 of 3'));
    return p;
  }

  it('shows the items of the app and lets the user pick them', async () => {
    await open();
    expect(screen.getByTestId('status-words').textContent).toMatch(/^\d+ words$/);
    expect(screen.getByTestId('status-track-changes').textContent).toBe('Track changes: Off');
    expect(screen.getByRole('radiogroup', { name: 'Views' })).toBeTruthy();
    expect(screen.getByRole('slider', { name: 'Zoom' })).toBeTruthy();
    // Overtype is off by default; the checklist turns it on, and words off.
    expect(screen.queryByTestId('status-overtype')).toBeNull();
    fireEvent.contextMenu(screen.getByTestId('status-bar'));
    expect(screen.getByRole('menu', { name: 'Customize status bar' })).toBeTruthy();
    expect(screen.getByTestId('status-item-page').textContent).toContain('Page 1 of');
    fireEvent.click(screen.getByTestId('status-item-overtype'));
    fireEvent.click(screen.getByTestId('status-item-words'));
    expect(screen.queryByTestId('status-words')).toBeNull();
    const overtype = screen.getByTestId('status-overtype');
    expect(overtype.textContent).toBe('Insert');
    await waitFor(() =>
      expect(JSON.parse(localStorage.getItem('blueoffice.statusbar.doc') ?? '{}')).toEqual({
        shown: ['overtype'],
        hidden: ['words'],
      }),
    );
    fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
    // The Insert key switches overtype mode; typing then replaces what follows the caret.
    const ime = document.querySelector('textarea')!;
    fireEvent.keyDown(ime, { key: 'Insert' });
    await waitFor(() => expect(screen.getByTestId('status-overtype').textContent).toBe('Overtype'));
  });

  it('overtype types over the next character', async () => {
    const p = await open();
    const ime = document.querySelector('textarea')!;
    fireEvent.keyDown(ime, { key: 'Insert' });
    ime.value = 'x';
    fireEvent.input(ime);
    await waitFor(() => expect(p.commands).toContainEqual({ cmd: 'insertText', text: 'x' }));
    const sent = p.commands.map((c) => c.cmd);
    expect(sent.indexOf('move')).toBeGreaterThan(-1);
    expect(sent.indexOf('move')).toBeLessThan(sent.lastIndexOf('insertText'));
    expect(p.commands).toContainEqual({ cmd: 'move', motion: 'right', extend: true });
  });

  it('a click on the word count opens the Word Count dialog', async () => {
    const p = await open();
    fireEvent.click(screen.getByTestId('status-words'));
    const dialog = await screen.findByTestId('word-count-dialog');
    await waitFor(() => expect(screen.getByTestId('word-count-words').textContent).toBe('42'));
    expect(screen.getByTestId('word-count-pages').textContent).toBe('3');
    expect(screen.getByTestId('word-count-charsWithSpaces').textContent).toBe('241');
    expect(dialog.textContent).toContain('Paragraphs');
    // Text boxes, footnotes and endnotes on request.
    fireEvent.click(screen.getByTestId('word-count-notes'));
    await waitFor(() => expect(screen.getByTestId('word-count-words').textContent).toBe('50'));
    expect(p.commands).toContainEqual({ cmd: 'wordCount', notes: true });
    fireEvent.click(screen.getByTestId('word-count-close'));
    await waitFor(() => expect(screen.queryByTestId('word-count-dialog')).toBeNull());
  });

  it('View › Window: Split shows a second pane of the same document', async () => {
    await open();
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'View' }));
    expect(document.querySelector('[data-command="window.new"]')).toBeTruthy();
    expect(document.querySelector('[data-command="window.arrangeAll"]')).toBeTruthy();
    const sync = document.querySelector('[data-command="window.syncScroll"]')!;
    expect(sync.hasAttribute('disabled') || sync.getAttribute('aria-disabled') === 'true').toBe(
      true,
    );
    const split = document.querySelector('[data-command="window.split"]')!;
    fireEvent.click(split);
    const pane = await screen.findByTestId('document-view-split');
    await waitFor(() => expect(pane.querySelector('[data-testid="split-scroller"]')).toBeTruthy());
    const bar = screen.getByRole('separator');
    expect(bar.getAttribute('aria-valuenow')).toBe('50');
    fireEvent.keyDown(bar, { key: 'ArrowDown' });
    await waitFor(() => expect(bar.getAttribute('aria-valuenow')).toBe('55'));
    // Both panes are views of one engine document: one textarea each for typing.
    expect(document.querySelectorAll('.bo-doc-scroller')).toHaveLength(2);
    fireEvent.click(document.querySelector('[data-command="window.split"]')!);
    await waitFor(() => expect(screen.queryByTestId('document-view-split')).toBeNull());
    expect(document.querySelectorAll('.bo-doc-scroller')).toHaveLength(1);
  });

  it('Arrange All explains what to do in a browser', async () => {
    await open();
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'View' }));
    fireEvent.click(document.querySelector('[data-command="window.arrangeAll"]')!);
    await waitFor(() =>
      expect(screen.getByTestId('status-info').textContent).toContain('drag the browser tab'),
    );
    // No other window: side by side says so.
    fireEvent.click(document.querySelector('[data-command="window.sideBySide"]')!);
    await waitFor(() =>
      expect(screen.getByTestId('status-info').textContent).toContain('no other window'),
    );
  });
});
