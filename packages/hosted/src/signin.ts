/**
 * Desktop sign-in to a host: OAuth 2.0 authorization code grant (RFC 6749
 * §4.1) with PKCE S256 (RFC 7636) in the system browser; the identity
 * provider redirects to the app's deep link (`bluedoc://auth?code=…&state=…`).
 * The resulting access token is the user's host token for BOHP.
 */
import { pkceChallenge, randomToken } from '@blueoffice/platform';
import type { DriveSignIn } from './drive';
import type { FetchLike } from './session';

/** In-flight sign-in (kept in memory until the deep link comes back). */
export interface PendingSignIn {
  url: string;
  state: string;
  verifier: string;
  redirectUri: string;
}

// PKCE helpers are shared with the web client's OIDC sign-in (`@blueoffice/platform`).
export { pkceChallenge };
const random = (n: number): string => randomToken(n);

/** Builds the authorization URL; `loginHint` pre-selects the account. */
export async function beginSignIn(
  cfg: DriveSignIn,
  redirectUri: string,
  loginHint?: string,
): Promise<PendingSignIn> {
  // 43–128 characters from the unreserved set (RFC 7636 §4.1): 32 random bytes → 43.
  const verifier = random(32);
  const state = random(16);
  const q = new URLSearchParams({
    response_type: 'code',
    client_id: cfg.clientId,
    redirect_uri: redirectUri,
    scope: cfg.scope,
    state,
    code_challenge: await pkceChallenge(verifier),
    code_challenge_method: 'S256',
  });
  if (loginHint) q.set('login_hint', loginHint);
  const sep = cfg.authorizeUrl.includes('?') ? '&' : '?';
  return { url: `${cfg.authorizeUrl}${sep}${q.toString()}`, state, verifier, redirectUri };
}

/** Parameters of the redirect (`bluedoc://auth?code=…&state=…` or `?error=…`). */
export function parseCallback(url: string): { code?: string; state?: string; error?: string } {
  const q = new URLSearchParams(url.includes('?') ? url.slice(url.indexOf('?') + 1) : '');
  const out: { code?: string; state?: string; error?: string } = {};
  const code = q.get('code');
  const state = q.get('state');
  const error = q.get('error');
  if (code) out.code = code;
  if (state) out.state = state;
  if (error) out.error = error;
  return out;
}

/** Exchanges the code for the access token (checks `state` first). */
export async function completeSignIn(
  cfg: DriveSignIn,
  pending: PendingSignIn,
  callbackUrl: string,
  fetchImpl: FetchLike,
): Promise<string> {
  const cb = parseCallback(callbackUrl);
  if (cb.error) throw new Error(`signIn.${cb.error}`);
  if (!cb.code || cb.state !== pending.state) throw new Error('signIn.state');
  const res = await fetchImpl(cfg.tokenUrl, {
    method: 'POST',
    headers: { 'content-type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({
      grant_type: 'authorization_code',
      code: cb.code,
      code_verifier: pending.verifier,
      redirect_uri: pending.redirectUri,
      client_id: cfg.clientId,
    }).toString(),
  });
  if (!res.ok) throw new Error(`signIn.token ${res.status}`);
  const body = (await res.json()) as { access_token?: unknown };
  if (typeof body.access_token !== 'string' || !body.access_token) throw new Error('signIn.token');
  return body.access_token;
}
