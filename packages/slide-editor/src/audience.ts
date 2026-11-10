import type { DisplayTarget, MessageBus, Platform, SlideInkStroke } from '@blueoffice/platform';

/**
 * Presenter ↔ audience screen protocol over {@link Platform.channel}.
 * The audience window asks for the deck (`hello`), gets it once (`deck`,
 * base64 so it also travels as JSON on desktop) and then follows `show`.
 */
export type AudienceMsg =
  | { type: 'hello' }
  | { type: 'deck'; name: string; data: string; slide: number; black: boolean }
  | {
      type: 'show';
      slide: number;
      black: boolean;
      ended: boolean;
      /** Click steps played on the slide and how they were reached (animations). */
      click?: number;
      enter?: 'transition' | 'click' | 'jump';
      /** Presenter tools mirrored on the audience screen (SLIDE-SHOW-001). */
      tools?: AudienceTools;
      /** Zoom transition into / out of a zoom (SLIDE-ZOOM-001). */
      zoom?: import('./presenting').ZoomMove;
    };

/** White screen, zoom (x, y as fractions of the stage) and the slide's ink. */
export interface AudienceTools {
  white: boolean;
  zoom: { scale: number; x: number; y: number };
  ink: SlideInkStroke[];
}

export function toBase64(bytes: Uint8Array): string {
  let s = '';
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    s += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(s);
}

export function fromBase64(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

export function isAudienceMsg(m: unknown): m is AudienceMsg {
  return (
    typeof m === 'object' &&
    m !== null &&
    ['hello', 'deck', 'show'].includes((m as { type?: string }).type ?? '')
  );
}

/** Presenter side: opens the audience window and keeps it in step. */
export class AudienceLink {
  private bus: MessageBus | null = null;
  private off: (() => void) | null = null;
  private state: { slide: number; black: boolean; ended: boolean; click?: number } = {
    slide: 0,
    black: false,
    ended: false,
  };

  constructor(
    private readonly platform: Platform,
    private readonly deck: () => Promise<{ name: string; bytes: Uint8Array }>,
  ) {}

  get open(): boolean {
    return this.bus !== null;
  }

  /**
   * Open the audience screen for a new channel: a second window, or — where the
   * platform has `displays` — a window on another screen or a presentation display.
   */
  async start(slide: number, target: DisplayTarget = { kind: 'window' }): Promise<void> {
    this.stop();
    const id = `audience-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    const query = `?app=slide&audience=${encodeURIComponent(id)}`;
    this.state = { slide, black: false, ended: false };
    const displays = this.platform.displays;
    if (displays && target.kind === 'presentation') {
      // The presentation display is reached through its own connection.
      const own = await displays.open(query, target);
      if (!own) throw new Error('audience.noChannel');
      this.attach(own);
      return;
    }
    const bus = this.platform.channel?.(id);
    if (!bus) throw new Error('audience.noChannel');
    this.attach(bus);
    if (displays && target.kind === 'screen') await displays.open(query, target);
    else await this.platform.window.openNew(query);
  }

  private attach(bus: MessageBus): void {
    this.bus = bus;
    this.off = bus.onMessage((m) => {
      if (isAudienceMsg(m) && m.type === 'hello') void this.sendDeck();
    });
  }

  private async sendDeck(): Promise<void> {
    const d = await this.deck();
    this.bus?.post({
      type: 'deck',
      name: d.name,
      data: toBase64(d.bytes),
      slide: this.state.slide,
      black: this.state.black,
    } satisfies AudienceMsg);
  }

  /** Current slide (and click step) / black screen / end of the show. */
  update(
    slide: number,
    black: boolean,
    ended: boolean,
    click?: number,
    enter?: 'transition' | 'click' | 'jump',
    tools?: AudienceTools,
    zoom?: import('./presenting').ZoomMove,
  ): void {
    this.state = { slide, black, ended, click };
    this.bus?.post({
      type: 'show',
      slide,
      black,
      ended,
      click,
      enter,
      tools,
      zoom,
    } satisfies AudienceMsg);
  }

  stop(): void {
    this.off?.();
    this.off = null;
    this.bus?.close();
    this.bus = null;
  }
}
