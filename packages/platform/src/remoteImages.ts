/**
 * Pictures from the web for the spreadsheet `IMAGE` function. The formula engine never
 * opens connections: BlueSheet asks the engine which URLs it needs (`imageRequests`),
 * fetches them through this service and hands the bytes back (`imageLoaded`).
 *
 * The web and desktop platforms both use the webview's `fetch` (HTTPS only, no
 * credentials, redirects refused, size-limited). A host that disallows web pictures
 * leaves `Platform.images` undefined: every `IMAGE` picture then shows `#BLOCKED!`.
 */

/** Why a picture could not be fetched. */
export type RemoteImageError = 'blocked' | 'connect';

/** Result of one fetch: the bytes, or the reason. */
export type RemoteImageResult =
  { ok: true; bytes: Uint8Array } | { ok: false; error: RemoteImageError };

/** Fetches pictures referenced by `IMAGE`. */
export interface RemoteImages {
  fetchImage(url: string): Promise<RemoteImageResult>;
}

/** Largest picture accepted (bytes); matches the engine's limit. */
export const MAX_REMOTE_IMAGE_BYTES = 25 * 1024 * 1024;

type FetchLike = (
  url: string,
  init: { credentials: 'omit'; redirect: 'error'; mode: 'cors' },
) => Promise<{ ok: boolean; arrayBuffer(): Promise<ArrayBuffer> }>;

/** [`RemoteImages`] over a `fetch` function. */
export class FetchRemoteImages implements RemoteImages {
  constructor(private readonly fetchFn: FetchLike = (u, i) => globalThis.fetch(u, i)) {}

  async fetchImage(url: string): Promise<RemoteImageResult> {
    if (!/^https:\/\//i.test(url)) return { ok: false, error: 'blocked' };
    try {
      const res = await this.fetchFn(url, { credentials: 'omit', redirect: 'error', mode: 'cors' });
      if (!res.ok) return { ok: false, error: 'connect' };
      const buf = await res.arrayBuffer();
      if (buf.byteLength === 0 || buf.byteLength > MAX_REMOTE_IMAGE_BYTES) {
        return { ok: false, error: 'connect' };
      }
      return { ok: true, bytes: new Uint8Array(buf) };
    } catch {
      return { ok: false, error: 'connect' };
    }
  }
}
