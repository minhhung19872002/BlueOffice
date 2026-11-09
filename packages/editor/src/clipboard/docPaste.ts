import type { DocPasteMode } from '@blueoffice/platform';

/** How a BlueDoc paste can be shown: an engine paste mode or a picture. */
export type DocPasteChoice = DocPasteMode | 'picture';

/** Where pasted content comes from (each has its own default mode). */
export type PasteSource = 'same' | 'other' | 'plain';

/** Default paste modes, per source (kept per user in local storage). */
export interface PasteDefaults {
  /** Copied in this document. */
  same: 'keep' | 'merge' | 'text';
  /** Formatted content from another document or program. */
  other: 'keep' | 'merge' | 'text';
  /** Plain text: as text, or a link when it is a single URL / e-mail address. */
  plain: 'text' | 'link';
}

export const DEFAULT_PASTE: PasteDefaults = { same: 'keep', other: 'keep', plain: 'text' };

const STORAGE_KEY = 'bo.pasteDefaults';

const pick = <T extends string>(v: unknown, allowed: readonly T[], fallback: T): T =>
  typeof v === 'string' && (allowed as readonly string[]).includes(v) ? (v as T) : fallback;

/** Default paste modes kept by this browser / app. */
export function loadPasteDefaults(): PasteDefaults {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    const v = (raw ? JSON.parse(raw) : {}) as Record<string, unknown>;
    const fmt = ['keep', 'merge', 'text'] as const;
    return {
      same: pick(v.same, fmt, DEFAULT_PASTE.same),
      other: pick(v.other, fmt, DEFAULT_PASTE.other),
      plain: pick(v.plain, ['text', 'link'] as const, DEFAULT_PASTE.plain),
    };
  } catch {
    return { ...DEFAULT_PASTE };
  }
}

export function savePasteDefaults(d: PasteDefaults): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(d));
  } catch {
    // Storage unavailable (private mode): the defaults last for this session only.
  }
}

/** Single URL or e-mail address (mirrors `bo_edit::paste::clipboard_link`). */
export function isLink(text: string): boolean {
  const t = text.trim();
  if (!t || /\s/.test(t)) return false;
  if (/^(https?:\/\/|ftp:\/\/|mailto:)./i.test(t)) return true;
  if (/^www\.[^.]+\..+/i.test(t)) return true;
  return /^[A-Za-z0-9._+-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+$/.test(t);
}

/** What one paste holds; kept so the paste can be redone in another mode. */
export interface DocPasteRecord {
  text: string;
  html: string;
  internal: string;
  /** Picture (PNG, base64) of a BlueOffice copy, or a picture from the clipboard. */
  png?: string;
  /** The clipboard holds a picture only. */
  pictureOnly?: boolean;
  source: PasteSource;
  /** Mode of the paste as it is now in the document. */
  mode: DocPasteChoice;
}

/** Modes the paste options offer for a record, in menu order. */
export function pasteChoices(
  r: Pick<DocPasteRecord, 'text' | 'html' | 'internal' | 'png' | 'pictureOnly'>,
): DocPasteChoice[] {
  if (r.pictureOnly) return r.png ? ['picture'] : [];
  const out: DocPasteChoice[] = [];
  const formatted = !!(r.html || r.internal);
  if (formatted) out.push('keep', 'merge');
  if (r.text || formatted) out.push('text');
  if (r.png) out.push('picture');
  if (isLink(r.text)) out.push('link');
  return out;
}

/** Mode of a new paste under the user's defaults. */
export function defaultChoice(
  r: Pick<DocPasteRecord, 'text' | 'html' | 'internal' | 'png' | 'pictureOnly' | 'source'>,
  d: PasteDefaults,
): DocPasteChoice {
  if (r.pictureOnly) return 'picture';
  if (r.source === 'plain') return d.plain === 'link' && isLink(r.text) ? 'link' : 'text';
  return r.source === 'same' ? d.same : d.other;
}

/** Bytes of a base64 string. */
export function fromBase64(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Base64 of bytes. */
export function toBase64(bytes: Uint8Array): string {
  let bin = '';
  const step = 0x8000;
  for (let i = 0; i < bytes.length; i += step) {
    bin += String.fromCharCode(...bytes.subarray(i, i + step));
  }
  return btoa(bin);
}
