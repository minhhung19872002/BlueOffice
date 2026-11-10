import type {
  EngineHost,
  SlideCommand,
  SlideEditResult,
  SlideTransform,
} from '@blueoffice/platform';
import { RepeatTracker, SLIDE_REPEATABLE } from '@blueoffice/editor';

/**
 * Sends slide commands to the engine one at a time (the engine holds the
 * deck, the selection and the history) and reports every result. Drag
 * previews are coalesced: only the latest pointer position is sent.
 */
export class SlideController {
  private queue: Promise<unknown> = Promise.resolve();
  private pending: SlideCommand | null = null;
  private busy = false;
  /** Last repeatable command (F4, CORE-UI-004). */
  readonly repeats = new RepeatTracker<SlideCommand>(SLIDE_REPEATABLE);
  /** The latest result, as soon as it arrives (dictation reads it between renders). */
  last: SlideEditResult | null = null;

  constructor(
    private readonly engine: EngineHost,
    private readonly onResult: (r: SlideEditResult) => void,
    private readonly onError: (e: unknown) => void,
  ) {}

  private enqueue(f: () => Promise<SlideEditResult>): Promise<SlideEditResult | null> {
    const p = this.queue.then(async () => {
      try {
        const r = await f();
        this.last = r;
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
    this.repeats.record(cmd);
    return this.enqueue(() => this.engine.slideCommand(cmd));
  }

  /** Send the last repeatable command again (F4); null when there is none. */
  repeat(): Promise<SlideEditResult | null> {
    const c = this.repeats.get();
    return c ? this.enqueue(() => this.engine.slideCommand(c)) : Promise.resolve(null);
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
  private clipText: string | null = null;

  /** New marker for a copy (with its plain text) written to the system clipboard. */
  newClipMark(text: string): string {
    this.clipMark = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    this.clipText = text;
    return this.clipMark;
  }

  /**
   * Is the system clipboard still holding the last copy of this editor? By its
   * HTML marker, or — hosts sanitize HTML — by the same plain text.
   */
  isOwnClip(mark: string | undefined, text?: string): boolean {
    if (mark) return mark === this.clipMark;
    return this.clipText !== null && (text ?? '') === this.clipText;
  }

  private copiedShapes: { text: string; png?: string } | null = null;

  get shapesCopy(): { text: string; png?: string } | null {
    return this.copiedShapes;
  }

  rememberShapesCopy(copy: { text: string; png?: string } | null): void {
    this.copiedShapes = copy;
  }
}
