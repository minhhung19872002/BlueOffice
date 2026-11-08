/**
 * Browsing a host (BlueDrive) from the desktop Start screen and resolving
 * files of a sync folder: the BOHP §A2 endpoints proxied by the BlueOffice
 * server (`/api/hosts/{host}/…`, docs/bohp.md). The desktop never calls the
 * host directly; the user's host token goes along as `Authorization: Bearer`.
 */
import type { FetchLike } from './session';

/** A configured host (`GET /api/hosts`). */
export interface DriveHostInfo {
  name: string;
  displayName: string;
  protocol: string;
  /** Offers the browsing endpoints (BOHP hosts). */
  browse: boolean;
  /** OAuth 2.0 authorization code + PKCE settings; null → token sign-in only. */
  signIn: DriveSignIn | null;
}

export interface DriveSignIn {
  authorizeUrl: string;
  tokenUrl: string;
  clientId: string;
  scope: string;
}

export interface DriveItem {
  kind: 'folder' | 'file';
  id: string;
  name: string;
  size?: number | null;
  version?: string | null;
  lastModified?: string | null;
  ownerId?: string | null;
  openedAt?: string | null;
}

export interface DriveFolder {
  id: string;
  name: string;
  parentId: string | null;
}

export interface DriveListing {
  folder: DriveFolder;
  items: DriveItem[];
}

export interface DriveUser {
  id: string;
  name: string;
  avatarUrl?: string | null;
}

/** Error of a browsing call; `status` 401 means the host token is no longer valid. */
export class DriveError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

const str = (v: unknown): string => (typeof v === 'string' ? v : '');
const opt = (v: unknown): string | null => (typeof v === 'string' ? v : null);

function toItem(v: unknown): DriveItem | null {
  const o = v as Record<string, unknown> | null;
  if (!o || typeof o !== 'object' || typeof o.id !== 'string') return null;
  return {
    kind: o.kind === 'folder' ? 'folder' : 'file',
    id: o.id,
    name: str(o.name),
    size: typeof o.size === 'number' ? o.size : null,
    version: opt(o.version),
    lastModified: opt(o.lastModified),
    ownerId: opt(o.ownerId),
    openedAt: opt(o.openedAt),
  };
}

const toItems = (v: unknown): DriveItem[] =>
  Array.isArray(v) ? v.map(toItem).filter((i): i is DriveItem => i !== null) : [];

/** Hosts known to the server, with their sign-in settings. */
export async function listHosts(api: string, fetchImpl: FetchLike): Promise<DriveHostInfo[]> {
  const res = await fetchImpl(`${api}/api/hosts`);
  if (!res.ok) throw new DriveError(res.status, `hosts ${res.status}`);
  const body = (await res.json()) as { hosts?: unknown };
  if (!Array.isArray(body.hosts)) return [];
  return body.hosts.flatMap((h: unknown) => {
    const o = h as Record<string, unknown> | null;
    if (!o || typeof o.name !== 'string') return [];
    const s = o.signIn as Record<string, unknown> | null | undefined;
    const signIn =
      s && typeof s.authorizeUrl === 'string' && typeof s.tokenUrl === 'string'
        ? {
            authorizeUrl: s.authorizeUrl,
            tokenUrl: s.tokenUrl,
            clientId: str(s.clientId),
            scope: str(s.scope) || 'openid profile',
          }
        : null;
    return [
      {
        name: o.name,
        displayName: str(o.displayName) || o.name,
        protocol: str(o.protocol),
        browse: o.browse === true,
        signIn,
      },
    ];
  });
}

/** Browsing calls for one host and one signed-in user. */
export class DriveClient {
  constructor(
    private readonly api: string,
    readonly host: string,
    readonly token: string,
    private readonly fetchImpl: FetchLike,
  ) {}

  private async get(path: string): Promise<unknown> {
    const res = await this.fetchImpl(
      `${this.api}/api/hosts/${encodeURIComponent(this.host)}${path}`,
      {
        headers: { authorization: `Bearer ${this.token}` },
      },
    );
    if (!res.ok) throw new DriveError(res.status, `drive ${path.split('?')[0]} ${res.status}`);
    return res.json();
  }

  async me(): Promise<DriveUser> {
    const o = (await this.get('/me')) as Record<string, unknown>;
    return { id: str(o.id), name: str(o.name), avatarUrl: opt(o.avatarUrl) };
  }

  /** Children of a folder (`root` = the user's top folder). */
  async children(folderId = 'root'): Promise<DriveListing> {
    const o = (await this.get(`/folders/${encodeURIComponent(folderId)}/children`)) as Record<
      string,
      unknown
    >;
    const f = (o.folder ?? {}) as Record<string, unknown>;
    return {
      folder: { id: str(f.id) || folderId, name: str(f.name), parentId: opt(f.parentId) },
      items: toItems(o.items),
    };
  }

  async shared(): Promise<DriveItem[]> {
    return toItems(((await this.get('/shared')) as { items?: unknown }).items);
  }

  async recent(limit = 20): Promise<DriveItem[]> {
    return toItems(((await this.get(`/recent?limit=${limit}`)) as { items?: unknown }).items);
  }

  /** File at a '/'-separated path inside the user's sync folder, or null. */
  async resolve(relativePath: string): Promise<{ id: string; name: string } | null> {
    try {
      const o = (await this.get(`/resolve?path=${encodeURIComponent(relativePath)}`)) as Record<
        string,
        unknown
      >;
      return typeof o.id === 'string' ? { id: o.id, name: str(o.name) } : null;
    } catch (e) {
      if (e instanceof DriveError && e.status === 404) return null;
      throw e;
    }
  }
}
