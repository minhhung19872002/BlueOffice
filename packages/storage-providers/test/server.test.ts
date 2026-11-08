import { describe, expect, it } from 'vitest';
import {
  BlueDriveProvider,
  BlueOfficeServerProvider,
  StorageConflictError,
  StorageError,
  WopiProvider,
  type ProviderFetch,
} from '../src';

interface Call {
  method: string;
  url: string;
  headers: Record<string, string>;
  body: unknown;
}

/**
 * In-process stand-in for the BlueOffice server routes the providers use:
 * `/api/files` (versions + If-Match) and `/api/sessions` (BOHP / WOPI host
 * files with checkpoint, save-as, browse proxy). The real server is exercised
 * by the Playwright collaboration suite (`collab-storage.spec.ts`).
 */
function fakeServer() {
  const calls: Call[] = [];
  const files = new Map<string, { name: string; version: number; bytes: Uint8Array }>([
    [
      '11111111-1111-1111-1111-111111111111',
      { name: 'Kế hoạch.docx', version: 3, bytes: new Uint8Array([1]) },
    ],
  ]);
  const host = new Map<string, { name: string; version: number; bytes: Uint8Array }>([
    ['f42', { name: 'Báo cáo.xlsx', version: 7, bytes: new Uint8Array([7]) }],
  ]);
  let seq = 0;
  const sessions = new Map<string, { fileId: string; token: string; wopi: boolean }>();
  const fetchImpl: ProviderFetch = async (url, init) => {
    const method = init?.method ?? 'GET';
    const headers = init?.headers ?? {};
    calls.push({ method, url, headers, body: init?.body });
    const u = new URL(url);
    const reply = (status: number, body: unknown, bytes?: Uint8Array, etag?: string) => ({
      ok: status < 400,
      status,
      headers: { get: (n: string) => (n.toLowerCase() === 'etag' ? (etag ?? null) : null) },
      json: async () => body,
      arrayBuffer: async () => (bytes ?? new Uint8Array()).slice().buffer,
    });
    const path = u.pathname;
    // --- /api/files (user token)
    if (path.startsWith('/api/files') || path.startsWith('/api/folders')) {
      if (headers.authorization !== 'Bearer user-jwt') return reply(401, null);
      const id = path.split('/')[3] ?? '';
      const f = files.get(id);
      const dto = (fid: string) => {
        const x = files.get(fid)!;
        return {
          id: fid,
          name: x.name,
          folderId: null,
          size: x.bytes.length,
          version: x.version,
          role: 'editor',
        };
      };
      if (path === '/api/folders')
        return reply(200, { folder: null, folders: [], files: [...files.keys()].map(dto) });
      if (path === '/api/files' && method === 'POST') {
        const nid = `22222222-0000-0000-0000-00000000000${++seq}`;
        files.set(nid, {
          name: u.searchParams.get('name')!,
          version: 1,
          bytes: init?.body as Uint8Array,
        });
        return reply(201, dto(nid));
      }
      if (!f) return reply(404, null);
      if (path.endsWith('/content') && method === 'GET')
        return reply(200, null, f.bytes, `"${f.version}"`);
      if (path.endsWith('/content') && method === 'PUT') {
        if (headers['if-match'] && headers['if-match'] !== `"${f.version}"`)
          return reply(409, { title: 'Version conflict' });
        f.version += 1;
        f.bytes = init?.body as Uint8Array;
        return reply(200, dto(id));
      }
      return reply(200, dto(id));
    }
    // --- sessions
    if (path === '/api/sessions' && method === 'POST') {
      const b = JSON.parse(String(init?.body)) as Record<string, string>;
      const wopi = !!b.wopiSrc;
      const fileId = wopi ? 'w1' : b.fileId!;
      if (!wopi && b.token !== 'host-jwt') return reply(403, null);
      if (wopi && b.token !== 'wopi-token') return reply(401, null);
      if (!wopi && !host.has(fileId)) return reply(404, null);
      if (wopi && !host.has('w1'))
        host.set('w1', { name: 'Hợp đồng.docx', version: 1, bytes: new Uint8Array([9]) });
      const sid = `S${++seq}`;
      sessions.set(sid, { fileId, token: `tok-${sid}`, wopi });
      const f = host.get(fileId)!;
      return reply(201, {
        sessionId: sid,
        sessionToken: `tok-${sid}`,
        mode: 'edit',
        file: { id: fileId, name: f.name, version: String(f.version) },
        permissions: { view: true, comment: true, edit: true, share: false, download: true },
      });
    }
    const m = /^\/api\/sessions\/([^/]+)(\/.*)?$/.exec(path);
    if (m) {
      const s = sessions.get(m[1]!);
      if (!s || headers['x-session-token'] !== s.token) return reply(401, null);
      const f = host.get(s.fileId)!;
      switch (`${method} ${m[2] ?? ''}`) {
        case 'GET /contents':
          return reply(200, null, f.bytes);
        case 'POST /checkpoint':
          f.version += 1;
          f.bytes = init?.body as Uint8Array;
          return reply(200, { saved: true, version: String(f.version) });
        case 'POST /save-as': {
          const name = u.searchParams.get('name')!;
          if ([...host.values()].some((x) => x.name === name))
            return reply(409, { error: 'name-conflict' });
          host.set(`n${++seq}`, { name, version: 1, bytes: init?.body as Uint8Array });
          return reply(200, { id: `n${seq}`, name, version: '1' });
        }
        case 'DELETE ':
          sessions.delete(m[1]!);
          return reply(204, null);
        default:
          return reply(404, null);
      }
    }
    // --- browse proxy
    if (path.startsWith('/api/hosts/bluedrive/')) {
      if (headers.authorization !== 'Bearer host-jwt') return reply(401, null);
      if (path.endsWith('/folders/root/children'))
        return reply(200, {
          folder: { id: 'root', name: 'Tệp của tôi', parentId: null },
          items: [{ kind: 'file', id: 'f42', name: 'Báo cáo.xlsx', version: '7' }],
        });
      if (path.endsWith('/recent'))
        return reply(200, { items: [{ kind: 'file', id: 'f42', name: 'Báo cáo.xlsx' }] });
      if (path.endsWith('/me')) return reply(200, { id: 'u1', name: 'Nguyễn Văn An' });
    }
    return reply(404, null);
  };
  return { fetchImpl, calls, files, host, sessions };
}

describe('BlueOfficeServerProvider (/api/files)', () => {
  const auth = (t: string | null) => ({ getAccessToken: async () => t });

  it('opens with the ETag version and saves with If-Match', async () => {
    const s = fakeServer();
    const p = new BlueOfficeServerProvider({
      api: 'https://office/',
      auth: auth('user-jwt'),
      fetch: s.fetchImpl,
    });
    const listed = await p.list();
    expect(listed.files.map((f) => f.name)).toEqual(['Kế hoạch.docx']);
    const id = listed.files[0]!.id;
    const doc = (await p.open({ provider: p.id, id, name: 'Kế hoạch.docx' }))!;
    expect(doc).toMatchObject({ version: '3', permissions: { edit: true, share: false } });
    expect([...doc.bytes]).toEqual([1]);
    const saved = await p.save(doc, new Uint8Array([2]));
    expect(saved.version).toBe('4');
    expect(s.calls.at(-1)!.headers['if-match']).toBe('"3"');
    // Saving the stale copy again: someone else's version 4 is not overwritten.
    await expect(p.save(doc, new Uint8Array([3]))).rejects.toBeInstanceOf(StorageConflictError);
    expect([...s.files.get(id)!.bytes]).toEqual([2]);
  });

  it('creates new files and requires a signed-in user', async () => {
    const s = fakeServer();
    const p = new BlueOfficeServerProvider({
      api: 'https://office',
      auth: auth('user-jwt'),
      fetch: s.fetchImpl,
    });
    const created = (await p.saveAs('Biên bản.docx', new Uint8Array([5])))!;
    expect(created.ref.name).toBe('Biên bản.docx');
    expect(s.calls.at(-1)!.url).toBe('https://office/api/files?name=Bi%C3%AAn+b%E1%BA%A3n.docx');
    const anon = new BlueOfficeServerProvider({
      api: 'https://office',
      auth: auth(null),
      fetch: s.fetchImpl,
    });
    await expect(anon.list()).rejects.toBeInstanceOf(StorageError);
    expect(await anon.open()).toBeNull();
  });
});

describe('BlueDriveProvider (BOHP through the server)', () => {
  it('browses, opens a session, saves versions, saves as and closes', async () => {
    const s = fakeServer();
    const p = new BlueDriveProvider({
      api: 'https://office',
      token: async () => 'host-jwt',
      fetch: s.fetchImpl,
    });
    expect((await p.me()).name).toBe('Nguyễn Văn An');
    const items = (await p.list()).items;
    expect(items[0]).toMatchObject({ id: 'f42', kind: 'file' });
    expect((await p.recent()).length).toBe(1);
    const doc = (await p.open({ provider: p.id, id: 'f42', name: 'Báo cáo.xlsx' }))!;
    expect(doc).toMatchObject({ version: '7', permissions: { edit: true, download: true } });
    // The token goes to the server only (session create), never in a URL.
    expect(s.calls.every((c) => !c.url.includes('host-jwt'))).toBe(true);
    const saved = await p.save(doc, new Uint8Array([8]));
    expect(saved.version).toBe('8');
    expect(s.host.get('f42')!.version).toBe(8);
    const copy = (await p.saveAs('Báo cáo (bản sao).xlsx', new Uint8Array([9])))!;
    expect(copy.ref.name).toBe('Báo cáo (bản sao).xlsx');
    await expect(p.saveAs('Báo cáo.xlsx', new Uint8Array())).rejects.toBeInstanceOf(
      StorageConflictError,
    );
    await p.close('f42');
    expect(s.sessions.size).toBe(0);
    await expect(p.save(doc, new Uint8Array())).rejects.toThrow('storage.noSession');
  });

  it('refuses a bad host token', async () => {
    const s = fakeServer();
    const p = new BlueDriveProvider({
      api: 'https://office',
      token: async () => 'forged',
      fetch: s.fetchImpl,
    });
    await expect(p.open({ provider: 'bluedrive', id: 'f42', name: 'x' })).rejects.toThrow(
      'storage.session 403',
    );
    await expect(p.list()).rejects.toThrow('storage.browse 401');
  });
});

describe('WopiProvider ([MS-WOPI] host through the server)', () => {
  it('opens a WOPISrc with its access token and saves through the session', async () => {
    const s = fakeServer();
    const src = 'https://cloud.example.vn/wopi/files/123';
    const p = new WopiProvider({
      api: 'https://office',
      token: async (w) => (w === src ? 'wopi-token' : null),
      fetch: s.fetchImpl,
    });
    const doc = (await p.open({ provider: 'wopi', id: src, name: 'Hợp đồng.docx' }))!;
    expect(JSON.parse(String(s.calls[0]!.body))).toEqual({ wopiSrc: src, token: 'wopi-token' });
    expect(doc.ref).toEqual({ provider: 'wopi', id: src, name: 'Hợp đồng.docx' });
    expect((await p.save(doc, new Uint8Array([4]))).version).toBe('2');
    await expect(
      p.open({ provider: 'wopi', id: 'https://other/wopi/files/9', name: 'x' }),
    ).rejects.toThrow('storage.signIn');
  });
});
