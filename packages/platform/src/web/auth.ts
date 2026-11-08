/**
 * Web wiring of {@link OidcSession}: configuration from the BlueOffice server
 * (`GET /api/auth/config`), redirect URI `<origin><base>auth/callback`,
 * tokens in `sessionStorage`, navigation with `location.assign`.
 */
import { OidcSession, fetchOidcConfig, resolveOidcEndpoints, type OidcFetch } from '../oidc';

/** Path (relative to the app base) the provider redirects back to. */
export const AUTH_CALLBACK_PATH = 'auth/callback';

export interface WebAuthOptions {
  /** Server base (`""` = same origin). */
  api: string;
  /** App base URL path (Vite `BASE_URL`, default `/`). */
  base?: string;
  fetch?: OidcFetch;
}

/** The OIDC session when the server has a web client configured, else `null` (anonymous). */
export async function createWebAuth(o: WebAuthOptions): Promise<OidcSession | null> {
  const fetchImpl: OidcFetch =
    o.fetch ??
    ((url, init) =>
      window.fetch(url, init).then((r) => ({ ok: r.ok, status: r.status, json: () => r.json() })));
  const cfg = await fetchOidcConfig(o.api, fetchImpl).catch(() => null);
  if (!cfg) return null;
  const endpoints = await resolveOidcEndpoints(cfg, fetchImpl);
  const base = o.base ?? '/';
  return new OidcSession({
    endpoints,
    redirectUri: new URL(`${base}${AUTH_CALLBACK_PATH}`, window.location.origin).toString(),
    fetch: fetchImpl,
    storage: window.sessionStorage,
    navigate: (url) => window.location.assign(url),
  });
}
