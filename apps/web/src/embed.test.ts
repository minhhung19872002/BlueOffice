import { describe, expect, it, vi } from 'vitest';
import type { HostConnection } from '@blueoffice/editor-sdk';
import type { Platform } from '@blueoffice/platform';
import { appForFile, commandHandler, embedPlatform, openSession, type FetchLike } from './embed';

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

function conn(): HostConnection & { events: [string, unknown][] } {
  const events: [string, unknown][] = [];
  return {
    events,
    ready: new Promise(() => {}),
    emit: (e, p) => events.push([e, p]),
    onCommand: () => {},
    close: () => {},
  };
}

function basePlatform(): Platform {
  return {
    files: { openFile: async () => null, saveFile: vi.fn(async (_b, n: string) => n) },
    shell: {
      recentFiles: async () => [],
      addRecentFile: async () => {},
      onOpenRequest: () => () => {},
    },
    engine: {
      saveDocx: async () => new Uint8Array([1]),
      savePptx: async () => new Uint8Array([2]),
      exportPdf: async () => new Uint8Array([37]),
    },
    print: { printPdf: vi.fn(async () => {}) },
  } as unknown as Platform;
}

describe('embedded mode', () => {
  it('picks the app from the file name', () => {
    expect(appForFile('a.DOCX')).toBe('doc');
    expect(appForFile('bảng.xlsx')).toBe('sheet');
    expect(appForFile('x.ods')).toBe('sheet');
    expect(appForFile('deck.pptx')).toBe('slide');
  });

  it('creates the session with the host token and downloads the file', async () => {
    const { fetchImpl, calls } = server();
    const r = await openSession(
      'https://office.example.vn',
      {
        fileId: 'F1',
        accessToken: 'jwt',
        host: 'bluedrive',
        mode: 'edit',
        lang: 'vi-VN',
        theme: 'auto',
        ui: {},
      },
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

  it('opens the session file and saves to the host, other formats download', async () => {
    const { fetchImpl, calls } = server();
    const c = conn();
    const base = basePlatform();
    const p = embedPlatform(base, {
      api: '',
      session,
      file: { name: 'Báo cáo.docx', bytes: new Uint8Array([1]) },
      app: 'doc',
      conn: c,
      fetchImpl,
    });
    const opened = await new Promise<string>((res) => p.shell.onOpenRequest((f) => res(f.name)));
    expect(opened).toBe('Báo cáo.docx');
    await p.files.saveFile(new Uint8Array([9]), 'Báo cáo.docx', '');
    expect(calls.at(-1)!.url).toBe('/api/sessions/S%201/checkpoint?save=true');
    expect(c.events).toEqual([['saved', { version: '7' }]]);
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

  it('refuses to save a view-only session', async () => {
    const { fetchImpl } = server();
    const p = embedPlatform(basePlatform(), {
      api: '',
      session: { ...session, mode: 'view' },
      file: { name: 'a.pptx', bytes: new Uint8Array() },
      app: 'slide',
      conn: conn(),
      fetchImpl,
    });
    await expect(p.files.saveFile(new Uint8Array(), 'a.pptx', '')).rejects.toThrow('readOnly');
  });
});
