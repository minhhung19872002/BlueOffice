/**
 * Organisation add-in catalogue (CORE-ADDIN-001, ADR-0013): the BlueOffice
 * server keeps the add-in manifests an administrator published and which of
 * them each user installed. Off (404) until an administrator sets
 * `Addins:Enabled`; then the editors show the "Add-in" tab.
 *
 * - signed-in web users: `GET /api/addins`, `POST|DELETE /api/addins/{id}/install`
 *   with their bearer token;
 * - hosted sessions (embedded editor, desktop deep link): the same routes with
 *   `?session=<id>` and the `x-session-token` header.
 */

/** One catalogue entry as the server lists it. */
export interface AddinListing {
  /** The published manifest (validated again by the editor before use). */
  manifest: unknown;
  /** An administrator marked it trusted: its frame may keep its own origin (`allow-same-origin`). */
  trusted: boolean;
  /** Installed by the current user. */
  installed: boolean;
  /** Permissions the user consented to when installing. */
  granted: string[];
}

export interface AddinCatalog {
  /** Add-ins published for `app`; `null` when the feature is off or the server is unreachable. */
  list(app: 'doc' | 'sheet' | 'slide'): Promise<AddinListing[] | null>;
  /** Installs `id` for the current user, consenting to `permissions` (all of the manifest's). */
  install(id: string, permissions: string[]): Promise<void>;
  /** Removes the install (the editor also deletes the add-in's storage). */
  uninstall(id: string): Promise<void>;
}

export type AddinFetch = (
  input: string,
  init?: { method?: string; headers?: Record<string, string>; body?: string },
) => Promise<{ ok: boolean; status: number; json(): Promise<unknown> }>;

export interface ServerAddinCatalogOptions {
  /** Server base URL (`''`: same origin). */
  baseUrl: string;
  /** Hosted session id (adds `?session=`). */
  session?: string;
  headers?: () => Record<string, string> | Promise<Record<string, string>>;
  fetch?: AddinFetch;
}

const isObject = (v: unknown): v is Record<string, unknown> =>
  !!v && typeof v === 'object' && !Array.isArray(v);

/** The BlueOffice server's catalogue endpoints. */
export class ServerAddinCatalog implements AddinCatalog {
  private readonly fetchImpl: AddinFetch;

  constructor(private readonly opts: ServerAddinCatalogOptions) {
    this.fetchImpl = opts.fetch ?? ((i, init) => fetch(i, init));
  }

  private url(path: string, query: Record<string, string> = {}): string {
    const q = new URLSearchParams(query);
    if (this.opts.session) q.set('session', this.opts.session);
    const qs = q.toString();
    return `${this.opts.baseUrl.replace(/\/+$/, '')}${path}${qs ? `?${qs}` : ''}`;
  }

  private async headers(json = false): Promise<Record<string, string>> {
    return {
      ...(await this.opts.headers?.()),
      ...(json ? { 'content-type': 'application/json' } : {}),
    };
  }

  async list(app: 'doc' | 'sheet' | 'slide'): Promise<AddinListing[] | null> {
    try {
      const res = await this.fetchImpl(this.url('/api/addins', { app }), {
        headers: await this.headers(),
      });
      if (!res.ok) return null;
      const body = await res.json();
      if (!isObject(body) || !Array.isArray(body.addins)) return null;
      return body.addins.filter(isObject).map((a) => ({
        manifest: a.manifest,
        trusted: a.trusted === true,
        installed: a.installed === true,
        granted: Array.isArray(a.granted)
          ? a.granted.filter((p): p is string => typeof p === 'string')
          : [],
      }));
    } catch {
      // Unreachable, or not a BlueOffice server (a static site answers HTML): off.
      return null;
    }
  }

  async install(id: string, permissions: string[]): Promise<void> {
    const res = await this.fetchImpl(this.url(`/api/addins/${encodeURIComponent(id)}/install`), {
      method: 'POST',
      headers: await this.headers(true),
      body: JSON.stringify({ permissions }),
    });
    if (!res.ok) throw new Error(`addins.installFailed:${res.status}`);
  }

  async uninstall(id: string): Promise<void> {
    const res = await this.fetchImpl(this.url(`/api/addins/${encodeURIComponent(id)}/install`), {
      method: 'DELETE',
      headers: await this.headers(),
    });
    if (!res.ok && res.status !== 404) throw new Error(`addins.uninstallFailed:${res.status}`);
  }
}
