/**
 * File protection shared by BlueDoc, BlueSheet and BlueSlide (CORE-SEC-001):
 * mark as final, read-only recommended / password to modify, restricted
 * editing with editable regions (BlueDoc), protected ranges (BlueSheet) and
 * sensitivity labels. Mirrors `bo_engine::security`.
 */
import type { Pos, SelRect } from './types';

/** BlueOffice label (custom properties `BO_Label_*`). */
export interface LabelSpec {
  /** Identifier from the administrator's list. */
  id?: string | null;
  name: string;
  /** Marking text placed in the header (BlueDoc). */
  marking?: string | null;
}

/** A label another product wrote into the file (kept, shown read-only). */
export interface ForeignLabel {
  id: string;
  name?: string | null;
  enabled: boolean;
  method?: string | null;
  siteId?: string | null;
}

/** Restricted editing (`w:documentProtection`, ECMA-376 Part 1, §17.15.1.29). */
export interface DocProtection {
  edit: string;
  enforced: boolean;
  hasPassword: boolean;
}

/** Editable region (`w:permStart` / `w:permEnd`, §17.13.7). */
export interface EditableRegion {
  id: string;
  group?: string | null;
  editor?: string | null;
  /** The current user may edit it. */
  mine: boolean;
  start: Pos;
  end: Pos;
}

/** Protected range of a sheet (`protectedRange`, §18.3.1.71). */
export interface ProtectedRangeInfo {
  name: string;
  sqref: string;
  hasPassword: boolean;
  hasSecurityDescriptor: boolean;
  /** Editable in this session (no password, or unlocked). */
  unlocked: boolean;
}

export interface SecurityInfo {
  markedFinal: boolean;
  readOnlyRecommended: boolean;
  writePassword: boolean;
  /** The session is read-only now. */
  readOnly: boolean;
  label?: LabelSpec | null;
  foreignLabels: ForeignLabel[];
  protection?: DocProtection;
  regions?: EditableRegion[];
  regionRects?: SelRect[];
  showRegions: boolean;
  identity: string[];
}

/** Security commands accepted by `EngineHost.command` (BlueDoc),
 * `EngineHost.slideCommand` (BlueSlide) and `SheetHost.exec` (BlueSheet). */
export type SecurityCommand =
  | { cmd: 'security' }
  | { cmd: 'setIdentity'; names: string[] }
  | { cmd: 'setReadOnly'; on: boolean }
  | { cmd: 'unlockWrite'; password: string }
  | { cmd: 'markFinal'; on: boolean }
  | { cmd: 'setReadOnlyRecommended'; on: boolean }
  | { cmd: 'setWritePassword'; password: string }
  | { cmd: 'setLabel'; label?: LabelSpec | null }
  | { cmd: 'protectDocument'; password?: string }
  | { cmd: 'unprotectDocument'; password?: string }
  | { cmd: 'addEditableRegion'; editor?: string | null }
  | { cmd: 'removeEditableRegion'; id: string }
  | { cmd: 'nextEditableRegion'; forward?: boolean }
  | { cmd: 'showEditableRegions'; on: boolean }
  | { cmd: 'protectedRanges'; sheet: number }
  | { cmd: 'addProtectedRange'; sheet: number; name: string; sqref: string; password?: string }
  | { cmd: 'removeProtectedRange'; sheet: number; name: string }
  | { cmd: 'unlockProtectedRange'; sheet: number; name: string; password?: string };

/** Answer of a security command sent to `SheetHost.exec`. */
export interface SheetSecurityResult {
  security: SecurityInfo;
  ranges?: ProtectedRangeInfo[];
  error?: string;
}

/** Engine error messages (hosts translate them). */
export const SECURITY_ERRORS = {
  readOnly: 'read only',
  documentProtected: 'document protected',
  writePasswordRequired: 'write password required',
  wrongPassword: 'wrong protection password',
  unsupportedHash: 'unsupported password format',
} as const;

/** The editor opens the file asking how: final notice, or read-only prompt. */
export type OpenProtection = 'none' | 'final' | 'recommended' | 'password';

/** How a file with this state should be offered on open. */
export function openProtection(s: SecurityInfo): OpenProtection {
  if (!s.readOnly) return 'none';
  if (s.markedFinal) return 'final';
  if (s.writePassword) return 'password';
  if (s.readOnlyRecommended) return 'recommended';
  return 'none';
}

/** Label name for the status bar: BlueOffice's own, else an applied foreign one. */
export function labelName(s: SecurityInfo | null | undefined): string | null {
  if (!s) return null;
  if (s.label?.name) return s.label.name;
  const f = s.foreignLabels.find((l) => l.enabled && l.name);
  return f?.name ?? null;
}

/**
 * The administrator's list of labels: `window.__BLUEOFFICE_LABELS__` (set by
 * the deployment, e.g. in a config script) or the `blueoffice.labels`
 * storage key, as JSON `LabelSpec[]`; else BlueOffice's defaults.
 */
export function configuredLabels(defaults: LabelSpec[]): LabelSpec[] {
  const fromWindow = (globalThis as { __BLUEOFFICE_LABELS__?: unknown }).__BLUEOFFICE_LABELS__;
  const parse = (v: unknown): LabelSpec[] | null =>
    Array.isArray(v)
      ? v.filter(
          (x): x is LabelSpec =>
            !!x && typeof x === 'object' && typeof (x as LabelSpec).name === 'string',
        )
      : null;
  const w = parse(fromWindow);
  if (w && w.length) return w;
  try {
    const raw = globalThis.localStorage?.getItem('blueoffice.labels');
    const s = raw ? parse(JSON.parse(raw)) : null;
    if (s && s.length) return s;
  } catch {
    // Ignore unreadable settings.
  }
  return defaults;
}
