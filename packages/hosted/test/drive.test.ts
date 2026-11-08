import { describe, expect, it } from 'vitest';
import {
  DriveClient,
  DriveError,
  beginSignIn,
  completeSignIn,
  listHosts,
  parseCallback,
  parseSyncFolders,
  pkceChallenge,
  syncFolderOf,
  type FetchLike,
} from '../src';

type Route = (url: string, init?: Parameters<FetchLike>[1]) => { status?: number; body?: unknown };

function fake(route: Route) {
  const calls: { url: string; init: Parameters<FetchLike>[1] }[] = [];
  const fetchImpl: FetchLike = async (url, init) => {
    calls.push({ url, init });
    const r = route(url, init);
    const status = r.status ?? 200;
    return {
      ok: status < 400,
      status,
      json: async () => r.body,
      arrayBuffer: async () => new ArrayBuffer(0),
    };
  };
  return { fetchImpl, calls };
}

describe('BlueDrive browsing through the server', () => {
  it('lists hosts with their sign-in settings', async () => {
    const { fetchImpl } = fake(() => ({
      body: {
        hosts: [
          {
            name: 'bluedrive',
            displayName: 'BlueDrive',
            protocol: 'bohp',
            browse: true,
            signIn: {
              authorizeUrl: 'https://sso/a',
              tokenUrl: 'https://sso/t',
              clientId: 'c',
              scope: '',
            },
          },
          { name: 'wopi', protocol: 'wopi', browse: false, signIn: null },
          { bogus: true },
        ],
      },
    }));
    const hosts = await listHosts('https://office', fetchImpl);
    expect(hosts).toHaveLength(2);
    expect(hosts[0]!.signIn?.scope).toBe('openid profile');
    expect(hosts[1]).toMatchObject({
      name: 'wopi',
      displayName: 'wopi',
      browse: false,
      signIn: null,
    });
  });

  it('sends the host token and maps listings', async () => {
    const { fetchImpl, calls } = fake((url) => {
      if (url.endsWith('/me')) return { body: { id: 'u2', name: 'Trần Thị Bình' } };
      if (url.includes('/folders/'))
        return {
          body: {
            folder: { id: 'd1', name: 'Dự án', parentId: 'root' },
            items: [
              { kind: 'folder', id: 'd2', name: 'Quý 4' },
              { kind: 'file', id: 'f1', name: 'Kế hoạch.docx', version: '3', size: 10 },
              { name: 'no id' },
            ],
          },
        };
      if (url.includes('/resolve'))
        return url.includes('missing') ? { status: 404 } : { body: { id: 'f9', name: 'a.docx' } };
      if (url.includes('/recent')) return { status: 401 };
      return { body: { items: [{ id: 'f3', name: 'Chia sẻ.xlsx' }] } };
    });
    const c = new DriveClient('https://office', 'bluedrive', 'jwt', fetchImpl);
    expect((await c.me()).name).toBe('Trần Thị Bình');
    const l = await c.children('d1');
    expect(l.folder.parentId).toBe('root');
    expect(l.items.map((i) => `${i.kind}:${i.name}`)).toEqual([
      'folder:Quý 4',
      'file:Kế hoạch.docx',
    ]);
    expect(calls[1]!.url).toBe('https://office/api/hosts/bluedrive/folders/d1/children');
    expect(calls[1]!.init?.headers).toEqual({ authorization: 'Bearer jwt' });
    expect((await c.shared())[0]!.kind).toBe('file');
    expect(await c.resolve('Dự án/a.docx')).toEqual({ id: 'f9', name: 'a.docx' });
    expect(calls.at(-1)!.url).toContain(`path=${encodeURIComponent('Dự án/a.docx')}`);
    expect(await c.resolve('missing.docx')).toBeNull();
    await expect(c.recent()).rejects.toBeInstanceOf(DriveError);
  });
});

describe('sign-in (OAuth 2.0 code + PKCE)', () => {
  const cfg = {
    authorizeUrl: 'https://sso/auth',
    tokenUrl: 'https://sso/token',
    clientId: 'desk',
    scope: 'openid',
  };

  it('computes the S256 challenge as BASE64URL(SHA256(verifier))', async () => {
    const verifier = 'dBjftJeZ4CVP-mJ92K9yXIvAPPa2avWaHYaa4IibgyU';
    // Reference value: Node `createHash('sha256').update(verifier).digest('base64url')`.
    expect(await pkceChallenge(verifier)).toBe('LbiC3PXhSMi6mESlINDCRYqtI460Ph9e7yGndbR8iag');
  });

  it('builds the authorization URL and exchanges the code after checking state', async () => {
    const p = await beginSignIn(cfg, 'bluedoc://auth', 'u2');
    const u = new URL(p.url);
    expect(u.origin + u.pathname).toBe('https://sso/auth');
    expect(u.searchParams.get('code_challenge')).toBe(await pkceChallenge(p.verifier));
    expect(u.searchParams.get('code_challenge_method')).toBe('S256');
    expect(u.searchParams.get('redirect_uri')).toBe('bluedoc://auth');
    expect(u.searchParams.get('login_hint')).toBe('u2');
    expect(p.verifier.length).toBeGreaterThanOrEqual(43);

    const { fetchImpl, calls } = fake(() => ({ body: { access_token: 'host-jwt' } }));
    const token = await completeSignIn(
      cfg,
      p,
      `bluedoc://auth?code=abc&state=${p.state}`,
      fetchImpl,
    );
    expect(token).toBe('host-jwt');
    const form = new URLSearchParams(String(calls[0]!.init?.body));
    expect(form.get('code_verifier')).toBe(p.verifier);
    expect(form.get('grant_type')).toBe('authorization_code');

    await expect(
      completeSignIn(cfg, p, 'bluedoc://auth?code=abc&state=other', fetchImpl),
    ).rejects.toThrow('signIn.state');
    await expect(
      completeSignIn(cfg, p, 'bluedoc://auth?error=access_denied', fetchImpl),
    ).rejects.toThrow('signIn.access_denied');
    expect(parseCallback('bluedoc://auth')).toEqual({});
  });
});

describe('sync folders', () => {
  const folders = [
    { host: 'bluedrive', path: 'C:\\Users\\An\\BlueDrive' },
    { host: 'bluedrive', path: 'C:\\Users\\An\\BlueDrive\\Dự án' },
    { host: 'other', path: '/home/an/Drive' },
  ];

  it('finds the most specific folder and the relative path', () => {
    expect(syncFolderOf('c:\\users\\an\\bluedrive\\Dự án\\Q4\\Kế hoạch.docx', folders)).toEqual({
      folder: folders[1],
      relative: 'Q4/Kế hoạch.docx',
    });
    expect(syncFolderOf('C:/Users/An/BlueDrive/a.docx', folders)?.relative).toBe('a.docx');
    expect(syncFolderOf('/home/an/Drive/x/y.xlsx', folders)?.folder.host).toBe('other');
    // Case matters outside Windows; prefixes must end at a separator; `..` is refused.
    expect(syncFolderOf('/home/an/drive/y.xlsx', folders)).toBeNull();
    expect(syncFolderOf('C:\\Users\\An\\BlueDrive2\\a.docx', folders)).toBeNull();
    expect(syncFolderOf('C:\\Users\\An\\BlueDrive\\..\\secret.docx', folders)).toBeNull();
    expect(syncFolderOf('C:\\Users\\An\\BlueDrive', folders)).toBeNull();
  });

  it('parses the stored setting defensively', () => {
    expect(parseSyncFolders('[{"host":"bluedrive","path":" D:\\\\Sync "},{"host":1},"x"]')).toEqual(
      [{ host: 'bluedrive', path: 'D:\\Sync' }],
    );
    expect(parseSyncFolders('{')).toEqual([]);
    expect(parseSyncFolders(null)).toEqual([]);
  });
});
