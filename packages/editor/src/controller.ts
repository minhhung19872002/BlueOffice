import type { ImeEvent, PagePointer } from '@blueoffice/canvas-host';
import type {
  ClipPayload,
  EditCommand,
  EditResult,
  EngineHost,
  ClipboardAccess,
} from '@blueoffice/platform';
import { keyToCommand } from './keys';
import { DOC_REPEATABLE, RepeatTracker } from './repeat';
import { clipboardHistory, type ClipboardHistory, type ClipItem } from './clipboard/history';
import { CopyInFlight } from './clipboard/copyInFlight';
import {
  DEFAULT_PASTE,
  defaultChoice,
  fromBase64,
  toBase64,
  type DocPasteChoice,
  type DocPasteRecord,
  type PasteDefaults,
} from './clipboard/docPaste';

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
  /** The last paste while it can still be redone in another mode (paste options). */
  lastPaste: DocPasteRecord | null = null;
  /** Default paste modes (BlueEditor loads the user's). */
  pasteDefaults: PasteDefaults = DEFAULT_PASTE;
  /** Clipboard history copies are added to. */
  history: ClipboardHistory = clipboardHistory;
  /** The copy still on its way to the system clipboard (pastes wait for it). */
  readonly copies = new CopyInFlight<ClipPayload>();
  private pasteListener: ((r: DocPasteRecord | null) => void) | null = null;
  /** Last repeatable edit (F4, CORE-UI-004). */
  readonly repeats = new RepeatTracker<EditCommand>(DOC_REPEATABLE);

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

  /** Run a command after the previous ones (stamped with the host clock for revisions).
   * Any other edit or click ends the chance to change the last paste's mode. */
  run(cmd: EditCommand): Promise<EditResult | null> {
    this.setPaste(null);
    this.repeats.record(cmd);
    return this.exec(cmd);
  }

  /** Send the last repeatable edit again at the current selection (F4); null when none. */
  repeat(): Promise<EditResult | null> {
    const c = this.repeats.get();
    if (!c) return Promise.resolve(null);
    this.setPaste(null);
    // The repeated text is not joined to what is typed next.
    this.repeats.endRun();
    return this.exec(c);
  }

  /** Default paste modes for new pastes. */
  setPasteDefaults(d: PasteDefaults): void {
    this.pasteDefaults = d;
  }

  /** Follow the last paste (paste options button). */
  onPasteChange(listener: ((r: DocPasteRecord | null) => void) | null): void {
    this.pasteListener = listener;
  }

  private setPaste(r: DocPasteRecord | null): void {
    if (r === null && this.lastPaste === null) return;
    this.lastPaste = r;
    this.pasteListener?.(r);
  }

  private exec(cmd: EditCommand): Promise<EditResult | null> {
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
    this.setPaste(null);
    return this.insertImageRaw(bytes, name);
  }

  private insertImageRaw(bytes: Uint8Array, name: string): Promise<EditResult | null> {
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
        this.pasteEvent(e.text, e.html, e.internal, e.image).catch((err: unknown) =>
          this.onError(err),
        );
        break;
      case 'composition':
        break;
    }
  }

  /** A `paste` event: its content was read when the event fired, so when a copy of this
   * editor was still on its way to the system clipboard that content is older than the
   * copy — paste the copy instead (CORE-MISC-003). */
  async pasteEvent(
    text: string,
    html: string,
    internal = '',
    image?: Blob,
  ): Promise<EditResult | null> {
    const copied = await this.copies.settle();
    if (copied.result) return this.paste(copied.result.text, copied.result.html);
    if (image && !text && !html && !internal) {
      return this.paste('', '', '', new Uint8Array(await image.arrayBuffer()));
    }
    return this.paste(text, html, internal);
  }

  /** Paste, preferring the lossless internal format of our own copies; the mode comes
   * from the user's defaults for where the content comes from. */
  paste(text: string, html: string, internal = '', image?: Uint8Array): Promise<EditResult | null> {
    const own = !internal && this.lastClip && this.lastClip.text === text ? this.lastClip : null;
    const ownInternal = internal || own?.internal || '';
    return this.pasteRecord({
      text,
      html,
      internal: ownInternal,
      png: own?.png ?? (image ? toBase64(image) : undefined),
      pictureOnly: !!image && !text && !html && !internal,
      source: ownInternal ? 'same' : html ? 'other' : 'plain',
      mode: 'keep',
    });
  }

  /** Paste an item of the clipboard history. */
  pasteItem(item: ClipItem): Promise<EditResult | null> {
    const internal = item.internal ?? '';
    return this.pasteRecord({
      text: item.text,
      html: item.html ?? '',
      internal,
      png: item.png,
      pictureOnly: !item.text && !internal && !!item.png,
      source:
        internal && internal === this.lastClip?.internal
          ? 'same'
          : internal || item.html
            ? 'other'
            : 'plain',
      mode: 'keep',
    });
  }

  private async pasteRecord(rec: DocPasteRecord): Promise<EditResult | null> {
    if (!rec.text && !rec.html && !rec.internal && !rec.png) return null;
    this.setPaste(null);
    rec.mode = defaultChoice(rec, this.pasteDefaults);
    const r = await this.pasteAs(rec, rec.mode);
    if (r && !r.error) this.setPaste(rec);
    return r;
  }

  private pasteAs(rec: DocPasteRecord, mode: DocPasteChoice): Promise<EditResult | null> {
    if (mode === 'picture') {
      return rec.png
        ? this.insertImageRaw(fromBase64(rec.png), 'clipboard.png')
        : Promise.resolve(null);
    }
    return this.exec({
      cmd: 'paste',
      internal: rec.internal,
      html: rec.html,
      text: rec.text,
      mode,
    });
  }

  /** Redo the last paste in another mode: undo it, paste again (one undo step overall). */
  async switchPaste(mode: DocPasteChoice): Promise<EditResult | null> {
    const rec = this.lastPaste;
    if (!rec || rec.mode === mode) return null;
    const undone = await this.exec({ cmd: 'undo' });
    if (!undone || undone.error) return undone;
    const r = await this.pasteAs(rec, mode);
    if (r && !r.error) {
      this.setPaste({ ...rec, mode });
      return r;
    }
    // Could not paste that way: put the paste back as it was.
    await this.pasteAs(rec, rec.mode);
    return r;
  }

  /** Copy (or cut) the selection to the clipboard history and the system clipboard; a
   * paste started before it is done waits for it (`copies`). */
  async copy(cut = false): Promise<void> {
    await this.copies.track(this.copyNow(cut));
  }

  private async copyNow(cut: boolean): Promise<ClipPayload | null> {
    const r = await this.run({ cmd: cut ? 'cut' : 'copy' });
    if (!r?.clip || !r.clip.text) return null;
    this.lastClip = r.clip;
    this.history.push({
      app: 'doc',
      kind: r.clip.internal ? 'formatted' : 'text',
      text: r.clip.text,
      html: r.clip.html || undefined,
      internal: r.clip.internal || undefined,
      png: r.clip.png,
    });
    try {
      await this.clipboard.writeHtml(r.clip.html, r.clip.text);
    } catch {
      await this.clipboard.writeText(r.clip.text).catch(() => {});
    }
    return r.clip;
  }

  /** Paste from the system clipboard (ribbon button): text, HTML and pictures when the
   * platform can read them, else plain text. A copy still in flight is pasted itself
   * (the system clipboard may not hold it yet, or may refuse it). */
  async pasteFromClipboard(): Promise<void> {
    const copied = await this.copies.settle();
    if (copied.result) {
      await this.paste(copied.result.text, copied.result.html);
      return;
    }
    const all = await Promise.resolve()
      .then(() => this.clipboard.read?.())
      .catch(() => undefined);
    if (all && (all.text || all.html || all.image)) {
      await this.paste(all.text ?? '', all.html ?? '', '', all.image);
      return;
    }
    const text = await this.clipboard.readText().catch(() => '');
    await this.paste(text, '');
  }

  /** Wait for queued commands. */
  idle(): Promise<unknown> {
    return this.queue;
  }
}
