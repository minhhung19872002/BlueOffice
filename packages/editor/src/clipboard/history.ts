import { useSyncExternalStore } from 'react';

/** App an item was copied in. */
export type ClipApp = 'doc' | 'sheet' | 'slide';

/** What a copied item holds. */
export type ClipKind = 'text' | 'formatted' | 'cells' | 'shapes' | 'slides' | 'picture';

/** One copy / cut made inside BlueOffice. */
export interface ClipItem {
  id: number;
  app: ClipApp;
  kind: ClipKind;
  /** Plain-text form (preview, and what other apps paste). */
  text: string;
  /** HTML form (BlueDoc). */
  html?: string;
  /** BlueDoc internal form (lossless within the suite). */
  internal?: string;
  /** Picture of the content (PNG, base64). */
  png?: string;
  /** The copy as the app that made it keeps it (BlueSheet: editor instance + engine clip
   * id), so the item pastes with its formats there; not compared between items. */
  ref?: string;
  /** Unix ms. */
  time: number;
}

/** Most items kept. */
export const CLIP_HISTORY_MAX = 24;

/** Pictures larger than this (base64 characters) are not kept in session storage. */
const STORED_PNG_MAX = 400_000;

const STORAGE_KEY = 'bo.clipboardHistory';

/** Minimal storage (sessionStorage in the apps, a map in tests). */
export interface HistoryStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

function sessionStore(): HistoryStorage | null {
  try {
    return typeof sessionStorage === 'undefined' ? null : sessionStorage;
  } catch {
    return null;
  }
}

function isItem(v: unknown): v is ClipItem {
  if (!v || typeof v !== 'object') return false;
  const o = v as Record<string, unknown>;
  return typeof o.id === 'number' && typeof o.text === 'string' && typeof o.app === 'string';
}

/**
 * Clipboard history of the session (CORE-PASTE-001): the last {@link CLIP_HISTORY_MAX}
 * items copied or cut in BlueDoc, BlueSheet and BlueSlide. One instance per window is
 * shared by the three apps; it is also kept in session storage so a reload keeps it.
 */
export class ClipboardHistory {
  private items: ClipItem[] = [];
  private listeners = new Set<() => void>();
  private nextId = 1;

  constructor(private readonly storage: HistoryStorage | null = sessionStore()) {
    try {
      const raw = storage?.getItem(STORAGE_KEY);
      const v: unknown = raw ? JSON.parse(raw) : [];
      if (Array.isArray(v)) this.items = v.filter(isItem).slice(0, CLIP_HISTORY_MAX);
    } catch {
      this.items = [];
    }
    this.nextId = this.items.reduce((m, i) => Math.max(m, i.id), 0) + 1;
  }

  /** Items, newest first. */
  list(): readonly ClipItem[] {
    return this.items;
  }

  /** Add a copy (an equal newest item is replaced, so repeated copies do not pile up). */
  push(item: Omit<ClipItem, 'id' | 'time'>, now = Date.now()): ClipItem | null {
    if (!item.text && !item.png && !item.internal) return null;
    const same = (i: ClipItem) =>
      i.app === item.app &&
      i.kind === item.kind &&
      i.text === item.text &&
      (i.internal ?? '') === (item.internal ?? '');
    const added: ClipItem = { ...item, id: this.nextId++, time: now };
    this.items = [added, ...this.items.filter((i) => !same(i))].slice(0, CLIP_HISTORY_MAX);
    this.changed();
    return added;
  }

  remove(id: number): void {
    const n = this.items.length;
    this.items = this.items.filter((i) => i.id !== id);
    if (this.items.length !== n) this.changed();
  }

  clear(): void {
    if (!this.items.length) return;
    this.items = [];
    this.changed();
  }

  get(id: number): ClipItem | undefined {
    return this.items.find((i) => i.id === id);
  }

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  private changed(): void {
    try {
      const kept = this.items.map((i) =>
        i.png && i.png.length > STORED_PNG_MAX ? { ...i, png: undefined } : i,
      );
      if (kept.length) this.storage?.setItem(STORAGE_KEY, JSON.stringify(kept));
      else this.storage?.removeItem(STORAGE_KEY);
    } catch {
      // Storage full or unavailable: the history lasts for this page only.
    }
    for (const l of this.listeners) l();
  }
}

/** The history shared by the apps of this window. */
export const clipboardHistory = new ClipboardHistory();

/** Items of a history, re-rendering when it changes. */
export function useClipboardHistory(h: ClipboardHistory = clipboardHistory): readonly ClipItem[] {
  return useSyncExternalStore(h.subscribe, () => h.list());
}
