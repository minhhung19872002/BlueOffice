import { describe, expect, it, vi } from 'vitest';
import { SessionMode, sessionVersions, type FetchLike } from '../src';

const session = () => ({
  sessionId: 'S 1',
  sessionToken: 'secret',
  mode: 'edit' as const,
  file: { id: 'F1', name: 'Bảng.xlsx', version: '3' },
  allowedOrigins: [],
});

function server() {
  const calls: { url: string; method: string; headers: Record<string, string> }[] = [];
  const fetchImpl: FetchLike = async (url, init) => {
    calls.push({ url, method: init?.method ?? 'GET', headers: init?.headers ?? {} });
    const missing = url.includes('/versions/9/');
    const body = url.endsWith('/versions')
      ? {
          versions: [
            {
              version: '3',
              size: 10,
              modifiedAt: '2026-11-10T08:30:00Z',
              modifiedBy: { id: 'u2', name: 'Bình' },
              note: null,
            },
            { version: '2', modifiedAt: null, modifiedBy: null, note: 'Restored' },
            { size: 1 },
          ],
        }
      : url.endsWith('/restore')
        ? { version: '4' }
        : null;
    return {
      ok: !missing,
      status: missing ? 404 : 200,
      json: async () => body,
      arrayBuffer: async () => new Uint8Array([80, 75, 3, 4]).buffer,
    };
  };
  return { fetchImpl, calls };
}

describe('session version history (CORE-VERSION-001)', () => {
  it('lists versions with author and time, reads one, restores one', async () => {
    const { fetchImpl, calls } = server();
    const s = session();
    const onRestored = vi.fn();
    const v = sessionVersions('', s, new SessionMode('edit'), fetchImpl, onRestored);
    expect(v.fileName).toBe('Bảng.xlsx');
    expect(v.canRestore()).toBe(true);
    const list = await v.list();
    expect(list).toEqual([
      {
        version: '3',
        size: 10,
        modifiedAt: '2026-11-10T08:30:00Z',
        modifiedBy: { id: 'u2', name: 'Bình' },
        note: null,
      },
      { version: '2', modifiedAt: null, modifiedBy: null, note: 'Restored' },
    ]);
    expect(calls[0]).toMatchObject({
      url: '/api/sessions/S%201/versions',
      headers: { 'x-session-token': 'secret' },
    });
    expect(Array.from(await v.contents('2'))).toEqual([80, 75, 3, 4]);
    expect(calls.at(-1)!.url).toBe('/api/sessions/S%201/versions/2/contents');
    await expect(v.contents('9')).rejects.toThrow('session.versionContents 404');
    await expect(v.restore('2')).resolves.toEqual({ version: '4' });
    expect(calls.at(-1)).toMatchObject({
      url: '/api/sessions/S%201/versions/2/restore',
      method: 'POST',
    });
    expect(s.file.version).toBe('4');
    expect(onRestored).toHaveBeenCalledWith('4');
  });

  it('a view session cannot restore', async () => {
    const { fetchImpl, calls } = server();
    const v = sessionVersions('', session(), new SessionMode('view'), fetchImpl);
    expect(v.canRestore()).toBe(false);
    await expect(v.restore('2')).rejects.toThrow('session.readOnly');
    expect(calls).toHaveLength(0);
  });
});
