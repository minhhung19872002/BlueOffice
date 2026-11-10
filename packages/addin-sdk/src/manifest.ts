/**
 * Add-in manifest (CORE-ADDIN-001, ADR-0013): BlueOffice's own JSON format,
 * described by `manifest.schema.json` and checked by {@link validateManifest}
 * (the server checks the same rules when an administrator publishes one).
 */
import { BASE_PERMISSIONS, NETWORK_PREFIX, type AddinPermission } from './permissions';

export const MANIFEST_VERSION = 1;

export type AddinApp = 'doc' | 'sheet' | 'slide';
export const ADDIN_APPS: readonly AddinApp[] = ['doc', 'sheet', 'slide'];

/** Text in both UI languages. */
export interface LocalizedText {
  vi: string;
  en: string;
}

export interface AddinRibbonCommand {
  /** Unique in the manifest: letters, digits, `-`, `_`. */
  id: string;
  label: LocalizedText;
  tooltip?: LocalizedText;
  /** `openPane` opens the add-in's task pane; `event` also sends a `command` event to it. */
  action: 'openPane' | 'event';
  /** Applications showing the command (default: the manifest's `apps`). */
  apps?: AddinApp[];
}

export interface AddinRibbon {
  /**
   * Id of the ribbon tab that gets the commands (`home`, `insert`, `review`,
   * `view`…); absent or unknown: the "Add-in" tab.
   */
  tab?: string;
  /** Name of the group holding the commands (default: the add-in name). */
  group?: LocalizedText;
  commands: AddinRibbonCommand[];
}

export interface AddinManifest {
  $schema?: string;
  manifestVersion: 1;
  /** Reverse domain name, e.g. `vn.example.hello`. */
  id: string;
  name: LocalizedText;
  description?: LocalizedText;
  /** Semantic version `1.2.3`. */
  version: string;
  publisher: string;
  /** Icon URL (one of `allowedOrigins`). */
  icon?: string;
  /** Page loaded in the task pane (one of `allowedOrigins`). */
  startUrl: string;
  /** Origins the add-in's pages come from (`https://host[:port]`). */
  allowedOrigins: string[];
  permissions: AddinPermission[];
  apps: AddinApp[];
  ribbon?: AddinRibbon;
}

export interface ManifestError {
  /** JSON path of the offending value, e.g. `ribbon.commands[0].label.vi`. */
  path: string;
  message: string;
}

export type ManifestResult =
  { ok: true; manifest: AddinManifest } | { ok: false; errors: ManifestError[] };

export const MANIFEST_LIMITS = {
  idChars: 128,
  nameChars: 100,
  descriptionChars: 1000,
  publisherChars: 100,
  urlChars: 2048,
  origins: 20,
  permissions: 30,
  commands: 20,
  labelChars: 60,
  tooltipChars: 200,
} as const;

const ID_RE = /^[a-z0-9](?:[a-z0-9-]{0,62})(?:\.[a-z0-9](?:[a-z0-9-]{0,62}))+$/;
const COMMAND_ID_RE = /^[A-Za-z][A-Za-z0-9_-]{0,63}$/;
const VERSION_RE =
  /^(0|[1-9]\d{0,8})\.(0|[1-9]\d{0,8})\.(0|[1-9]\d{0,8})(?:-[0-9A-Za-z.-]{1,40})?$/;
const TAB_RE = /^[A-Za-z][A-Za-z0-9]{0,39}$/;

const TOP_KEYS = new Set([
  '$schema',
  'manifestVersion',
  'id',
  'name',
  'description',
  'version',
  'publisher',
  'icon',
  'startUrl',
  'allowedOrigins',
  'permissions',
  'apps',
  'ribbon',
]);

/** Fields every manifest has (kept in step with `manifest.schema.json`). */
export const REQUIRED_FIELDS = [
  'manifestVersion',
  'id',
  'name',
  'version',
  'publisher',
  'startUrl',
  'allowedOrigins',
  'permissions',
  'apps',
] as const;

const isLoopback = (host: string) => host === 'localhost' || host === '127.0.0.1';

/**
 * `o` is a canonical origin an add-in may come from: `https://host[:port]`, or
 * `http://localhost[:port]` / `http://127.0.0.1[:port]` for development.
 */
export function isAddinOrigin(o: unknown): o is string {
  if (typeof o !== 'string' || o.length > 300) return false;
  let u: URL;
  try {
    u = new URL(o);
  } catch {
    return false;
  }
  if (u.origin !== o) return false;
  return u.protocol === 'https:' || (u.protocol === 'http:' && isLoopback(u.hostname));
}

/** `url` is an absolute http(s) URL without credentials whose origin is in `origins`. */
export function urlInOrigins(url: unknown, origins: readonly string[]): url is string {
  if (typeof url !== 'string' || url.length > MANIFEST_LIMITS.urlChars) return false;
  let u: URL;
  try {
    u = new URL(url);
  } catch {
    return false;
  }
  if (u.username || u.password) return false;
  if (u.protocol !== 'https:' && u.protocol !== 'http:') return false;
  return origins.includes(u.origin);
}

const isObject = (v: unknown): v is Record<string, unknown> =>
  !!v && typeof v === 'object' && !Array.isArray(v);

/** No C0 control characters or DEL (labels are shown as plain text). */
const plainText = (s: string) =>
  ![...s].some((c) => c.charCodeAt(0) < 0x20 || c.charCodeAt(0) === 0x7f);

/** Validates parsed JSON as a manifest (all errors, not just the first). */
export function validateManifest(value: unknown): ManifestResult {
  const errors: ManifestError[] = [];
  const err = (path: string, message: string) => errors.push({ path, message });
  if (!isObject(value)) return { ok: false, errors: [{ path: '', message: 'expected an object' }] };
  const m = value;

  for (const k of Object.keys(m)) if (!TOP_KEYS.has(k)) err(k, 'unknown property');
  for (const k of REQUIRED_FIELDS) if (m[k] === undefined) err(k, 'required');

  const text = (v: unknown, path: string, max: number, required = true): boolean => {
    if (v === undefined && !required) return true;
    if (typeof v !== 'string' || !v.trim()) {
      err(path, 'expected a non-empty string');
      return false;
    }
    if (v.length > max) err(path, `at most ${max} characters`);
    else if (!plainText(v)) err(path, 'control characters are not allowed');
    return true;
  };
  const localized = (v: unknown, path: string, max: number) => {
    if (!isObject(v)) {
      err(path, 'expected { "vi": "…", "en": "…" }');
      return;
    }
    for (const k of Object.keys(v))
      if (k !== 'vi' && k !== 'en') err(`${path}.${k}`, 'unknown language');
    text(v.vi, `${path}.vi`, max);
    text(v.en, `${path}.en`, max);
  };
  const apps = (v: unknown, path: string): AddinApp[] => {
    if (!Array.isArray(v) || v.length === 0) {
      err(path, 'expected a non-empty list of doc, sheet, slide');
      return [];
    }
    const out: AddinApp[] = [];
    v.forEach((a, i) => {
      if (!ADDIN_APPS.includes(a as AddinApp)) err(`${path}[${i}]`, 'expected doc, sheet or slide');
      else if (out.includes(a as AddinApp)) err(`${path}[${i}]`, 'duplicate');
      else out.push(a as AddinApp);
    });
    return out;
  };

  if (m.$schema !== undefined && typeof m.$schema !== 'string') err('$schema', 'expected a string');
  if (m.manifestVersion !== undefined && m.manifestVersion !== MANIFEST_VERSION) {
    err('manifestVersion', `expected ${MANIFEST_VERSION}`);
  }
  if (m.id !== undefined) {
    if (typeof m.id !== 'string' || m.id.length > MANIFEST_LIMITS.idChars || !ID_RE.test(m.id)) {
      err('id', 'expected a lower-case reverse domain name such as vn.example.hello');
    }
  }
  if (m.name !== undefined) localized(m.name, 'name', MANIFEST_LIMITS.nameChars);
  if (m.description !== undefined) {
    localized(m.description, 'description', MANIFEST_LIMITS.descriptionChars);
  }
  if (m.version !== undefined && (typeof m.version !== 'string' || !VERSION_RE.test(m.version))) {
    err('version', 'expected a semantic version such as 1.0.0');
  }
  if (m.publisher !== undefined) text(m.publisher, 'publisher', MANIFEST_LIMITS.publisherChars);

  const origins: string[] = [];
  if (m.allowedOrigins !== undefined) {
    const list = m.allowedOrigins;
    if (!Array.isArray(list) || list.length === 0)
      err('allowedOrigins', 'expected a non-empty list');
    else if (list.length > MANIFEST_LIMITS.origins) {
      err('allowedOrigins', `at most ${MANIFEST_LIMITS.origins} origins`);
    } else {
      list.forEach((o, i) => {
        if (!isAddinOrigin(o)) {
          err(`allowedOrigins[${i}]`, 'expected an origin https://host[:port] (no path)');
        } else if (origins.includes(o)) err(`allowedOrigins[${i}]`, 'duplicate');
        else origins.push(o);
      });
    }
  }
  if (m.startUrl !== undefined && !urlInOrigins(m.startUrl, origins)) {
    err('startUrl', 'expected an http(s) URL on one of allowedOrigins');
  }
  if (m.icon !== undefined && !urlInOrigins(m.icon, origins)) {
    err('icon', 'expected an http(s) URL on one of allowedOrigins');
  }

  const perms: string[] = [];
  if (m.permissions !== undefined) {
    const list = m.permissions;
    if (!Array.isArray(list)) err('permissions', 'expected a list');
    else if (list.length > MANIFEST_LIMITS.permissions) {
      err('permissions', `at most ${MANIFEST_LIMITS.permissions} permissions`);
    } else {
      list.forEach((p, i) => {
        const path = `permissions[${i}]`;
        if (typeof p !== 'string') err(path, 'expected a string');
        else if (perms.includes(p)) err(path, 'duplicate');
        else if (p.startsWith(NETWORK_PREFIX)) {
          if (!isAddinOrigin(p.slice(NETWORK_PREFIX.length))) {
            err(path, 'expected network:https://host[:port]');
          } else perms.push(p);
        } else if (!(BASE_PERMISSIONS as readonly string[]).includes(p)) {
          err(path, 'unknown permission');
        } else perms.push(p);
      });
    }
  }

  let appList: AddinApp[] = [];
  if (m.apps !== undefined) appList = apps(m.apps, 'apps');

  if (m.ribbon !== undefined) {
    const r = m.ribbon;
    if (!isObject(r)) err('ribbon', 'expected an object');
    else {
      for (const k of Object.keys(r)) {
        if (k !== 'tab' && k !== 'group' && k !== 'commands')
          err(`ribbon.${k}`, 'unknown property');
      }
      if (!perms.includes('ui:ribbon'))
        err('ribbon', 'ribbon commands need the ui:ribbon permission');
      if (r.tab !== undefined && (typeof r.tab !== 'string' || !TAB_RE.test(r.tab))) {
        err('ribbon.tab', 'expected a tab id such as home or insert');
      }
      if (r.group !== undefined) localized(r.group, 'ribbon.group', MANIFEST_LIMITS.labelChars);
      const cmds = r.commands;
      if (!Array.isArray(cmds) || cmds.length === 0) {
        err('ribbon.commands', 'expected a non-empty list');
      } else if (cmds.length > MANIFEST_LIMITS.commands) {
        err('ribbon.commands', `at most ${MANIFEST_LIMITS.commands} commands`);
      } else {
        const ids: string[] = [];
        cmds.forEach((c, i) => {
          const path = `ribbon.commands[${i}]`;
          if (!isObject(c)) {
            err(path, 'expected an object');
            return;
          }
          for (const k of Object.keys(c)) {
            if (!['id', 'label', 'tooltip', 'action', 'apps'].includes(k)) {
              err(`${path}.${k}`, 'unknown property');
            }
          }
          if (typeof c.id !== 'string' || !COMMAND_ID_RE.test(c.id)) {
            err(`${path}.id`, 'expected letters, digits, - or _ (starting with a letter)');
          } else if (ids.includes(c.id)) err(`${path}.id`, 'duplicate');
          else ids.push(c.id);
          localized(c.label, `${path}.label`, MANIFEST_LIMITS.labelChars);
          if (c.tooltip !== undefined) {
            localized(c.tooltip, `${path}.tooltip`, MANIFEST_LIMITS.tooltipChars);
          }
          if (c.action !== 'openPane' && c.action !== 'event') {
            err(`${path}.action`, 'expected openPane or event');
          }
          if (c.apps !== undefined) {
            for (const a of apps(c.apps, `${path}.apps`)) {
              if (appList.length && !appList.includes(a)) {
                err(`${path}.apps`, `${a} is not one of the manifest's apps`);
              }
            }
          }
        });
      }
    }
  }

  return errors.length
    ? { ok: false, errors }
    : { ok: true, manifest: value as unknown as AddinManifest };
}

/** Parses and validates manifest JSON text. */
export function parseManifest(json: string): ManifestResult {
  let value: unknown;
  try {
    value = JSON.parse(json);
  } catch (e) {
    return { ok: false, errors: [{ path: '', message: `invalid JSON: ${(e as Error).message}` }] };
  }
  return validateManifest(value);
}

/** The text of `t` in `lang` (`vi-VN` / `en-US` / `vi` / `en`). */
export function localize(t: LocalizedText, lang: string): string {
  return lang.toLowerCase().startsWith('en') ? t.en : t.vi;
}

/** Commands of `manifest` shown in `app`. */
export function ribbonCommands(manifest: AddinManifest, app: AddinApp): AddinRibbonCommand[] {
  if (!manifest.permissions.includes('ui:ribbon')) return [];
  return (manifest.ribbon?.commands ?? []).filter((c) => (c.apps ?? manifest.apps).includes(app));
}
