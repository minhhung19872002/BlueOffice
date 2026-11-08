import type { ImeEvent, PagePointer } from '@blueoffice/canvas-host';
import type {
  ClipPayload,
  EditCommand,
  EditResult,
  EngineHost,
  ClipboardAccess,
} from '@blueoffice/platform';
import { keyToCommand } from './keys';

/** What the controller needs from the view (DocumentView implements it). */
export interface EditView {
  refresh(info: EditResult['info'], dirty: number[]): void;
  setSelection(
    caret: EditResult['caret'],
    rects: EditResult['rects'],
    scrollIntoView?: boolean,
  ): void;
}

/**
 * Sends editing commands to the engine one at a time (the engine is the
 * single source of truth), applies each result to the view and reports it.
 * Mouse drags are coalesced so only the latest pointer position is sent.
 */
export class DocController {
  private queue: Promise<unknown> = Promise.resolve();
  private dragPending: EditCommand | null = null;
  private dragBusy = false;
  /** Last clip copied from this editor (internal format for lossless paste). */
  lastClip: ClipPayload | null = null;

  constructor(
    private readonly engine: EngineHost,
    private readonly view: EditView,
    private readonly clipboard: ClipboardAccess,
    private readonly onResult: (r: EditResult) => void,
    private readonly onError: (e: unknown) => void,
  ) {}

  private apply(r: EditResult): EditResult {
    if (r.dirty.length || r.info) this.view.refresh(r.info, r.dirty);
    this.view.setSelection(r.caret, r.rects);
    this.onResult(r);
    return r;
  }

  /** Show a result produced elsewhere (a collaborator's edit): no scrolling to the caret. */
  external(r: EditResult): void {
    if (r.dirty.length || r.info) this.view.refresh(r.info, r.dirty);
    this.view.setSelection(r.caret, r.rects, false);
    this.onResult(r);
  }

  /** Run a command after the previous ones (stamped with the host clock for revisions). */
  run(cmd: EditCommand): Promise<EditResult | null> {
    const p = this.queue.then(async () => {
      try {
        return this.apply(await this.engine.command({ ...cmd, now: Date.now() }));
      } catch (e) {
        this.onError(e);
        return null;
      }
    });
    this.queue = p;
    return p;
  }

  /** Run a read-only command (spelling marks) after the previous ones without applying its result:
   * the caret, selection and open header/note editor are left as the editing commands set them. */
  query(cmd: EditCommand): Promise<EditResult | null> {
    const p = this.queue.then(async () => {
      try {
        return await this.engine.command({ ...cmd, now: Date.now() });
      } catch (e) {
        this.onError(e);
        return null;
      }
    });
    this.queue = p;
    return p;
  }

  /** Insert a picture through the engine. */
  insertImage(bytes: Uint8Array, name: string): Promise<EditResult | null> {
    const p = this.queue.then(async () => {
      try {
        return this.apply(await this.engine.insertImage(bytes, name));
      } catch (e) {
        this.onError(e);
        return null;
      }
    });
    this.queue = p;
    return p;
  }

  private drag(cmd: EditCommand): void {
    this.dragPending = cmd;
    if (this.dragBusy) return;
    this.dragBusy = true;
    const next = async () => {
      const c = this.dragPending;
      this.dragPending = null;
      if (c) await this.run(c);
      if (this.dragPending) await next();
      else this.dragBusy = false;
    };
    void next();
  }

  pointer(e: PagePointer): void {
    if (e.type === 'down') {
      void this.run({
        cmd: 'click',
        page: e.page,
        x: e.x,
        y: e.y,
        extend: e.shift,
        count: e.count,
      });
    } else if (e.type === 'drag') {
      this.drag({ cmd: 'dragTo', page: e.page, x: e.x, y: e.y });
    }
  }

  input(e: ImeEvent): void {
    switch (e.type) {
      case 'insert':
        void this.run({ cmd: 'insertText', text: e.text });
        break;
      case 'key': {
        const c = keyToCommand(e);
        if (c) void this.run(c);
        break;
      }
      case 'paste':
        void this.paste(e.text, e.html, e.internal);
        break;
      case 'composition':
        break;
    }
  }

  /** Paste, preferring the lossless internal format of our own copies. */
  paste(text: string, html: string, internal = ''): Promise<EditResult | null> {
    const own =
      !internal && this.lastClip && this.lastClip.text === text ? this.lastClip.internal : '';
    return this.run({ cmd: 'paste', internal: internal || own, html, text });
  }

  async copy(cut = false): Promise<void> {
    const r = await this.run({ cmd: cut ? 'cut' : 'copy' });
    if (!r?.clip || !r.clip.text) return;
    this.lastClip = r.clip;
    try {
      await this.clipboard.writeHtml(r.clip.html, r.clip.text);
    } catch {
      await this.clipboard.writeText(r.clip.text).catch(() => {});
    }
  }

  /** Paste from the system clipboard (ribbon button). */
  async pasteFromClipboard(): Promise<void> {
    const text = await this.clipboard.readText().catch(() => '');
    await this.paste(text, '');
  }

  /** Wait for queued commands. */
  idle(): Promise<unknown> {
    return this.queue;
  }
}
