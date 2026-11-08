import { createHash } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import {
  OidcSession,
  fetchOidcConfig,
  jwtClaims,
  resolveOidcEndpoints,
  type KeyValueStore,
  type OidcFetch,
} from '../src/oidc';

const b64url = (s: string | Buffer) =>
  Buffer.from(s).toString('base64').replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
const jwt = (claims: Record<string, unknown>) =>
  `${b64url(JSON.stringify({ alg: 'none' }))}.${b64url(JSON.stringify(claims))}.sig`;

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

/**
 * Fake OpenID provider: discovery, authorization codes bound to a PKCE
 * challenge (S256), one-time rotating refresh tokens.
 */
class FakeProvider {
  codes = new Map<string, { challenge: string; redirectUri: string; user: string }>();
  refresh = new Map<string, string>();
  requests: Record<string, string>[] = [];
  lifetime = 300;
  seq = 0;
  failRefresh = false;

  authorize(url: string, user: string): string {
    const q = new URL(url).searchParams;
    expect(q.get('response_type')).toBe('code');
    expect(q.get('code_challenge_method')).toBe('S256');
    const code = `code-${++this.seq}`;
    this.codes.set(code, {
      challenge: q.get('code_challenge')!,
      redirectUri: q.get('redirect_uri')!,
      user,
    });
    return `${q.get('redirect_uri')}?code=${code}&state=${q.get('state')}`;
  }

  issue(user: string) {
    const rt = `rt-${++this.seq}`;
    this.refresh.set(rt, user);
    return {
      access_token: jwt({ sub: user, name: user === 'u1' ? 'Nguyễn Văn An' : user, n: this.seq }),
      refresh_token: rt,
      expires_in: this.lifetime,
      token_type: 'Bearer',
    };
  }

  fetch: OidcFetch = async (url, init) => {
    const res = (status: number, body: unknown) => ({
      ok: status < 400,
      status,
      json: async () => body,
    });
    if (url === 'https://idp/realms/bo/.well-known/openid-configuration') {
      return res(200, {
        issuer: 'https://idp/realms/bo',
        authorization_endpoint: 'https://idp/auth',
        token_endpoint: 'https://idp/token',
        end_session_endpoint: 'https://idp/logout',
      });
    }
    if (url === 'https://office/api/auth/config') {
      return res(200, { oidc: { authority: 'https://idp/realms/bo', clientId: 'blueoffice-web' } });
    }
    if (url !== 'https://idp/token') return res(404, {});
    const form = Object.fromEntries(new URLSearchParams(init?.body ?? ''));
    this.requests.push(form);
    if (form.grant_type === 'authorization_code') {
      const c = this.codes.get(form.code ?? '');
      this.codes.delete(form.code ?? '');
      const challenge = b64url(
        createHash('sha256')
          .update(form.code_verifier ?? '')
          .digest(),
      );
      if (!c || c.challenge !== challenge || c.redirectUri !== form.redirect_uri)
        return res(400, { error: 'invalid_grant' });
      return res(200, this.issue(c.user));
    }
    if (form.grant_type === 'refresh_token') {
      const user = this.refresh.get(form.refresh_token ?? '');
      this.refresh.delete(form.refresh_token ?? '');
      if (!user || this.failRefresh) return res(400, { error: 'invalid_grant' });
      return res(200, this.issue(user));
    }
    return res(400, { error: 'unsupported_grant_type' });
  };
}

async function setup(now = { t: 1_000_000 }) {
  const idp = new FakeProvider();
  const cfg = await fetchOidcConfig('https://office', idp.fetch);
  const endpoints = await resolveOidcEndpoints(cfg!, idp.fetch);
  const storage = new MemoryStore();
  const navigations: string[] = [];
  const make = () =>
    new OidcSession({
      endpoints,
      redirectUri: 'https://office/auth/callback',
      fetch: idp.fetch,
      storage,
      navigate: (u) => navigations.push(u),
      now: () => now.t,
    });
  return { idp, endpoints, storage, navigations, make, now };
}

describe('OIDC web sign-in (code + PKCE)', () => {
  it('reads the server config and completes it with discovery', async () => {
    const { endpoints } = await setup();
    expect(endpoints).toEqual({
      authorizeUrl: 'https://idp/auth',
      tokenUrl: 'https://idp/token',
      endSessionUrl: 'https://idp/logout',
      clientId: 'blueoffice-web',
      scope: 'openid profile',
    });
    const none = await fetchOidcConfig('https://office', async () => ({
      ok: true,
      status: 200,
      json: async () => ({ oidc: null }),
    }));
    expect(none).toBeNull();
  });

  it('redirects, exchanges the code with the verifier and knows the user', async () => {
    const { idp, navigations, make, storage } = await setup();
    const s = make();
    expect(s.user).toBeNull();
    expect(await s.getAccessToken()).toBeNull();
    await s.signIn('/sheet?x=1');
    const back = idp.authorize(navigations[0]!, 'u1');
    // The page reloads at the callback: a new session object reads the pending state.
    const s2 = make();
    const changes: (string | null)[] = [];
    s2.onChange((u) => changes.push(u?.name ?? null));
    expect(await s2.handleRedirect(back)).toBe('/sheet?x=1');
    expect(s2.user).toEqual({ id: 'u1', name: 'Nguyễn Văn An' });
    expect(changes).toEqual(['Nguyễn Văn An']);
    expect(jwtClaims((await s2.getAccessToken())!)?.sub).toBe('u1');
    // Survives a reload of the tab.
    expect(make().user?.id).toBe('u1');
    expect([...storage.map.keys()]).toEqual(['blueoffice.oidc.tokens']);
  });

  it('rejects a callback whose state does not match', async () => {
    const { idp, navigations, make } = await setup();
    const s = make();
    await s.signIn();
    const back = idp.authorize(navigations[0]!, 'u1').replace(/state=[^&]+/, 'state=forged');
    await expect(s.handleRedirect(back)).rejects.toThrow('auth.state');
    await expect(
      s.handleRedirect('https://office/auth/callback?error=access_denied'),
    ).rejects.toThrow('auth.access_denied');
    expect(s.user).toBeNull();
  });

  it('refreshes once shortly before expiry and rotates the refresh token', async () => {
    const { idp, navigations, make, now } = await setup();
    const s = make();
    await s.signIn();
    await s.handleRedirect(idp.authorize(navigations[0]!, 'u1'));
    const first = await s.getAccessToken();
    now.t += 200_000; // 100 s left: still valid
    expect(await s.getAccessToken()).toBe(first);
    now.t += 50_000; // 50 s left (< 60 s margin): refresh, concurrent callers share it
    const [a, b] = await Promise.all([s.getAccessToken(), s.getAccessToken()]);
    expect(a).toBe(b);
    expect(a).not.toBe(first);
    expect(idp.requests.map((r) => r.grant_type)).toEqual(['authorization_code', 'refresh_token']);
    // The used refresh token is gone; the new one works.
    now.t += 300_000;
    expect(await s.getAccessToken()).not.toBe(a);
    expect(idp.requests).toHaveLength(3);
    expect(idp.requests[2]!.refresh_token).not.toBe(idp.requests[1]!.refresh_token);
  });

  it('signs out when the refresh token is refused, and through end-session on sign-out', async () => {
    const { idp, navigations, make, now } = await setup();
    const s = make();
    await s.signIn();
    await s.handleRedirect(idp.authorize(navigations[0]!, 'u1'));
    idp.failRefresh = true;
    now.t += 290_000;
    expect(await s.getAccessToken()).toBeNull();
    expect(s.user).toBeNull();
    idp.failRefresh = false;
    await s.signIn();
    await s.handleRedirect(idp.authorize(navigations[1]!, 'u1'));
    await s.signOut();
    expect(s.user).toBeNull();
    expect(navigations[2]).toMatch(/^https:\/\/idp\/logout\?client_id=blueoffice-web/);
  });
});
