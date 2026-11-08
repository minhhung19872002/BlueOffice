// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
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
    fireEvent.click(screen.getByText('Light / Dark'));
    await waitFor(() => expect(root.getAttribute('data-theme')).toBe('dark'));
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
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Tệp' }));
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
});
