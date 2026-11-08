import { describe, expect, it, vi } from 'vitest';
import type { OfflineCache, Platform } from '@blueoffice/platform';
import {
  OfflineSaves,
  SessionMode,
  hostedPlatform,
  offlinePlatform,
  openHosted,
  type FetchLike,
  type SyncOutcome,
} from '../src';

class MemoryCache implements OfflineCache {
  readonly map = new Map<string, Uint8Array>();
  async get(k: string) {
    return this.map.get(k) ?? null;
  }
  async put(k: string, v: Uint8Array) {
    this.map.set(k, v);
  }
  async delete(k: string) {
    this.map.delete(k);
  }
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
    shell: new Shell(),
    storage: { id: 'local', open: async () => null, save: vi.fn(), saveAs: vi.fn() },
    engine: {
      saveDocx: async () => new Uint8Array([1]),
      exportPdf: async () => new Uint8Array([37]),
    },
    print: { printPdf: vi.fn(async () => {}) },
  } as unknown as Platform;
}

const session = (version = '1') => ({
  sessionId: 'S1',
  sessionToken: 'secret',
  mode: 'edit' as const,
  file: { id: 'f7', name: 'Báo cáo.docx', version },
  user: { id: 'u2', name: 'Trần Thị Bình' },
  allowedOrigins: [],
});

type Reply = { status?: number; body?: unknown; bytes?: number[] } | 'offline';

function server(route: (url: string, method: string) => Reply) {
  const calls: { url: string; method: string; body?: unknown }[] = [];
  const fetchImpl: FetchLike = async (url, init) => {
    const method = init?.method ?? 'GET';
    calls.push({ url, method, body: init?.body });
    const r = route(url, method);
    if (r === 'offline') throw new TypeError('Failed to fetch');
    const status = r.status ?? 200;
    return {
      ok: status < 400,
      status,
      json: async () => r.body ?? null,
      arrayBuffer: async () => new Uint8Array(r.bytes ?? [80, 75]).buffer,
    };
  };
  return { fetchImpl, calls };
}

describe('offline saves of host files', () => {
  it('keeps a save locally when the server is unreachable, then syncs on the next open', async () => {
    const cache = new MemoryCache();
    const saves = new OfflineSaves(cache);
    let up = false;
    const { fetchImpl } = server(() => (up ? { body: { version: '2' } } : 'offline'));
    const onSavedOffline = vi.fn();
    const p = hostedPlatform(basePlatform(), {
      api: 'https://office',
      session: session('1'),
      file: { name: 'Báo cáo.docx', bytes: new Uint8Array([1]) },
      app: 'doc',
      mode: new SessionMode('edit'),
      fetchImpl,
      offline: { saves, host: 'bluedrive', isOffline: () => false, onSavedOffline },
    });
    await p.files.saveFile(new Uint8Array([9, 9]), 'Báo cáo.docx', '');
    expect(onSavedOffline).toHaveBeenCalledOnce();
    const pending = await saves.get('bluedrive', 'f7');
    expect(pending?.meta).toMatchObject({ baseVersion: '1', name: 'Báo cáo.docx' });
    expect([...pending!.bytes]).toEqual([9, 9]);

    // Online again: saves go to the host and advance the base version.
    up = true;
    await p.files.saveFile(new Uint8Array([3]), 'Báo cáo.docx', '');
    up = false;
    await p.files.saveFile(new Uint8Array([4]), 'Báo cáo.docx', '');
    expect((await saves.get('bluedrive', 'f7'))?.meta.baseVersion).toBe('2');
  });

  it('opening a file with a pending save syncs it: a newer host version yields a conflict copy', async () => {
    const saves = new OfflineSaves(new MemoryCache());
    await saves.put(
      { host: 'bluedrive', fileId: 'f7', name: 'Báo cáo.docx', baseVersion: '1', savedAt: 1 },
      new Uint8Array([5, 5]),
    );
    const { fetchImpl, calls } = server((url) => {
      if (url.endsWith('/api/sessions')) return { body: session('2') };
      if (url.includes('/sync'))
        return {
          body: {
            outcome: 'conflict-copy',
            version: '2',
            conflictCopy: {
              id: 'f8',
              name: 'Báo cáo (xung đột - Trần Thị Bình - 2026-11-08 09-00).docx',
            },
          },
        };
      return { bytes: [80, 75, 3, 4] };
    });
    const synced: SyncOutcome[] = [];
    const h = await openHosted({
      platform: basePlatform(),
      api: 'https://office',
      base: 'https://office',
      ref: { host: 'bluedrive', fileId: 'f7', accessToken: 'jwt' },
      fetchImpl,
      offline: saves,
      onOfflineSync: (r) => synced.push(r),
    });
    const sync = calls.find((c) => c.url.includes('/sync'))!;
    expect(sync.url).toBe('https://office/api/sessions/S1/sync?baseVersion=1');
    expect([...(sync.body as Uint8Array)]).toEqual([5, 5]);
    expect(synced[0]?.conflictCopy?.name).toContain('(xung đột - Trần Thị Bình - ');
    expect(await saves.get('bluedrive', 'f7')).toBeNull();
    // The host version is opened (and cached for offline use).
    expect([...h.file.bytes]).toEqual([80, 75, 3, 4]);
    expect((await saves.getCopy('bluedrive', 'f7'))?.meta.version).toBe('2');
  });

  it('a pending save on an unchanged file becomes the new version and is reopened', async () => {
    const saves = new OfflineSaves(new MemoryCache());
    await saves.put(
      { host: 'bluedrive', fileId: 'f7', name: 'Báo cáo.docx', baseVersion: '1', savedAt: 1 },
      new Uint8Array([5]),
    );
    let opened = 0;
    const { fetchImpl, calls } = server((url, method) => {
      if (url.endsWith('/api/sessions')) return { body: session(String(++opened)) };
      if (url.includes('/sync')) return { body: { outcome: 'saved', version: '2' } };
      if (method === 'DELETE') return { status: 204 };
      return { bytes: [opened] };
    });
    const synced: SyncOutcome[] = [];
    const h = await openHosted({
      platform: basePlatform(),
      api: 'https://office',
      base: 'https://office',
      ref: { host: 'bluedrive', fileId: 'f7', accessToken: 'jwt' },
      fetchImpl,
      offline: saves,
      onOfflineSync: (r) => synced.push(r),
    });
    expect(synced[0]).toMatchObject({ outcome: 'saved', version: '2' });
    expect(calls.some((c) => c.method === 'DELETE')).toBe(true);
    expect(h.session.file.version).toBe('2');
    expect([...h.file.bytes]).toEqual([2]);
  });

  it('edits the cached copy offline and keeps saves pending', async () => {
    const saves = new OfflineSaves(new MemoryCache());
    expect(await saves.getCopy('bluedrive', 'f7')).toBeNull();
    await saves.putCopy(
      { host: 'bluedrive', fileId: 'f7', name: 'Báo cáo.docx', version: '4' },
      new Uint8Array([1]),
    );
    const copy = (await saves.getCopy('bluedrive', 'f7'))!;
    const base = basePlatform();
    const p = offlinePlatform(base, saves, copy);
    const opened = await new Promise<string>((r) => p.shell.onOpenRequest((f) => r(f.name)));
    expect(opened).toBe('Báo cáo.docx');
    await p.files.saveFile(new Uint8Array([2]), 'Báo cáo.docx', '');
    expect((await saves.get('bluedrive', 'f7'))?.meta.baseVersion).toBe('4');
    // The pending save is now the newest local content.
    expect([...(await saves.getCopy('bluedrive', 'f7'))!.bytes]).toEqual([2]);
    // Other formats still go to the disk.
    await p.files.saveFile(new Uint8Array([37]), 'Báo cáo.pdf', 'application/pdf');
    expect(base.files.saveFile).toHaveBeenCalledWith(
      new Uint8Array([37]),
      'Báo cáo.pdf',
      'application/pdf',
    );
  });
});

describe('audit of host files', () => {
  it('reports prints and exports to the session audit endpoint', async () => {
    const { fetchImpl, calls } = server(() => ({ status: 204 }));
    const base = basePlatform();
    const p = hostedPlatform(base, {
      api: 'https://office',
      session: session(),
      file: { name: 'Báo cáo.docx', bytes: new Uint8Array([1]) },
      app: 'doc',
      mode: new SessionMode('edit'),
      fetchImpl,
    });
    await p.print.printPdf(new Uint8Array([37]), 'Báo cáo.docx');
    expect(base.print.printPdf).toHaveBeenCalled();
    await p.files.saveFile(new Uint8Array([37]), 'Báo cáo.pdf', 'application/pdf');
    await new Promise((r) => setTimeout(r, 0));
    const audits = calls.filter((c) => c.url.endsWith('/api/sessions/S1/audit'));
    expect(audits.map((c) => JSON.parse(String(c.body)) as unknown)).toEqual([
      { action: 'print', detail: 'Báo cáo.docx' },
      { action: 'export', detail: 'pdf' },
    ]);
  });

  it('close saves unsaved changes first (SDK `close`), not when the host asked for view mode', async () => {
    const { fetchImpl, calls } = server((url) =>
      url.endsWith('/api/sessions')
        ? { body: session('1') }
        : url.includes('/checkpoint')
          ? { body: { version: '2' } }
          : { bytes: [80, 75] },
    );
    const withEngine = () => {
      const p = basePlatform();
      (p.engine as unknown as Record<string, unknown>).command = async (c: { cmd: string }) => ({
        cmd: c.cmd,
      });
      return p;
    };
    const saved: unknown[] = [];
    const h = await openHosted({
      platform: withEngine(),
      api: 'https://office',
      base: 'https://office',
      ref: { host: 'bluedrive', fileId: 'f7', accessToken: 'jwt' },
      fetchImpl,
      onSaved: (r) => saved.push(r),
    });
    const dirty: boolean[] = [];
    h.dirty.subscribe((d) => dirty.push(d));
    await h.platform.engine.command({ cmd: 'insertText', text: 'a' });
    await expect(h.command('close')).resolves.toEqual({ saved: true });
    expect(calls.filter((c) => c.url.includes('/checkpoint?save=true'))).toHaveLength(1);
    expect(dirty).toEqual([true, false]);
    expect(saved).toEqual([{ version: '2' }]);
    // Closing twice does nothing more.
    await expect(h.close()).resolves.toEqual({ saved: true });

    const v = await openHosted({
      platform: withEngine(),
      api: 'https://office',
      base: 'https://office',
      ref: { host: 'bluedrive', fileId: 'f7', accessToken: 'jwt' },
      requestedMode: 'view',
      fetchImpl,
    });
    expect(v.mode.value).toBe('view');
    await v.platform.engine.command({ cmd: 'insertText', text: 'a' });
    expect(v.dirty.dirty).toBe(false);
    await expect(v.close()).resolves.toEqual({ saved: false });
    expect(calls.filter((c) => c.url.includes('/checkpoint'))).toHaveLength(1);
  });
});
