/**
 * BlueOffice user sign-in of the desktop apps (prompt §3.4 `auth`: "desktop =
 * OAuth device/loopback"): OAuth 2.0 for native apps (RFC 8252) — the
 * authorization code + PKCE flow of {@link OidcSession} runs in the system
 * browser (§6, never in the app's web view) and comes back through the app's
 * private-use URI scheme (§7.1): `bluedoc://auth/oidc?code=…&state=…`,
 * forwarded by the Tauri shell as an `auth-callback` event. The provider is
 * the one the BlueOffice server publishes (`GET /api/auth/config`); its
 * client must allow the `blue*://auth/oidc` redirect URIs.
 *
 * Tokens are kept per server in the web view's `localStorage`, so the user
 * stays signed in across restarts until the refresh token expires.
 */
import {
  OidcSession,
  fetchOidcConfig,
  resolveOidcEndpoints,
  type KeyValueStore,
  type OidcFetch,
} from '../oidc';
import type { AuthSession, ShellIntegration, UserInfo } from '../types';

/** Path of the sign-in redirect (`<scheme>://auth/oidc`). */
export const DESKTOP_OIDC_PATH = 'auth/oidc';

export type DesktopScheme = 'bluedoc' | 'bluesheet' | 'blueslide';

export interface DesktopAuthOptions {
  scheme: DesktopScheme;
  shell: Pick<ShellIntegration, 'openExternal' | 'onAuthCallback'>;
  fetch?: OidcFetch;
  /** Token store (default `localStorage`). */
  storage?: KeyValueStore;
  now?: () => number;
  random?: (n: number) => Uint8Array;
}

/** Is `url` a BlueOffice sign-in redirect (not a BlueDrive one, `…://auth`)? */
export function isOidcCallback(url: string): boolean {
  const m = /^blue(?:doc|sheet|slide):\/\/([^?#]*)/i.exec(url);
  return !!m && m[1]?.replace(/\/+$/, '') === DESKTOP_OIDC_PATH;
}

/** `store` with every key prefixed (tokens of different servers stay apart). */
function prefixed(store: KeyValueStore, prefix: string): KeyValueStore {
  return {
    getItem: (k) => store.getItem(`${prefix}${k}`),
    setItem: (k, v) => store.setItem(`${prefix}${k}`, v),
    removeItem: (k) => store.removeItem(`${prefix}${k}`),
  };
}

const memoryStore = (): KeyValueStore => {
  const m = new Map<string, string>();
  return {
    getItem: (k) => m.get(k) ?? null,
    setItem: (k, v) => void m.set(k, v),
    removeItem: (k) => void m.delete(k),
  };
};

/**
 * {@link AuthSession} of the desktop shell: anonymous until {@link DesktopAuth.configure}
 * finds an OIDC client on the BlueOffice server.
 */
export class DesktopAuth implements AuthSession {
  private session: OidcSession | null = null;
  private api: string | null = null;
  private offSession: (() => void) | null = null;
  private readonly listeners = new Set<(u: UserInfo | null) => void>();
  private readonly errors = new Set<(e: Error) => void>();
  private offCallback: (() => void) | null = null;

  constructor(private readonly o: DesktopAuthOptions) {}

  /** Listens for the sign-in redirect (once a provider is configured). */
  private listen(): void {
    this.offCallback ??=
      this.o.shell.onAuthCallback?.((url) => {
        if (!isOidcCallback(url) || !this.session) return;
        this.session.handleRedirect(url).catch((e: unknown) => {
          const err = e instanceof Error ? e : new Error(String(e));
          this.errors.forEach((f) => f(err));
        });
      }) ?? null;
  }

  /** An OIDC client is configured on the current server. */
  get enabled(): boolean {
    return this.session !== null;
  }

  get user(): UserInfo | null {
    return this.session?.user ?? null;
  }

  /**
   * Uses the OIDC client of the BlueOffice server at `api` (`null`: no server,
   * anonymous). Returns whether sign-in is available.
   */
  async configure(api: string | null): Promise<boolean> {
    const base = api ? api.replace(/\/+$/, '') : null;
    if (base === this.api && (this.session || !base)) return this.enabled;
    const before = this.user?.id ?? null;
    this.api = base;
    this.offSession?.();
    this.offSession = null;
    this.session = null;
    const fetchImpl = this.o.fetch ?? defaultFetch;
    const cfg = base ? await fetchOidcConfig(base, fetchImpl).catch(() => null) : null;
    if (cfg && this.api === base && base) {
      const endpoints = await resolveOidcEndpoints(cfg, fetchImpl).catch(() => null);
      if (endpoints && this.api === base) {
        const storage = this.o.storage ?? globalThis.localStorage ?? memoryStore();
        this.session = new OidcSession({
          endpoints,
          redirectUri: `${this.o.scheme}://${DESKTOP_OIDC_PATH}`,
          fetch: fetchImpl,
          storage: prefixed(storage, `blueoffice.desktop.${base}|`),
          navigate: (url) => void this.o.shell.openExternal?.(url),
          ...(this.o.now ? { now: this.o.now } : {}),
          ...(this.o.random ? { random: this.o.random } : {}),
        });
        this.offSession = this.session.onChange((u) => this.listeners.forEach((f) => f(u)));
        this.listen();
      }
    }
    if ((this.user?.id ?? null) !== before) this.listeners.forEach((f) => f(this.user));
    return this.enabled;
  }

  onChange(fn: (u: UserInfo | null) => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  /** Sign-in failures (state mismatch, token endpoint errors). */
  onError(fn: (e: Error) => void): () => void {
    this.errors.add(fn);
    return () => this.errors.delete(fn);
  }

  getAccessToken(): Promise<string | null> {
    return this.session?.getAccessToken() ?? Promise.resolve(null);
  }

  /** Opens the provider's sign-in page in the system browser. */
  async signIn(): Promise<void> {
    if (!this.session) throw new Error('auth.notConfigured');
    if (!this.o.shell.openExternal) throw new Error('signIn.unsupported');
    await this.session.signIn();
  }

  async signOut(): Promise<void> {
    await this.session?.signOut();
  }

  dispose(): void {
    this.offCallback?.();
    this.offCallback = null;
    this.offSession?.();
  }
}

const defaultFetch: OidcFetch = (url, init) =>
  globalThis.fetch(url, init).then((r) => ({ ok: r.ok, status: r.status, json: () => r.json() }));
