import type {
  DisplayCapabilities,
  DisplayScreen,
  DisplayService,
  DisplayTarget,
} from '../slideShow';
import type { MessageBus } from '../types';

/**
 * Audience screen on another monitor in the browser, by capability detection:
 * - Window Management API (W3C draft `window.getScreenDetails()`): list the
 *   screens, open the window at a screen's position;
 * - Presentation API (W3C `PresentationRequest`): the browser shows the page on a
 *   presentation display it picks; messages go over the presentation connection;
 * - otherwise a plain second window.
 */

/** The parts of the Window Management API used here (not in lib.dom yet). */
interface ScreenDetailed {
  availLeft: number;
  availTop: number;
  availWidth: number;
  availHeight: number;
  left?: number;
  top?: number;
  width: number;
  height: number;
  isPrimary?: boolean;
  label?: string;
}
interface ScreenDetails {
  screens: ScreenDetailed[];
}
type WithScreens = { getScreenDetails?: () => Promise<ScreenDetails> };

/** The parts of the Presentation API used here. */
interface PresentationConnectionLike {
  state: string;
  send(data: string): void;
  close(): void;
  terminate?(): void;
  addEventListener(type: 'message', fn: (e: MessageEvent) => void): void;
  removeEventListener(type: 'message', fn: (e: MessageEvent) => void): void;
}
interface PresentationRequestLike {
  start(): Promise<PresentationConnectionLike>;
}
type PresentationRequestCtor = new (urls: string[]) => PresentationRequestLike;
interface PresentationReceiverLike {
  connectionList: Promise<{
    connections: PresentationConnectionLike[];
    addEventListener(
      type: 'connectionavailable',
      fn: (e: { connection: PresentationConnectionLike }) => void,
    ): void;
  }>;
}

function presentationCtor(): PresentationRequestCtor | null {
  const g = globalThis as unknown as { PresentationRequest?: PresentationRequestCtor };
  return typeof g.PresentationRequest === 'function' ? g.PresentationRequest : null;
}

function receiver(): PresentationReceiverLike | null {
  if (typeof navigator === 'undefined') return null;
  const p = (navigator as unknown as { presentation?: { receiver?: PresentationReceiverLike } })
    .presentation;
  return p?.receiver ?? null;
}

/** Message bus over presentation connections (JSON text messages). */
export class PresentationBus implements MessageBus {
  private readonly handlers = new Set<(m: unknown) => void>();
  private readonly conns: PresentationConnectionLike[] = [];
  private readonly queue: string[] = [];
  private readonly onMsg = (e: MessageEvent) => {
    let m: unknown;
    try {
      m = typeof e.data === 'string' ? JSON.parse(e.data) : null;
    } catch {
      return;
    }
    for (const h of this.handlers) h(m);
  };

  add(c: PresentationConnectionLike): void {
    this.conns.push(c);
    c.addEventListener('message', this.onMsg);
    for (const q of this.queue.splice(0)) this.sendTo(c, q);
  }

  private sendTo(c: PresentationConnectionLike, s: string) {
    try {
      c.send(s);
    } catch {
      // A connection that is not open yet or closed: dropped.
    }
  }

  post(msg: unknown): void {
    const s = JSON.stringify(msg);
    if (this.conns.length === 0) this.queue.push(s);
    for (const c of this.conns) this.sendTo(c, s);
  }

  onMessage(handler: (msg: unknown) => void): () => void {
    this.handlers.add(handler);
    return () => this.handlers.delete(handler);
  }

  close(): void {
    for (const c of this.conns) {
      c.removeEventListener('message', this.onMsg);
      try {
        if (c.terminate) c.terminate();
        else c.close();
      } catch {
        // Already closed.
      }
    }
    this.conns.length = 0;
    this.handlers.clear();
  }
}

/** In a page started by the Presentation API: the bus to the presenting page. */
export function presentationReceiverBus(): MessageBus | null {
  const r = receiver();
  if (!r) return null;
  const bus = new PresentationBus();
  void r.connectionList
    .then((list) => {
      for (const c of list.connections) bus.add(c);
      list.addEventListener('connectionavailable', (e) => bus.add(e.connection));
    })
    .catch(() => {});
  return bus;
}

export class WebDisplays implements DisplayService {
  capabilities(): DisplayCapabilities {
    const w = typeof window === 'undefined' ? null : (window as unknown as WithScreens);
    return {
      screens: typeof w?.getScreenDetails === 'function',
      presentation: presentationCtor() !== null,
      window: typeof window !== 'undefined',
    };
  }

  async screens(): Promise<DisplayScreen[]> {
    const w = window as unknown as WithScreens;
    if (typeof w.getScreenDetails !== 'function') return [];
    const d = await w.getScreenDetails();
    return d.screens.map((s, i) => ({
      id: String(i),
      label: s.label || '',
      primary: !!s.isPrimary,
      left: s.availLeft,
      top: s.availTop,
      width: s.availWidth,
      height: s.availHeight,
    }));
  }

  async open(query: string, target: DisplayTarget): Promise<MessageBus | null> {
    const url = query.startsWith('?') ? `${location.pathname}${query}` : query;
    if (target.kind === 'presentation') {
      const Ctor = presentationCtor();
      if (!Ctor) throw new Error('display.noPresentation');
      const conn = await new Ctor([url]).start();
      const bus = new PresentationBus();
      bus.add(conn);
      return bus;
    }
    let features = 'noopener';
    if (target.kind === 'screen') {
      const s = (await this.screens()).find((x) => x.id === target.id);
      if (s) {
        features = `left=${s.left},top=${s.top},width=${s.width},height=${s.height},popup`;
      }
    }
    // A window placed on another screen keeps its opener so it can go full screen there.
    window.open(url, '_blank', features);
    return null;
  }
}
