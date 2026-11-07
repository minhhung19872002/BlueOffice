import type { MessageBus, Platform } from '@blueoffice/platform';

/**
 * Presenter ↔ audience screen protocol over {@link Platform.channel}.
 * The audience window asks for the deck (`hello`), gets it once (`deck`,
 * base64 so it also travels as JSON on desktop) and then follows `show`.
 */
export type AudienceMsg =
  | { type: 'hello' }
  | { type: 'deck'; name: string; data: string; slide: number; black: boolean }
  | { type: 'show'; slide: number; black: boolean; ended: boolean };

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
  private state = { slide: 0, black: false, ended: false };

  constructor(
    private readonly platform: Platform,
    private readonly deck: () => Promise<{ name: string; bytes: Uint8Array }>,
  ) {}

  get open(): boolean {
    return this.bus !== null;
  }

  /** Open the audience screen (a second window) for a new channel. */
  async start(slide: number): Promise<void> {
    this.stop();
    const id = `audience-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    const bus = this.platform.channel?.(id);
    if (!bus) throw new Error('audience.noChannel');
    this.bus = bus;
    this.state = { slide, black: false, ended: false };
    this.off = bus.onMessage((m) => {
      if (isAudienceMsg(m) && m.type === 'hello') void this.sendDeck();
    });
    await this.platform.window.openNew(`?app=slide&audience=${encodeURIComponent(id)}`);
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

  /** Current slide / black screen / end of the show. */
  update(slide: number, black: boolean, ended: boolean): void {
    this.state = { slide, black, ended };
    this.bus?.post({ type: 'show', slide, black, ended } satisfies AudienceMsg);
  }

  stop(): void {
    this.off?.();
    this.off = null;
    this.bus?.close();
    this.bus = null;
  }
}
