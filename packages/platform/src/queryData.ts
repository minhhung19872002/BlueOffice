/**
 * Folder and web page access for Get & Transform data (SHEET-PQ-001) in the browser:
 * folders through `<input type="file" webkitdirectory>` (HTML Living Standard + the
 * File and Directory Entries API), web pages only through the BlueOffice server's proxy
 * (`POST /api/data/web`), and only when the host names one — a page cannot read other
 * sites itself (CORS), and nothing is sent anywhere unless an administrator set it up.
 */
import type { PickedFolder, QueryDataSources } from './sheetQuery';

/** Largest web page relayed (bytes). */
export const MAX_WEB_BYTES = 20 * 1024 * 1024;

/** Is `url` an http(s) URL without credentials? */
export function isWebUrl(url: string): boolean {
  try {
    const u = new URL(url);
    return (u.protocol === 'http:' || u.protocol === 'https:') && !u.username && !u.password;
  } catch {
    return false;
  }
}

/** Options of {@link createBrowserQueryData}. */
export interface BrowserQueryDataOptions {
  /** Base URL of the server (`''` = same origin); `null`/absent: no web pages. */
  proxyBaseUrl?: string | null;
  /** Headers of each proxy request (bearer token). */
  headers?: () => Promise<Record<string, string>>;
  /** `fetch` (tests). */
  fetch?: (input: string, init?: RequestInit) => Promise<Response>;
  /** Folder picker (tests). */
  pickFolder?: () => Promise<PickedFolder | null>;
}

/** Ask for a folder with a hidden directory input. */
export function pickFolderWithInput(): Promise<PickedFolder | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.multiple = true;
    input.setAttribute('webkitdirectory', '');
    input.style.display = 'none';
    input.addEventListener('change', () => {
      const list = Array.from(input.files ?? []);
      input.remove();
      if (list.length === 0) return resolve(null);
      void Promise.all(
        list.map(async (f): Promise<{ name: string; bytes: Uint8Array } | null> => {
          const rel = (f as File & { webkitRelativePath?: string }).webkitRelativePath ?? f.name;
          // Only the files directly in the folder (not its subfolders).
          const parts = rel.split('/');
          return parts.length <= 2
            ? { name: f.name, bytes: new Uint8Array(await f.arrayBuffer()) }
            : null;
        }),
      ).then((files) => {
        const first = (list[0] as File & { webkitRelativePath?: string }).webkitRelativePath ?? '';
        resolve({
          name: first.split('/')[0] || 'folder',
          files: files
            .filter((f): f is { name: string; bytes: Uint8Array } => f !== null)
            .sort((a, b) => a.name.localeCompare(b.name)),
        });
      });
    });
    input.addEventListener('cancel', () => {
      input.remove();
      resolve(null);
    });
    document.body.appendChild(input);
    input.click();
  });
}

/** Get & Transform sources of the web platform. */
export function createBrowserQueryData(opts: BrowserQueryDataOptions = {}): QueryDataSources {
  const out: QueryDataSources = { pickFolder: opts.pickFolder ?? pickFolderWithInput };
  const base = opts.proxyBaseUrl;
  if (base === null || base === undefined) return out;
  const fetchImpl = opts.fetch ?? ((i: string, init?: RequestInit) => fetch(i, init));
  out.fetchWeb = async (url: string) => {
    if (!isWebUrl(url)) throw new Error('network');
    let res: Response;
    try {
      res = await fetchImpl(`${base.replace(/\/$/, '')}/api/data/web`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...(await (opts.headers?.() ?? {})) },
        body: JSON.stringify({ url }),
        credentials: 'same-origin',
      });
    } catch {
      throw new Error('network');
    }
    if (res.status === 404) throw new Error('unavailable');
    if (!res.ok) throw new Error(res.status === 413 ? 'tooLarge' : 'status');
    const bytes = new Uint8Array(await res.arrayBuffer());
    if (bytes.length > MAX_WEB_BYTES) throw new Error('tooLarge');
    return bytes;
  };
  return out;
}
