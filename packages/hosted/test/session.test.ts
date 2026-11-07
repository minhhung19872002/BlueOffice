import { describe, expect, it, vi } from 'vitest';
import type { Platform } from '@blueoffice/platform';
import {
  SessionMode,
  appForFile,
  commandHandler,
  fetchAllowedOrigins,
  hostedPlatform,
  openSession,
  parseMode,
  serverBase,
  type FetchLike,
} from '../src';

const session = {
  sessionId: 'S 1',
  sessionToken: 'secret',
  mode: 'edit' as const,
  file: { id: 'F1', name: 'Báo cáo.docx' },
  allowedOrigins: ['https://drive.example.vn'],
};

function server() {
  const calls: { url: string; method: string; headers: Record<string, string> }[] = [];
  const fetchImpl: FetchLike = async (url, init) => {
    calls.push({ url, method: init?.method ?? 'GET', headers: init?.headers ?? {} });
    const body = url.endsWith('/api/sessions')
      ? session
      : url.includes('/checkpoint')
        ? { version: '7' }
        : url.endsWith('/api/hosting/origins')
          ? { origins: ['https://drive.example.vn', '*', 3] }
          : null;
    return {
      ok: true,
      status: 200,
      json: async () => body,
      arrayBuffer: async () => new Uint8Array([80, 75, 3, 4]).buffer,
    };
  };
  return { fetchImpl, calls };
}

class Shell {
  recentFiles = async () => [];
  async addRecentFile() {}
  onOpenRequest() {
    return () => {};
  }
}

function basePlatform(): Platform {
  return {
    files: { openFile: async () => null, saveFile: vi.fn(async (_b, n: string) => n) },
    // A class instance, like the real web/desktop shells.
    shell: new Shell(),
    engine: {
      saveDocx: async () => new Uint8Array([1]),
      savePptx: async () => new Uint8Array([2]),
      exportPdf: async () => new Uint8Array([37]),
    },
    print: { printPdf: vi.fn(async () => {}) },
  } as unknown as Platform;
}

describe('hosted session', () => {
  it('picks the app from the file name', () => {
    expect(appForFile('a.DOCX')).toBe('doc');
    expect(appForFile('bảng.xlsx')).toBe('sheet');
    expect(appForFile('x.ods')).toBe('sheet');
    expect(appForFile('deck.pptx')).toBe('slide');
    expect(serverBase(' https://office.example.vn// ')).toBe('https://office.example.vn');
  });

  it('creates the session with the host token and downloads the file', async () => {
    const { fetchImpl, calls } = server();
    const r = await openSession(
      'https://office.example.vn',
      { fileId: 'F1', accessToken: 'jwt', host: 'bluedrive' },
      fetchImpl,
    );
    expect(calls[0]).toMatchObject({
      url: 'https://office.example.vn/api/sessions',
      method: 'POST',
    });
    expect(calls[1]!.url).toBe('https://office.example.vn/api/sessions/S%201/contents');
    expect(calls[1]!.headers['x-session-token']).toBe('secret');
    expect(r.file).toEqual({ name: 'Báo cáo.docx', bytes: new Uint8Array([80, 75, 3, 4]) });
  });

  it('reads the embedding origins from the server (never "*")', async () => {
    const { fetchImpl } = server();
    await expect(fetchAllowedOrigins('', fetchImpl)).resolves.toEqual(['https://drive.example.vn']);
  });

  it('opens the session file and saves to the host, other formats go to the disk', async () => {
    const { fetchImpl, calls } = server();
    const saved: unknown[] = [];
    const base = basePlatform();
    const p = hostedPlatform(base, {
      api: '',
      session,
      file: { name: 'Báo cáo.docx', bytes: new Uint8Array([1]) },
      app: 'doc',
      mode: new SessionMode('edit'),
      fetchImpl,
      onSaved: (r) => saved.push(r),
    });
    const opened = await new Promise<string>((res) => p.shell.onOpenRequest((f) => res(f.name)));
    expect(opened).toBe('Báo cáo.docx');
    await expect(p.shell.recentFiles()).resolves.toEqual([]);
    await p.files.saveFile(new Uint8Array([9]), 'Báo cáo.docx', '');
    expect(calls.at(-1)!.url).toBe('/api/sessions/S%201/checkpoint?save=true');
    expect(saved).toEqual([{ version: '7' }]);
    await p.files.saveFile(new Uint8Array([9]), 'Báo cáo.pdf', 'application/pdf');
    expect(base.files.saveFile).toHaveBeenCalledWith(
      expect.any(Uint8Array),
      'Báo cáo.pdf',
      'application/pdf',
    );

    // Host commands.
    const run = commandHandler(p, 'doc', 'Báo cáo.docx');
    await expect(run('save')).resolves.toEqual({ saved: true });
    await run('print');
    expect(base.print.printPdf).toHaveBeenCalled();
  });

  it('refuses to save once the session is no longer editable', async () => {
    const { fetchImpl } = server();
    const mode = new SessionMode('edit');
    const p = hostedPlatform(basePlatform(), {
      api: '',
      session,
      file: { name: 'a.pptx', bytes: new Uint8Array() },
      app: 'slide',
      mode,
      fetchImpl,
    });
    await expect(p.files.saveFile(new Uint8Array(), 'a.pptx', '')).resolves.toBe('a.pptx');
    mode.set('view');
    await expect(p.files.saveFile(new Uint8Array(), 'a.pptx', '')).rejects.toThrow('readOnly');
  });
});

describe('session mode', () => {
  it('notifies changes; revoked is final', () => {
    const m = new SessionMode('edit');
    const seen: string[] = [];
    m.subscribe((v) => seen.push(v));
    m.set('edit');
    m.set('view');
    m.set('edit');
    m.set('revoked');
    m.set('edit');
    expect(seen).toEqual(['view', 'edit', 'revoked']);
    expect(m.editable).toBe(false);
    expect(parseMode('none')).toBe('revoked');
    expect(parseMode('weird')).toBe('view');
    expect(parseMode('comment')).toBe('comment');
  });
});
