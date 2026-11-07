import type {
  EngineHost,
  SlideCommand,
  SlideEditResult,
  SlideTransform,
} from '@blueoffice/platform';

/**
 * Sends slide commands to the engine one at a time (the engine holds the
 * deck, the selection and the history) and reports every result. Drag
 * previews are coalesced: only the latest pointer position is sent.
 */
export class SlideController {
  private queue: Promise<unknown> = Promise.resolve();
  private pending: SlideCommand | null = null;
  private busy = false;

  constructor(
    private readonly engine: EngineHost,
    private readonly onResult: (r: SlideEditResult) => void,
    private readonly onError: (e: unknown) => void,
  ) {}

  private enqueue(f: () => Promise<SlideEditResult>): Promise<SlideEditResult | null> {
    const p = this.queue.then(async () => {
      try {
        const r = await f();
        this.onResult(r);
        return r;
      } catch (e) {
        this.onError(e);
        return null;
      }
    });
    this.queue = p;
    return p;
  }

  /** Run a command after the previous ones. */
  run(cmd: SlideCommand): Promise<SlideEditResult | null> {
    return this.enqueue(() => this.engine.slideCommand(cmd));
  }

  /** Insert a picture on the current slide. */
  insertPicture(bytes: Uint8Array, name: string): Promise<SlideEditResult | null> {
    return this.enqueue(() => this.engine.slideInsertPicture(bytes, name));
  }

  /** Preview a transform (coalesced while the engine is busy). */
  preview(transform: SlideTransform, snap: boolean, tolerance: number): void {
    this.pending = { cmd: 'transform', transform, snap, tolerance, preview: true };
    if (this.busy) return;
    this.busy = true;
    const next = async () => {
      const c = this.pending;
      this.pending = null;
      if (c) await this.run(c);
      if (this.pending) await next();
      else this.busy = false;
    };
    void next();
  }

  /** Drop a queued preview (the drag ended). */
  cancelPreview(): void {
    this.pending = null;
  }

  /** Wait for queued commands. */
  idle(): Promise<unknown> {
    return this.queue;
  }

  private clipMark = '';

  /** New marker for a copy written to the system clipboard. */
  newClipMark(): string {
    this.clipMark = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    return this.clipMark;
  }

  /** Was this marker written by the last copy of this editor? */
  isOwnClip(mark: string | undefined): boolean {
    return !!mark && mark === this.clipMark;
  }
}
