import { createHash } from 'node:crypto';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DesktopAuth, createDesktopPlatform, isOidcCallback, type Invoke } from '../src/desktop';
import type { KeyValueStore, OidcFetch } from '../src/oidc';
import { shouldCheckForUpdates, updateNotice } from '../src/shared';

class MemoryStore implements KeyValueStore {
  map = new Map<string, string>();
  getItem(k: string) {
    return this.map.get(k) ?? null;
  }
  setItem(k: string, v: string) {
    this.map.set(k, v);
  }
  removeItem(k: string) {
    this.map.delete(k);
  }
}

describe('desktop native engine', () => {
  it('reports native initialization failure without falling back to WASM', async () => {
    const call = vi.fn(async (cmd: string) => {
      if (cmd === 'engine_init') return 0;
      throw new Error(`unexpected call ${cmd}`);
    }) as unknown as Invoke;
    const p = createDesktopPlatform({ invoke: call });
    expect(p.engine.backend).toBe('native');
    await expect(p.engine.init()).rejects.toThrow('engine.nativeUnavailable');
    await expect(p.sheet!.newWorkbook('b.xlsx')).rejects.toThrow('engine.nativeUnavailable');
  });

  it('uses native commands for BlueSheet', async () => {
    const calls: string[] = [];
    const call = vi.fn(async (cmd: string, args?: unknown) => {
      calls.push(cmd);
      if (cmd === 'engine_init') return 16;
      if (cmd === 'sheet_call') return { name: (args as { args: string[] }).args[0] };
      return null;
    }) as unknown as Invoke;
    const p = createDesktopPlatform({ invoke: call });
    expect((await p.sheet!.newWorkbook('c.xlsx')).name).toBe('c.xlsx');
    expect(calls).toEqual(['engine_init', 'sheet_call']);
  });
});

describe('desktop print, updates', () => {
  it('prints through the print window of the shell', async () => {
    const calls: { cmd: string; options: unknown }[] = [];
    const fake = vi.fn(async (cmd: string, _args?: unknown, options?: unknown) => {
      calls.push({ cmd, options });
      return cmd === 'print_pdf' ? 'print-3' : null;
    }) as unknown as Invoke;
    const p = createDesktopPlatform({ invoke: fake });
    await p.print.printPdf(new TextEncoder().encode('%PDF-1.7'), 'Báo cáo.docx');
    expect(calls[0]).toEqual({
      cmd: 'print_pdf',
      options: { headers: { 'x-file-name': encodeURIComponent('Báo cáo.docx') } },
    });
    expect((p.print as unknown as { lastWindow: string | null }).lastWindow).toBe('print-3');
  });

  it('reports update checks, failures included', async () => {
    let answer: unknown = { state: 'available', current: '0.1.0', version: '0.2.0' };
    const fake = vi.fn(async (cmd: string) => {
      if (cmd === 'update_check') {
        if (answer instanceof Error) throw answer;
        return answer;
      }
      if (cmd === 'update_install') throw new Error('signature mismatch');
      return null;
    }) as unknown as Invoke;
    const p = createDesktopPlatform({ invoke: fake });
    expect(await p.updates.check()).toEqual({
      state: 'available',
      current: '0.1.0',
      version: '0.2.0',
    });
    answer = new Error('offline');
    expect(await p.updates.check()).toEqual({ state: 'failed', message: 'offline' });
    await expect(p.updates.install()).rejects.toThrow('signature mismatch');
  });

  it('describes update results with i18n keys', () => {
    expect(updateNotice({ state: 'available', current: '0.1.0', version: '0.2.0' })).toEqual({
      key: 'updates.available',
      params: { version: '0.2.0', current: '0.1.0' },
    });
    expect(updateNotice({ state: 'upToDate', current: '0.1.0' }).key).toBe('updates.upToDate');
    expect(updateNotice({ state: 'failed', message: 'x' }).params).toEqual({ message: 'x' });
    expect(updateNotice({ state: 'notConfigured' }).key).toBe('updates.notConfigured');
    expect(updateNotice({ state: 'disabled' }).key).toBe('updates.disabled');
  });

  it('checks automatically at most once a day', () => {
    const day = 24 * 60 * 60 * 1000;
    expect(shouldCheckForUpdates(null, 1000)).toBe(true);
    expect(shouldCheckForUpdates(NaN, 1000)).toBe(true);
    expect(shouldCheckForUpdates(0, day - 1)).toBe(false);
    expect(shouldCheckForUpdates(0, day)).toBe(true);
    // Clock set back: check again.
    expect(shouldCheckForUpdates(5 * day, day)).toBe(true);
    expect(shouldCheckForUpdates(0, 1000, 500)).toBe(true);
  });
});

describe('desktop recent files', () => {
  const store = new MemoryStore();
  beforeEach(() => {
    store.map.clear();
    vi.stubGlobal('localStorage', store);
  });
  afterEach(() => vi.unstubAllGlobals());

  it('keeps the 20 most recent files, newest first, without duplicates', async () => {
    const p = createDesktopPlatform({ invoke: (async () => null) as unknown as Invoke });
    expect(await p.shell.recentFiles()).toEqual([]);
    for (let i = 0; i < 25; i++) {
      await p.shell.addRecentFile({ name: `f${i}.docx`, path: `C:\\d\\f${i}.docx`, openedAt: i });
    }
    await p.shell.addRecentFile({ name: 'f10.docx', path: 'C:\\d\\f10.docx', openedAt: 99 });
    const list = await p.shell.recentFiles();
    expect(list).toHaveLength(20);
    expect(list[0]).toEqual({ name: 'f10.docx', path: 'C:\\d\\f10.docx', openedAt: 99 });
    expect(list.filter((f) => f.path.endsWith('f10.docx'))).toHaveLength(1);
    expect(list[1]!.name).toBe('f24.docx');
    // Stored under the desktop key (the web app keeps its own list).
    expect([...store.map.keys()]).toEqual(['blueoffice.desktop.recent']);
  });

  it('survives a corrupt stored list', async () => {
    store.setItem('blueoffice.desktop.recent', '{not json');
    const p = createDesktopPlatform({ invoke: (async () => null) as unknown as Invoke });
    expect(await p.shell.recentFiles()).toEqual([]);
  });
});

// ---------------------------------------------------------------- OIDC sign-in (system browser)

const b64url = (s: string | Buffer) =>
  Buffer.from(s).toString('base64').replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
const jwt = (claims: Record<string, unknown>) =>
  `${b64url(JSON.stringify({ alg: 'none' }))}.${b64url(JSON.stringify(claims))}.sig`;

function provider() {
  const requests: { url: string; body?: string }[] = [];
  const fetch: OidcFetch = async (url, init) => {
    requests.push({ url, ...(init?.body ? { body: init.body } : {}) });
    const ok = (v: unknown) => ({ ok: true, status: 200, json: async () => v });
    if (url === 'https://office.example.vn/api/auth/config') {
      return ok({ oidc: { authority: 'https://sso.example.vn', clientId: 'blueoffice-desktop' } });
    }
    if (url === 'https://plain.example.vn/api/auth/config') return ok({ oidc: null });
    if (url === 'https://sso.example.vn/.well-known/openid-configuration') {
      return ok({
        authorization_endpoint: 'https://sso.example.vn/authorize',
        token_endpoint: 'https://sso.example.vn/token',
      });
    }
    if (url === 'https://sso.example.vn/token') {
      return ok({
        access_token: jwt({ sub: 'u7', exp: 4_000_000_000 }),
        id_token: jwt({ sub: 'u7', name: 'Nguyễn Văn An' }),
        refresh_token: 'r1',
        expires_in: 3600,
      });
    }
    return { ok: false, status: 404, json: async () => null };
  };
  return { fetch, requests };
}

function shell() {
  const opened: string[] = [];
  let handler: ((url: string) => void) | null = null;
  return {
    opened,
    fire: (url: string) => handler?.(url),
    shell: {
      openExternal: async (url: string) => void opened.push(url),
      onAuthCallback: (h: (url: string) => void) => {
        handler = h;
        return () => {
          handler = null;
        };
      },
    },
  };
}

describe('DesktopAuth (OIDC + PKCE through the system browser)', () => {
  it('recognises its own redirect, not the BlueDrive one', () => {
    expect(isOidcCallback('bluedoc://auth/oidc?code=1&state=2')).toBe(true);
    expect(isOidcCallback('BlueSheet://auth/oidc/?code=1')).toBe(true);
    expect(isOidcCallback('bluedoc://auth?code=1')).toBe(false);
    expect(isOidcCallback('https://x/auth/oidc')).toBe(false);
  });

  it('signs in with the server provider and the bluedoc://auth/oidc deep link', async () => {
    const idp = provider();
    const sh = shell();
    const storage = new MemoryStore();
    const auth = new DesktopAuth({
      scheme: 'bluesheet',
      shell: sh.shell,
      fetch: idp.fetch,
      storage,
    });
    expect(auth.enabled).toBe(false);
    expect(await auth.configure('https://plain.example.vn')).toBe(false);
    expect(await auth.configure('https://office.example.vn/')).toBe(true);
    const users: (string | null)[] = [];
    auth.onChange((u) => users.push(u?.name ?? null));

    await auth.signIn();
    const url = new URL(sh.opened[0]!);
    expect(url.origin + url.pathname).toBe('https://sso.example.vn/authorize');
    const q = url.searchParams;
    expect(q.get('redirect_uri')).toBe('bluesheet://auth/oidc');
    expect(q.get('client_id')).toBe('blueoffice-desktop');
    expect(q.get('code_challenge_method')).toBe('S256');
    // The verifier stays in the app; the challenge is its SHA-256.
    const pending = JSON.parse(
      [...storage.map.entries()].find(([k]) => k.endsWith('blueoffice.oidc.pending'))![1],
    ) as { verifier: string; state: string };
    expect(q.get('code_challenge')).toBe(
      b64url(createHash('sha256').update(pending.verifier).digest()),
    );

    // A BlueDrive callback is not ours; the OIDC one completes the sign-in.
    sh.fire('bluesheet://auth?code=drive&state=x');
    sh.fire(`bluesheet://auth/oidc?code=c1&state=${q.get('state')}`);
    await vi.waitFor(() => expect(auth.user?.name).toBe('Nguyễn Văn An'));
    const token = idp.requests.find((r) => r.url === 'https://sso.example.vn/token')!;
    const form = new URLSearchParams(token.body);
    expect(form.get('code')).toBe('c1');
    expect(form.get('code_verifier')).toBe(pending.verifier);
    expect(form.get('redirect_uri')).toBe('bluesheet://auth/oidc');
    expect(users).toEqual(['Nguyễn Văn An']);
    expect(await auth.getAccessToken()).toMatch(/\./);
    // Tokens are kept per server.
    expect(
      [...storage.map.keys()].every((k) =>
        k.startsWith('blueoffice.desktop.https://office.example.vn|'),
      ),
    ).toBe(true);

    // Another server: anonymous there.
    await auth.configure('https://plain.example.vn');
    expect(auth.user).toBeNull();
    expect(users.at(-1)).toBeNull();
  });

  it('reports a callback with a wrong state', async () => {
    const idp = provider();
    const sh = shell();
    const auth = new DesktopAuth({
      scheme: 'bluedoc',
      shell: sh.shell,
      fetch: idp.fetch,
      storage: new MemoryStore(),
    });
    await auth.configure('https://office.example.vn');
    const errors: string[] = [];
    auth.onError((e) => errors.push(e.message));
    await auth.signIn();
    sh.fire('bluedoc://auth/oidc?code=c1&state=forged');
    await vi.waitFor(() => expect(errors).toEqual(['auth.state']));
    expect(auth.user).toBeNull();
  });

  it('refuses to sign in without an OIDC client', async () => {
    const auth = new DesktopAuth({
      scheme: 'bluedoc',
      shell: shell().shell,
      storage: new MemoryStore(),
    });
    await expect(auth.signIn()).rejects.toThrow('auth.notConfigured');
    expect(await auth.getAccessToken()).toBeNull();
  });
});
