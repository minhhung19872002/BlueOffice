/**
 * Windows of one document and of several documents (CORE-UI-007, View › Window):
 *
 *  - split panes: the divider position of two panes of the same document;
 *  - synchronous scrolling of two windows ("View Side by Side"): scroll positions travel
 *    as fractions of the scrollable extent, so documents of different lengths, zooms and
 *    window sizes stay aligned proportionally;
 *  - the roster of open windows (who can be paired), over the platform's window channel
 *    (`Platform.channel`: BroadcastChannel on the web, Tauri events on the desktop);
 *  - "New Window": the same document in a second window. The document is handed over
 *    through the offline cache and both windows then join a local session on the window
 *    channel (`@blueoffice/hosted` `joinWindowSession`), so edits appear in both.
 */
import type { MessageBus, OfflineCache, Platform } from './types';

// ---------------------------------------------------------------- split panes

/** The first pane's share of the split axis; both panes keep at least `SPLIT_MIN`. */
export const SPLIT_MIN = 0.15;
export const SPLIT_DEFAULT = 0.5;
/** Dragging the divider this close to an edge removes the split. */
export const SPLIT_REMOVE = 0.05;

export function clampSplit(ratio: number): number {
  if (!Number.isFinite(ratio)) return SPLIT_DEFAULT;
  return Math.min(1 - SPLIT_MIN, Math.max(SPLIT_MIN, ratio));
}

/** Pixel sizes of the two panes for a container of `total` px and a `divider` px wide bar. */
export function splitSizes(total: number, ratio: number, divider = 6): [number, number] {
  const room = Math.max(0, total - divider);
  const first = Math.round(room * clampSplit(ratio));
  return [first, room - first];
}

/**
 * The ratio after dragging the divider to `pos` px from the start of the container;
 * `null` when it was dropped at an edge (the split is removed).
 */
export function dragSplit(total: number, pos: number, divider = 6): number | null {
  const room = Math.max(1, total - divider);
  const ratio = (pos - divider / 2) / room;
  if (ratio < SPLIT_REMOVE || ratio > 1 - SPLIT_REMOVE) return null;
  return clampSplit(ratio);
}

/** Scroll position and zoom of one pane; the panes of a split keep their own. */
export interface PaneView {
  x: number;
  y: number;
  zoom: number;
}

/** Two panes of one document. */
export interface SplitPanes {
  ratio: number;
  panes: [PaneView, PaneView];
  /** The pane that takes the keyboard (0 or 1). */
  active: 0 | 1;
}

/** Splitting shows the same place in both panes; they scroll and zoom apart from then on. */
export function splitPanes(current: PaneView, ratio = SPLIT_DEFAULT): SplitPanes {
  return { ratio: clampSplit(ratio), panes: [{ ...current }, { ...current }], active: 0 };
}

/** Change one pane; the other is untouched. */
export function updatePane(s: SplitPanes, pane: 0 | 1, change: Partial<PaneView>): SplitPanes {
  const panes: [PaneView, PaneView] = [{ ...s.panes[0] }, { ...s.panes[1] }];
  panes[pane] = { ...panes[pane], ...change };
  return { ...s, panes };
}

/** Removing the split keeps the view of the active pane. */
export function unsplitPanes(s: SplitPanes): PaneView {
  return { ...s.panes[s.active] };
}

// ---------------------------------------------------------------- synchronous scrolling

/** What a scrollable view reports (`scrollTop`, `scrollHeight - clientHeight`, …). */
export interface ScrollMetrics {
  top: number;
  left: number;
  maxTop: number;
  maxLeft: number;
}

/** A scroll position as fractions (0…1) of the scrollable extent. */
export interface ScrollFraction {
  fx: number;
  fy: number;
}

const frac = (pos: number, max: number) => (max > 0 ? Math.min(1, Math.max(0, pos / max)) : 0);

export function scrollFraction(m: ScrollMetrics): ScrollFraction {
  return { fx: frac(m.left, m.maxLeft), fy: frac(m.top, m.maxTop) };
}

/** The position `f` (+ `offset`) maps to in a view with the extent of `m`. */
export function fractionToScroll(
  f: ScrollFraction,
  m: Pick<ScrollMetrics, 'maxTop' | 'maxLeft'>,
  offset: ScrollFraction = { fx: 0, fy: 0 },
): { top: number; left: number } {
  const clamp = (v: number) => Math.min(1, Math.max(0, v));
  return {
    top: Math.round(clamp(f.fy + offset.fy) * m.maxTop),
    left: Math.round(clamp(f.fx + offset.fx) * m.maxLeft),
  };
}

/**
 * One side of a synchronous-scrolling pair. Pairing keeps the distance the two views had
 * (`offset` = mine − theirs), so each can be positioned first and then scrolled together;
 * `reset` drops the distance: both views show the same relative place.
 */
export class SyncScroll {
  private offset: ScrollFraction = { fx: 0, fy: 0 };
  /** Position set by the last remote scroll (its own scroll event is not sent back). */
  private applied: { top: number; left: number } | null = null;

  /** Start (or re-base) with the positions both sides have now. */
  pair(mine: ScrollMetrics, theirs: ScrollFraction): void {
    const me = scrollFraction(mine);
    this.offset = { fx: me.fx - theirs.fx, fy: me.fy - theirs.fy };
    this.applied = null;
  }

  /** "Reset position": no distance any more; returns where this view goes for `theirs`. */
  reset(mine: ScrollMetrics, theirs: ScrollFraction): { top: number; left: number } {
    this.offset = { fx: 0, fy: 0 };
    return this.remote(theirs, mine);
  }

  /**
   * This view scrolled: the fraction to send (already converted to the peer's frame), or
   * `null` when the scroll is the echo of a remote one.
   */
  local(m: ScrollMetrics): ScrollFraction | null {
    const a = this.applied;
    if (a && Math.abs(a.top - m.top) <= 1 && Math.abs(a.left - m.left) <= 1) return null;
    this.applied = null;
    const me = scrollFraction(m);
    return { fx: me.fx - this.offset.fx, fy: me.fy - this.offset.fy };
  }

  /** The peer scrolled to `f` (in this view's frame once the offset is added). */
  remote(f: ScrollFraction, m: ScrollMetrics): { top: number; left: number } {
    const to = fractionToScroll(f, m, this.offset);
    this.applied = to;
    return to;
  }
}

// ---------------------------------------------------------------- roster of windows

export type WindowApp = 'doc' | 'sheet' | 'slide';

export interface WindowPeer {
  id: string;
  app: WindowApp;
  /** Document name shown when choosing the window to compare with. */
  title: string;
}

export type WindowMessage =
  | { t: 'hello'; peer: WindowPeer }
  | { t: 'here'; peer: WindowPeer }
  | { t: 'bye'; id: string }
  /** Start / stop side by side with `to`; `f` is the sender's position. */
  | { t: 'pair'; from: string; to: string; on: boolean; f: ScrollFraction }
  | { t: 'paired'; from: string; to: string; f: ScrollFraction }
  | { t: 'scroll'; from: string; to: string; f: ScrollFraction }
  | { t: 'reset'; from: string; to: string; f: ScrollFraction };

export const WINDOWS_CHANNEL = 'windows';

const isMessage = (m: unknown): m is WindowMessage =>
  !!m && typeof m === 'object' && typeof (m as { t?: unknown }).t === 'string';

export const newWindowId = (): string =>
  `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;

/** The view a window scrolls in sync (the editor's main scroller). */
export interface ScrollTarget {
  metrics(): ScrollMetrics;
  scrollTo(pos: { top: number; left: number }): void;
  /** Calls `fn` on every scroll; returns the unsubscribe function. */
  onScroll(fn: () => void): () => void;
}

export interface WindowLinkEvents {
  /** The open windows of the same app changed. */
  peers: (peers: WindowPeer[]) => void;
  /** Side by side started (`peer`) or ended (`null`). */
  paired: (peer: WindowPeer | null) => void;
}

/**
 * This window among the others: announces itself, lists the other windows of the same app
 * and runs side-by-side synchronous scrolling with one of them.
 */
export class WindowLink {
  private readonly others = new Map<string, WindowPeer>();
  private readonly off: (() => void)[] = [];
  private readonly sync = new SyncScroll();
  private partner: string | null = null;
  private syncOn = true;
  private unScroll: (() => void) | null = null;
  private readonly listeners: { [K in keyof WindowLinkEvents]: Set<WindowLinkEvents[K]> } = {
    peers: new Set(),
    paired: new Set(),
  };

  constructor(
    private readonly bus: MessageBus,
    private me: WindowPeer,
    private readonly target: () => ScrollTarget | null,
  ) {
    this.off.push(bus.onMessage((m) => this.receive(m)));
    bus.post({ t: 'hello', peer: me } satisfies WindowMessage);
  }

  on<K extends keyof WindowLinkEvents>(event: K, fn: WindowLinkEvents[K]): () => void {
    this.listeners[event].add(fn);
    return () => this.listeners[event].delete(fn);
  }

  get id(): string {
    return this.me.id;
  }

  /** Other open windows of the same app. */
  peers(): WindowPeer[] {
    return [...this.others.values()].filter((p) => p.app === this.me.app);
  }

  get pairedWith(): WindowPeer | null {
    return this.partner ? (this.others.get(this.partner) ?? null) : null;
  }

  get synchronous(): boolean {
    return this.syncOn;
  }

  /** The document name changed. */
  setTitle(title: string): void {
    if (title === this.me.title) return;
    this.me = { ...this.me, title };
    this.bus.post({ t: 'here', peer: this.me } satisfies WindowMessage);
  }

  /** View side by side with `peerId` (synchronous scrolling on). */
  pair(peerId: string): void {
    if (!this.others.has(peerId)) return;
    if (this.partner && this.partner !== peerId) this.unpair();
    const f = this.fraction();
    this.bus.post({ t: 'pair', from: this.me.id, to: peerId, on: true, f } satisfies WindowMessage);
    // The answer (`paired`) carries the peer's position; until then assume the same one.
    this.start(peerId, f);
  }

  unpair(): void {
    const to = this.partner;
    if (!to) return;
    this.bus.post({
      t: 'pair',
      from: this.me.id,
      to,
      on: false,
      f: this.fraction(),
    } satisfies WindowMessage);
    this.stop();
  }

  /** Synchronous scrolling on / off while side by side (this window only sends when on). */
  setSynchronous(on: boolean): void {
    if (on === this.syncOn) return;
    this.syncOn = on;
    // Scrolling apart and turning it on again keeps the new distance.
    const t = this.target();
    if (on && t) this.sync.pair(t.metrics(), this.lastPeer);
  }

  /** Both windows go to the same relative position. */
  resetPosition(): void {
    const to = this.partner;
    const t = this.target();
    if (!to || !t) return;
    const f = scrollFraction(t.metrics());
    this.sync.reset(t.metrics(), f);
    this.lastPeer = f;
    this.bus.post({ t: 'reset', from: this.me.id, to, f } satisfies WindowMessage);
  }

  dispose(): void {
    if (this.partner) this.unpair();
    this.bus.post({ t: 'bye', id: this.me.id } satisfies WindowMessage);
    for (const f of this.off.splice(0)) f();
    this.unScroll?.();
    this.bus.close();
  }

  private lastPeer: ScrollFraction = { fx: 0, fy: 0 };

  private fraction(): ScrollFraction {
    const t = this.target();
    return t ? scrollFraction(t.metrics()) : { fx: 0, fy: 0 };
  }

  private start(peerId: string, theirs: ScrollFraction): void {
    this.partner = peerId;
    this.lastPeer = theirs;
    const t = this.target();
    if (t) {
      this.sync.pair(t.metrics(), theirs);
      this.unScroll?.();
      this.unScroll = t.onScroll(() => this.scrolled());
    }
    for (const f of this.listeners.paired) f(this.pairedWith);
  }

  private stop(): void {
    this.partner = null;
    this.unScroll?.();
    this.unScroll = null;
    for (const f of this.listeners.paired) f(null);
  }

  private scrolled(): void {
    const to = this.partner;
    const t = this.target();
    if (!to || !t) return;
    const f = this.sync.local(t.metrics());
    if (f && this.syncOn)
      this.bus.post({ t: 'scroll', from: this.me.id, to, f } satisfies WindowMessage);
  }

  private changed(): void {
    const peers = this.peers();
    for (const f of this.listeners.peers) f(peers);
  }

  private receive(raw: unknown): void {
    if (!isMessage(raw)) return;
    const m = raw;
    switch (m.t) {
      case 'hello':
        if (m.peer.id === this.me.id) return;
        this.others.set(m.peer.id, m.peer);
        this.bus.post({ t: 'here', peer: this.me } satisfies WindowMessage);
        this.changed();
        break;
      case 'here':
        if (m.peer.id === this.me.id) return;
        this.others.set(m.peer.id, m.peer);
        this.changed();
        break;
      case 'bye':
        if (!this.others.delete(m.id)) return;
        if (this.partner === m.id) this.stop();
        this.changed();
        break;
      case 'pair':
        if (m.to !== this.me.id) return;
        if (m.on) {
          if (this.partner && this.partner !== m.from) this.unpair();
          this.start(m.from, m.f);
          this.bus.post({
            t: 'paired',
            from: this.me.id,
            to: m.from,
            f: this.fraction(),
          } satisfies WindowMessage);
        } else if (this.partner === m.from) {
          this.stop();
        }
        break;
      case 'paired': {
        if (m.to !== this.me.id || this.partner !== m.from) return;
        this.lastPeer = m.f;
        const t = this.target();
        if (t) this.sync.pair(t.metrics(), m.f);
        break;
      }
      case 'scroll':
      case 'reset': {
        if (m.to !== this.me.id || this.partner !== m.from) return;
        const t = this.target();
        if (!t) return;
        this.lastPeer = m.f;
        if (m.t === 'reset') t.scrollTo(this.sync.reset(t.metrics(), m.f));
        else if (this.syncOn) t.scrollTo(this.sync.remote(m.f, t.metrics()));
        break;
      }
    }
  }
}

// ---------------------------------------------------------------- new window

/** URL parameter naming the local session of a window opened by "New Window". */
export const WINDOW_PARAM = 'window';

const enc = new TextEncoder();
const dec = new TextDecoder();
const bytesKey = (id: string) => `window:${id}`;
const nameKey = (id: string) => `window:${id}:name`;

/** Keeps the document for the windows of session `id` (read by each of them on start). */
export async function stashWindowDocument(
  cache: OfflineCache,
  id: string,
  file: { bytes: Uint8Array; name: string },
): Promise<void> {
  await cache.put(bytesKey(id), file.bytes);
  await cache.put(nameKey(id), enc.encode(file.name));
}

export async function takeWindowDocument(
  cache: OfflineCache,
  id: string,
): Promise<{ bytes: Uint8Array; name: string } | null> {
  const [bytes, name] = await Promise.all([cache.get(bytesKey(id)), cache.get(nameKey(id))]);
  return bytes && name ? { bytes, name: dec.decode(name) } : null;
}

export async function dropWindowDocument(cache: OfflineCache, id: string): Promise<void> {
  await Promise.all([cache.delete(bytesKey(id)), cache.delete(nameKey(id))]);
}

/** The URL (web) or app query (desktop) of a window of session `id`. */
export function windowSessionUrl(
  kind: Platform['kind'],
  app: WindowApp,
  id: string,
  location: { pathname: string; search: string },
): string {
  const q = new URLSearchParams(kind === 'desktop' ? '' : location.search);
  if (kind === 'desktop') q.set('app', app);
  q.set(WINDOW_PARAM, id);
  return `${kind === 'desktop' ? '' : location.pathname}?${q.toString()}`;
}

/**
 * "New Window": hands the document to a new local session, opens a second window on it and
 * moves this window into the same session (it reloads from the handed-over document, so the
 * undo history of this window starts over). Returns the session id.
 */
export async function openDocumentWindow(
  platform: Pick<Platform, 'kind' | 'offline' | 'window'>,
  app: WindowApp,
  file: { bytes: Uint8Array; name: string },
  env: {
    location: { pathname: string; search: string };
    /** Moves this window to `url` (default: `window.location.assign`). */
    navigate?: (url: string) => void;
    id?: string;
  } = { location: globalThis.location },
): Promise<string> {
  const id = env.id ?? newWindowId();
  await stashWindowDocument(platform.offline, id, file);
  const url = windowSessionUrl(platform.kind, app, id, env.location);
  await platform.window.openNew(url);
  const navigate = env.navigate ?? ((u: string) => globalThis.location.assign(u));
  navigate(url);
  return id;
}
