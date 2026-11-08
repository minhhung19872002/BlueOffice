/**
 * OIDC sign-in of the web client (prompt §3.4 `auth`: "web = redirect"):
 * OAuth 2.0 authorization code grant (RFC 6749 §4.1) with PKCE S256
 * (RFC 7636) against the provider the BlueOffice server is configured with
 * (`GET /api/auth/config`, endpoints from OpenID Connect Discovery 1.0 §4 when
 * the server does not list them). Tokens live in `sessionStorage` (per tab,
 * gone when the tab closes); the access token is refreshed with the refresh
 * token grant (RFC 6749 §6) shortly before it expires.
 *
 * Everything browser-specific (storage, navigation, clock, fetch) is
 * injected so the flow is tested without a browser.
 */
import type { AuthSession, UserInfo } from './types';

/** Client settings published by the server (`GET /api/auth/config` → `oidc`). */
export interface OidcClientConfig {
  authority?: string | null;
  authorizeUrl?: string | null;
  tokenUrl?: string | null;
  endSessionUrl?: string | null;
  clientId: string;
  scope?: string | null;
}

/** Resolved endpoints. */
export interface OidcEndpoints {
  authorizeUrl: string;
  tokenUrl: string;
  endSessionUrl: string | null;
  clientId: string;
  scope: string;
}

export interface OidcResponse {
  ok: boolean;
  status: number;
  json(): Promise<unknown>;
}

export type OidcFetch = (
  url: string,
  init?: { method?: string; headers?: Record<string, string>; body?: string },
) => Promise<OidcResponse>;

/** Minimal `Storage` (sessionStorage in the browser). */
export interface KeyValueStore {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export interface OidcSessionOptions {
  endpoints: OidcEndpoints;
  /** Absolute redirect URI registered at the provider (e.g. `https://office/auth/callback`). */
  redirectUri: string;
  fetch: OidcFetch;
  storage: KeyValueStore;
  /** Leaves the page for the provider (`location.assign`). */
  navigate: (url: string) => void;
  now?: () => number;
  /** Refresh this long before expiry (ms, default 60 s). */
  refreshMarginMs?: number;
  /** Random bytes (default `crypto.getRandomValues`). */
  random?: (n: number) => Uint8Array;
}

/** Tokens kept for the tab. */
interface StoredTokens {
  accessToken: string;
  refreshToken?: string;
  idToken?: string;
  /** Unix ms. */
  expiresAt: number;
}

interface PendingRedirect {
  state: string;
  verifier: string;
  returnTo: string;
}

const TOKENS = 'blueoffice.oidc.tokens';
const PENDING = 'blueoffice.oidc.pending';

const b64url = (bytes: Uint8Array): string => {
  let s = '';
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
};

/** `BASE64URL(SHA256(verifier))` (RFC 7636 §4.2). */
export async function pkceChallenge(verifier: string): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(verifier));
  return b64url(new Uint8Array(digest));
}

/** Random URL-safe string from `n` random bytes (32 bytes → a 43-character PKCE verifier, RFC 7636 §4.1). */
export function randomToken(n: number, random?: (n: number) => Uint8Array): string {
  return b64url(random ? random(n) : crypto.getRandomValues(new Uint8Array(n)));
}

/** Claims of a JWT payload (no signature check: the server validates tokens). */
export function jwtClaims(token: string): Record<string, unknown> | null {
  const part = token.split('.')[1];
  if (!part) return null;
  try {
    const b64 = part.replace(/-/g, '+').replace(/_/g, '/');
    const bin = atob(b64 + '='.repeat((4 - (b64.length % 4)) % 4));
    const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
    const v: unknown = JSON.parse(new TextDecoder().decode(bytes));
    return typeof v === 'object' && v !== null ? (v as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

const str = (v: unknown): string | null => (typeof v === 'string' && v ? v : null);

/**
 * Endpoints from the server's client config, completed with the provider's
 * discovery document (`<authority>/.well-known/openid-configuration`).
 */
export async function resolveOidcEndpoints(
  cfg: OidcClientConfig,
  fetchImpl: OidcFetch,
): Promise<OidcEndpoints> {
  let authorizeUrl = str(cfg.authorizeUrl);
  let tokenUrl = str(cfg.tokenUrl);
  let endSessionUrl = str(cfg.endSessionUrl);
  if ((!authorizeUrl || !tokenUrl) && str(cfg.authority)) {
    const res = await fetchImpl(
      `${String(cfg.authority).replace(/\/+$/, '')}/.well-known/openid-configuration`,
    );
    if (!res.ok) throw new Error(`auth.discovery ${res.status}`);
    const d = (await res.json()) as Record<string, unknown>;
    authorizeUrl ??= str(d.authorization_endpoint);
    tokenUrl ??= str(d.token_endpoint);
    endSessionUrl ??= str(d.end_session_endpoint);
  }
  if (!authorizeUrl || !tokenUrl || !cfg.clientId) throw new Error('auth.config');
  return {
    authorizeUrl,
    tokenUrl,
    endSessionUrl,
    clientId: cfg.clientId,
    scope: str(cfg.scope) ?? 'openid profile',
  };
}

/** Reads `GET <api>/api/auth/config`; `null` when the server has no OIDC client configured. */
export async function fetchOidcConfig(
  api: string,
  fetchImpl: OidcFetch,
): Promise<OidcClientConfig | null> {
  const res = await fetchImpl(`${api}/api/auth/config`);
  if (!res.ok) return null;
  const body = (await res.json().catch(() => null)) as { oidc?: unknown } | null;
  const o = body?.oidc as Partial<OidcClientConfig> | null | undefined;
  return o && typeof o.clientId === 'string' && o.clientId ? (o as OidcClientConfig) : null;
}

/** {@link AuthSession} signed in through an OIDC provider with a browser redirect. */
export class OidcSession implements AuthSession {
  private tokens: StoredTokens | null;
  private refreshing: Promise<string | null> | null = null;
  private readonly now: () => number;
  private readonly margin: number;
  private readonly listeners = new Set<(u: UserInfo | null) => void>();

  constructor(private readonly o: OidcSessionOptions) {
    this.now = o.now ?? Date.now;
    this.margin = o.refreshMarginMs ?? 60_000;
    this.tokens = this.load();
  }

  /** The signed-in user (from the ID token, else the access token claims). */
  get user(): UserInfo | null {
    const t = this.tokens;
    if (!t) return null;
    const c = (t.idToken ? jwtClaims(t.idToken) : null) ?? jwtClaims(t.accessToken);
    if (!c) return null;
    const id = str(c.sub) ?? '';
    const name = str(c.name) ?? str(c.preferred_username) ?? str(c.email) ?? id;
    const avatar = str(c.picture);
    return { id, name, ...(avatar ? { avatarUrl: avatar } : {}) };
  }

  /** Called when the user signs in, out, or the session cannot be refreshed. */
  onChange(fn: (u: UserInfo | null) => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  /** A valid access token, refreshed when it is about to expire; `null` when signed out. */
  async getAccessToken(): Promise<string | null> {
    const t = this.tokens;
    if (!t) return null;
    if (t.expiresAt - this.margin > this.now()) return t.accessToken;
    if (!t.refreshToken) {
      // Expired and nothing to refresh with.
      if (t.expiresAt > this.now()) return t.accessToken;
      this.set(null);
      return null;
    }
    this.refreshing ??= this.refresh(t.refreshToken).finally(() => {
      this.refreshing = null;
    });
    return this.refreshing;
  }

  /** Redirects to the provider; the page comes back to {@link OidcSession.handleRedirect}. */
  async signIn(returnTo?: string, loginHint?: string): Promise<void> {
    const verifier = randomToken(32, this.o.random);
    const state = randomToken(16, this.o.random);
    const pending: PendingRedirect = { state, verifier, returnTo: returnTo ?? '/' };
    this.o.storage.setItem(PENDING, JSON.stringify(pending));
    const e = this.o.endpoints;
    const q = new URLSearchParams({
      response_type: 'code',
      client_id: e.clientId,
      redirect_uri: this.o.redirectUri,
      scope: e.scope,
      state,
      code_challenge: await pkceChallenge(verifier),
      code_challenge_method: 'S256',
    });
    if (loginHint) q.set('login_hint', loginHint);
    const sep = e.authorizeUrl.includes('?') ? '&' : '?';
    this.o.navigate(`${e.authorizeUrl}${sep}${q.toString()}`);
  }

  /**
   * Completes the redirect (`<redirectUri>?code=…&state=…`): checks the state,
   * exchanges the code and returns the page to go back to.
   */
  async handleRedirect(url: string): Promise<string> {
    const q = new URL(url).searchParams;
    const raw = this.o.storage.getItem(PENDING);
    this.o.storage.removeItem(PENDING);
    const pending = raw ? (JSON.parse(raw) as Partial<PendingRedirect>) : null;
    const error = q.get('error');
    if (error) throw new Error(`auth.${error}`);
    const code = q.get('code');
    if (!code || !pending?.verifier || q.get('state') !== pending.state) {
      throw new Error('auth.state');
    }
    await this.token({
      grant_type: 'authorization_code',
      code,
      code_verifier: pending.verifier,
      redirect_uri: this.o.redirectUri,
      client_id: this.o.endpoints.clientId,
    });
    return pending.returnTo ?? '/';
  }

  async signOut(): Promise<void> {
    const idToken = this.tokens?.idToken;
    this.set(null);
    const end = this.o.endpoints.endSessionUrl;
    if (end) {
      const q = new URLSearchParams({ client_id: this.o.endpoints.clientId });
      if (idToken) q.set('id_token_hint', idToken);
      this.o.navigate(`${end}${end.includes('?') ? '&' : '?'}${q.toString()}`);
    }
  }

  private async refresh(refreshToken: string): Promise<string | null> {
    try {
      await this.token({
        grant_type: 'refresh_token',
        refresh_token: refreshToken,
        client_id: this.o.endpoints.clientId,
      });
      return this.tokens?.accessToken ?? null;
    } catch {
      // Refresh token expired or revoked: signed out (the shell asks to sign in again).
      this.set(null);
      return null;
    }
  }

  private async token(form: Record<string, string>): Promise<void> {
    const res = await this.o.fetch(this.o.endpoints.tokenUrl, {
      method: 'POST',
      headers: { 'content-type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams(form).toString(),
    });
    if (!res.ok) throw new Error(`auth.token ${res.status}`);
    const b = (await res.json()) as Record<string, unknown>;
    const accessToken = str(b.access_token);
    if (!accessToken) throw new Error('auth.token');
    const expiresIn = typeof b.expires_in === 'number' ? b.expires_in : null;
    const exp = jwtClaims(accessToken)?.exp;
    const expiresAt =
      expiresIn !== null
        ? this.now() + expiresIn * 1000
        : typeof exp === 'number'
          ? exp * 1000
          : this.now() + 300_000;
    // A refresh response may omit the refresh token (keep the old one, RFC 6749 §6).
    const refreshToken = str(b.refresh_token) ?? this.tokens?.refreshToken;
    const idToken = str(b.id_token) ?? this.tokens?.idToken;
    this.set({
      accessToken,
      expiresAt,
      ...(refreshToken ? { refreshToken } : {}),
      ...(idToken ? { idToken } : {}),
    });
  }

  private load(): StoredTokens | null {
    try {
      const raw = this.o.storage.getItem(TOKENS);
      const t = raw ? (JSON.parse(raw) as Partial<StoredTokens>) : null;
      return t && typeof t.accessToken === 'string' && typeof t.expiresAt === 'number'
        ? (t as StoredTokens)
        : null;
    } catch {
      return null;
    }
  }

  private set(t: StoredTokens | null): void {
    const before = this.user?.id ?? null;
    this.tokens = t;
    try {
      if (t) this.o.storage.setItem(TOKENS, JSON.stringify(t));
      else this.o.storage.removeItem(TOKENS);
    } catch {
      // Storage unavailable: the session lasts as long as the page.
    }
    if ((this.user?.id ?? null) !== before) this.listeners.forEach((l) => l(this.user));
  }
}
