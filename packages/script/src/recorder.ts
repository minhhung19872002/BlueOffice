/**
 * Macro recorder (CORE-SCRIPT-002): watches the engine calls the editor makes
 * (`EngineHost.command` for BlueDoc, `SheetHost` for BlueSheet,
 * `EngineHost.slideCommand` for BlueSlide — wrapped like `packages/hosted`'s
 * dirty tracking) and turns the ones that change the document into BlueScript
 * API calls, emitted as source with {@link callToSource}.
 *
 * - Selection and navigation commands are not recorded verbatim: the recorder
 *   keeps the selection the user has (`cur`) and the one a replay would have
 *   (`base`), and only when an action needs the selection does it emit one
 *   `doc.select(…)` (or a `goTo` / `select` passthrough on slides).
 * - Consecutive typing is one `doc.insertText` (Enter is `\n`, Backspace drops
 *   the last typed character); consecutive edits of one shape's text are one
 *   `setText`.
 * - Commands without an API method become `doc.command({…})` /
 *   `workbook.command({…})` / `presentation.command({…})` passthroughs.
 * - Calls made while a script runs are not recorded ({@link ScriptRecorder.pause}).
 */
import type {
  EditCommand,
  EditResult,
  EngineHost,
  Selection,
  SheetCommand,
  SheetHost,
  SheetRange,
  SlideCommand,
  SlideEditResult,
  SlideShapeInfo,
  TimedCommand,
} from '@blueoffice/platform';
import { DOC_QUERY_COMMANDS } from '@blueoffice/platform';
import { callToSource, type ApiCall } from './prelude';
import type { ScriptApp } from './sandbox';
import { utf16Offset } from './host';

/** BlueDoc commands that only read or move the selection (mirrors `packages/hosted` DOC_READ_COMMANDS). */
export const DOC_READ: ReadonlySet<string> = new Set([
  // Every pure query (one shared list, see `DOC_QUERY_COMMANDS`).
  ...DOC_QUERY_COMMANDS,
  'state',
  'selectAll',
  'setSelection',
  'click',
  'dragTo',
  'move',
  'copy',
  'find',
  'selectMatch',
  'fontList',
  'notes',
  'review',
  'setRevisionView',
  'setMarkup',
  'nextRevision',
  'nextComment',
  'selectComment',
  'crossReferenceTargets',
  'closeHeaderFooter',
  'cancelFormat',
  'security',
  'setIdentity',
  'setReadOnly',
  'unlockWrite',
  'nextEditableRegion',
  'showEditableRegions',
  'oleInfo',
  'oleLinks',
  'smartArtInfo',
  'chartInfo',
  'chartGallery',
  'speechText',
  'speechRects',
  'checkAdminFormat',
  'autoCorrect',
  'proofOptions',
  'spelling',
  'showFormattingMarks',
  'setAuthor',
  'openSetup',
  'historyLen',
  'mergeHistory',
  'docProperties',
  'outline',
]);

/** BlueSlide commands that only read or move the selection. */
export const SLIDE_READ: ReadonlySet<string> = new Set([
  'state',
  'goTo',
  'select',
  'selectRect',
  'selectNext',
  'click',
  'hitTest',
  'shapeBoxes',
  'headerFooterInfo',
  'copyFormat',
  'textSelect',
  'textHit',
  'textExit',
  'cellTextHit',
  'cellNext',
  'copy',
  'copySlides',
  'tableSelect',
  'tableHit',
  'tableExit',
  'show',
  'chartGallery',
  'smartArtPreviews',
  'smartArtNodeAt',
  'security',
  'setIdentity',
  'setReadOnly',
  'unlockWrite',
  'compareDeck',
  'historyLen',
  'mergeHistory',
  'docProperties',
]);

/** BlueSheet `exec` commands that only read. */
export const SHEET_READ: ReadonlySet<string> = new Set([
  'presets',
  'formatState',
  'region',
  'occupied',
  'dataEdge',
  'columnEntries',
  'filterValues',
  'sortColors',
  'sortLists',
  'validation',
  'charts',
  'find',
  'names',
  'resolve',
  'copy',
  'pageSetup',
  'pivotSource',
  'pivots',
  'chartGallery',
  'chartTemplateExport',
  'chartTemplateImport',
  'security',
  'setIdentity',
  'setReadOnly',
  'unlockWrite',
  'protectedRanges',
  'unlockProtectedRange',
  'compareWorkbook',
  'smartArtInfo',
  'smartArtPreviews',
  'historyLen',
  'mergeHistory',
  'setSelection',
  // Map chart boundary datasets (CORE-CHART-006): engine set-up, not document edits.
  'mapDatasets',
  'registerMapDataset',
]);

export type RecorderEvent = { type: 'start' } | { type: 'stop'; source: string };

/** `0` → `A`, `27` → `AB`. */
export function columnName(c: number): string {
  let n = c + 1;
  let s = '';
  while (n > 0) {
    const m = (n - 1) % 26;
    s = String.fromCharCode(65 + m) + s;
    n = Math.floor((n - 1) / 26);
  }
  return s;
}

/** A1 address of a 0-based range. */
export function rangeAddress(r: SheetRange): string {
  const a = `${columnName(r.c0)}${r.r0 + 1}`;
  return r.r0 === r.r1 && r.c0 === r.c1 ? a : `${a}:${columnName(r.c1)}${r.r1 + 1}`;
}

const DEFAULT_SEL: Selection = { anchor: { p: 0, o: 0 }, focus: { p: 0, o: 0 } };
const sameSel = (a: Selection, b: Selection) =>
  a.anchor.p === b.anchor.p &&
  a.anchor.o === b.anchor.o &&
  a.focus.p === b.focus.p &&
  a.focus.o === b.focus.o;
const collapsed = (s: Selection) => s.anchor.p === s.focus.p && s.anchor.o === s.focus.o;

/** Paragraph text of the document being recorded (for UTF-8 → UTF-16 offsets). */
export type ParagraphText = (p: number) => Promise<string | null>;

interface SlideSel {
  slide: number;
  ids: number[];
  text: { id: number; start: number; end: number; cell?: [number, number] } | null;
}

const SLIDE_START: SlideSel = { slide: 0, ids: [], text: null };
const slideSelOf = (r: SlideEditResult): SlideSel => ({
  slide: r.slide,
  ids: r.selection.map((s) => s.id),
  text: r.textEdit
    ? {
        id: r.textEdit.id,
        start: r.textEdit.start,
        end: r.textEdit.end,
        ...(r.table?.id === r.textEdit.id && r.table.sel
          ? { cell: [r.table.sel[0], r.table.sel[1]] as [number, number] }
          : {}),
      }
    : null,
});
const sameIds = (a: number[], b: number[]) =>
  a.length === b.length && a.every((x, i) => x === b[i]);

const withoutNow = (cmd: TimedCommand): EditCommand => {
  const rest: TimedCommand = { ...cmd };
  delete rest.now;
  return rest;
};

/** Toggle kinds of `formatText` the API's `format()` sets by value. */
const DOC_TOGGLES = new Set(['bold', 'italic', 'underline', 'strike', 'superscript', 'subscript']);
/** `StylePatch` keys `range.format()` accepts. */
const SHEET_PATCH_KEYS = new Set([
  'numFmt',
  'fontName',
  'fontSize',
  'bold',
  'italic',
  'underline',
  'strike',
  'fontColor',
  'fillColor',
  'hAlign',
  'vAlign',
  'wrap',
  'indent',
  'rotation',
  'shrink',
  'locked',
  'formulaHidden',
]);

const color = (hex: string | null | undefined) =>
  hex === null || hex === undefined ? null : `#${hex.replace(/^#/, '')}`;

export class ScriptRecorder {
  private on = false;
  private paused = 0;
  private calls: ApiCall[] = [];
  private result: string | null = null;
  private readonly listeners = new Set<(e: RecorderEvent) => void>();

  // BlueDoc
  /** Selection a replay has now. */
  private docBase: Selection = DEFAULT_SEL;
  /** Selection the user has now. */
  private docCur: Selection = DEFAULT_SEL;
  /** Text typed since the last other action (not yet emitted). */
  private typing: string | null = null;
  private docText: ParagraphText | null = null;
  private docProbe: (() => Promise<Selection | null>) | null = null;

  // BlueSlide
  private slideBase: SlideSel = SLIDE_START;
  private slideCur: SlideSel = SLIDE_START;
  /** Text of one shape being typed (not yet emitted). */
  private shapeText: { slide: number; id: number; text: string } | null = null;
  private slideProbe: (() => Promise<SlideSel | null>) | null = null;

  constructor(readonly app: ScriptApp) {}

  get recording(): boolean {
    return this.on;
  }

  subscribe(fn: (e: RecorderEvent) => void): () => void {
    this.listeners.add(fn);
    return () => void this.listeners.delete(fn);
  }

  /**
   * Starts recording. The selection at this point is compared with the one a
   * freshly opened copy has (start of the document, first slide), so a replay
   * begins where the user did.
   */
  async start(): Promise<void> {
    if (this.on) return;
    this.calls = [];
    this.result = null;
    this.typing = null;
    this.shapeText = null;
    this.docBase = DEFAULT_SEL;
    this.docCur = DEFAULT_SEL;
    this.slideBase = SLIDE_START;
    this.slideCur = SLIDE_START;
    this.on = true;
    this.emit({ type: 'start' });
    try {
      const d = await this.docProbe?.();
      if (d) this.docCur = d;
      const s = await this.slideProbe?.();
      if (s) this.slideCur = s;
    } catch {
      // no document: the defaults stay
    }
  }

  /**
   * Waits for the editor's queued commands (keys typed just before Stop); set by
   * an editor whose commands go through a queue.
   */
  private settle: (() => Promise<unknown>) | null = null;

  /** Sets the wait for the editor's command queue; returns the function that removes it. */
  setSettle(fn: () => Promise<unknown>): () => void {
    this.settle = fn;
    return () => {
      if (this.settle === fn) this.settle = null;
    };
  }

  /** Stops recording; resolves to the script source (empty when nothing was recorded). */
  async stop(): Promise<string> {
    if (!this.on) return this.result ?? '';
    try {
      await this.settle?.();
    } catch {
      // the queue failed: stop with what was recorded
    }
    if (!this.on) return this.result ?? '';
    this.flush();
    this.on = false;
    const source = this.calls.map(callToSource).join('\n');
    this.calls = [];
    this.result = source;
    this.emit({ type: 'stop', source });
    return source;
  }

  /** The source of the last recording, once (for a pane opened after Stop). */
  takeResult(): string | null {
    const r = this.result;
    this.result = null;
    return r;
  }

  /** Runs `fn` without recording what it does (a script run). */
  async pause<T>(fn: () => Promise<T>): Promise<T> {
    this.paused++;
    try {
      return await fn();
    } finally {
      this.paused--;
    }
  }

  private get active(): boolean {
    return this.on && this.paused === 0;
  }

  private emit(e: RecorderEvent) {
    this.listeners.forEach((l) => l(e));
  }

  private push(method: string, ...args: unknown[]) {
    this.calls.push({ method, args });
  }

  private flush() {
    if (this.typing) this.push('doc.insertText', this.typing);
    this.typing = null;
    if (this.shapeText) {
      const s = this.shapeText;
      this.push('slide.setText', s.slide, s.id, s.text);
      this.slideBase = { slide: s.slide, ids: [s.id], text: null };
    }
    this.shapeText = null;
  }

  // ------------------------------------------------------------------ BlueDoc

  /** Wires the queries the recorder makes itself (not recorded). */
  attachDoc(probe: () => Promise<Selection | null>, text: ParagraphText) {
    this.docProbe = probe;
    this.docText = text;
  }

  /** Before an engine command: an action that needs the selection gets a `select` first. */
  async beforeDoc(cmd: EditCommand): Promise<void> {
    if (!this.active || DOC_READ.has(cmd.cmd)) return;
    const typingOn =
      (cmd.cmd === 'insertText' || cmd.cmd === 'splitParagraph' || cmd.cmd === 'deleteBackward') &&
      this.typing !== null &&
      sameSel(this.docBase, this.docCur);
    if (typingOn) return;
    this.flush();
    await this.docSelect();
  }

  /** After an engine command: what it did, as API calls. */
  afterDoc(cmd: EditCommand, r: EditResult | null | undefined): void {
    if (!this.active || !r || r.error) return;
    if (DOC_READ.has(cmd.cmd)) {
      if (r.selection) this.docCur = r.selection;
      return;
    }
    const before = this.docCur;
    const typingOn = this.typing !== null && sameSel(this.docBase, before);
    switch (cmd.cmd) {
      case 'insertText':
        this.typing = (typingOn ? this.typing! : '') + cmd.text;
        break;
      case 'splitParagraph':
        this.typing = `${typingOn ? this.typing! : ''}\n`;
        break;
      case 'deleteBackward':
        if (typingOn && this.typing && collapsed(before)) {
          const cps = [...this.typing];
          cps.pop();
          this.typing = cps.join('');
        } else {
          this.push('doc.command', { cmd: 'deleteBackward' });
        }
        break;
      case 'formatText': {
        const f = cmd.format;
        if (DOC_TOGGLES.has(f.kind)) {
          this.push('doc.format', { [f.kind]: !!r.format[f.kind as 'bold'] });
        } else if (f.kind === 'font') this.push('doc.format', { font: f.name });
        else if (f.kind === 'size') this.push('doc.format', { size: f.halfPoints / 2 });
        else if (f.kind === 'color') this.push('doc.format', { color: color(f.hex) });
        else if (f.kind === 'highlight') this.push('doc.format', { highlight: color(f.hex) });
        else this.push('doc.command', withoutNow(cmd));
        break;
      }
      case 'applyStyle':
        if (collapsed(before)) {
          this.push('doc.setParagraphStyle', before.focus.p, cmd.id);
          // The API puts the caret at the start of the paragraph.
          this.docBase = {
            anchor: { p: before.focus.p, o: 0 },
            focus: { p: before.focus.p, o: 0 },
          };
          this.docCur = r.selection;
          return;
        }
        this.push('doc.command', withoutNow(cmd));
        break;
      case 'insertPageBreak':
        this.push('doc.insertPageBreak');
        break;
      case 'insertTable':
        this.push('doc.insertTable', cmd.rows, cmd.cols, null);
        break;
      case 'replaceAll':
        this.push('doc.replace', cmd.query, cmd.with, cmd.options ?? {});
        break;
      default:
        this.push('doc.command', withoutNow(cmd));
    }
    this.docBase = r.selection;
    this.docCur = r.selection;
  }

  /** A picture (bytes are not recorded). */
  docPicture(r: EditResult | null | undefined): void {
    if (!this.active || !r || r.error) return;
    this.flush();
    this.push('note', 'picture not recorded');
    this.docBase = r.selection;
    this.docCur = r.selection;
  }

  private async docSelect() {
    const cur = this.docCur;
    if (sameSel(this.docBase, cur)) return;
    this.docBase = cur;
    if (cur.anchor.p === cur.focus.p && this.docText) {
      const text = await this.docText(cur.anchor.p).catch(() => null);
      if (text !== null) {
        const a = utf16Offset(text, cur.anchor.o);
        const b = utf16Offset(text, cur.focus.o);
        this.push('doc.select', cur.anchor.p, a, b);
        return;
      }
    }
    this.push('doc.command', { cmd: 'setSelection', selection: cur });
  }

  // ------------------------------------------------------------------ BlueSheet

  sheetInputs(sheet: number, cells: [number, number, string][]): void {
    if (!this.active || !cells.length) return;
    let r0 = Infinity;
    let c0 = Infinity;
    let r1 = -1;
    let c1 = -1;
    const at = new Map<string, string>();
    for (const [r, c, v] of cells) {
      r0 = Math.min(r0, r);
      c0 = Math.min(c0, c);
      r1 = Math.max(r1, r);
      c1 = Math.max(c1, c);
      at.set(`${r},${c}`, v);
    }
    const full = at.size === (r1 - r0 + 1) * (c1 - c0 + 1);
    if (full) {
      const rows: string[][] = [];
      for (let r = r0; r <= r1; r++) {
        const row: string[] = [];
        for (let c = c0; c <= c1; c++) row.push(at.get(`${r},${c}`) ?? '');
        rows.push(row);
      }
      this.push('sheet.setValues', sheet, rangeAddress({ r0, c0, r1, c1 }), rows);
      return;
    }
    for (const [r, c, v] of cells) {
      this.push('sheet.setValues', sheet, rangeAddress({ r0: r, c0: c, r1: r, c1: c }), [[v]]);
    }
  }

  sheetExec(cmd: SheetCommand): void {
    if (!this.active || SHEET_READ.has(cmd.cmd)) return;
    switch (cmd.cmd) {
      case 'setInputs':
        this.sheetInputs(cmd.sheet, cmd.cells);
        return;
      case 'format':
        if (Object.keys(cmd.patch).every((k) => SHEET_PATCH_KEYS.has(k))) {
          const patch: Record<string, unknown> = { ...cmd.patch };
          for (const k of ['fontColor', 'fillColor']) {
            if (typeof patch[k] === 'string') patch[k] = color(patch[k] as string);
          }
          this.push('sheet.format', cmd.sheet, rangeAddress(cmd.range), patch);
          return;
        }
        break;
      case 'merge':
        if (!cmd.across) {
          this.push('sheet.merge', cmd.sheet, rangeAddress(cmd.range), !!cmd.center);
          return;
        }
        break;
      case 'clearFormats':
        this.push('sheet.clear', cmd.sheet, rangeAddress(cmd.range), 'formats');
        return;
      default:
        break;
    }
    this.push('sheet.command', cmd);
  }

  /** Other `SheetHost` methods (passthrough where the engine has a command, else a note). */
  sheetOther(method: string, args: unknown[]): void {
    if (!this.active) return;
    if (method === 'addSheet') this.push('sheet.add', args[0] ?? null);
    else if (method === 'renameSheet') this.push('sheet.rename', args[0], args[1]);
    else
      this.push(
        'note',
        `sheet.${method}(${args.map((a) => JSON.stringify(a)).join(', ')}) not recorded`,
      );
  }

  // ------------------------------------------------------------------ BlueSlide

  attachSlide(probe: () => Promise<SlideEditResult | null>) {
    this.slideProbe = async () => {
      const r = await probe();
      return r ? slideSelOf(r) : null;
    };
  }

  beforeSlide(cmd: SlideCommand): void {
    if (!this.active || SLIDE_READ.has(cmd.cmd)) return;
    if (cmd.cmd === 'setText' && !cmd.cell) {
      const id = cmd.id ?? this.slideCur.text?.id ?? this.slideCur.ids[0];
      const s = this.shapeText;
      if (s && s.id === id && s.slide === this.slideCur.slide) return;
    }
    this.flush();
  }

  afterSlide(cmd: SlideCommand, r: SlideEditResult | null | undefined): void {
    if (!this.active || !r || r.error) return;
    if (SLIDE_READ.has(cmd.cmd)) {
      this.slideCur = slideSelOf(r);
      return;
    }
    const cur = this.slideCur;
    const single = cur.ids.length === 1 && !cur.text ? cur.ids[0]! : null;
    const picked = (id: number) => {
      this.slideBase = { slide: cur.slide, ids: [id], text: null };
      this.slideCur = slideSelOf(r);
    };
    switch (cmd.cmd) {
      case 'setText': {
        const id = cmd.id ?? cur.text?.id ?? cur.ids[0];
        if (!cmd.cell && id !== undefined) {
          this.shapeText = { slide: cur.slide, id, text: cmd.text };
          this.slideCur = slideSelOf(r);
          return;
        }
        break;
      }
      case 'setFill':
        if (single !== null) {
          this.push('slide.setFill', cur.slide, single, color(cmd.color));
          return picked(single);
        }
        break;
      case 'setLine':
        if (single !== null) {
          this.push('slide.setLine', cur.slide, single, color(cmd.color), cmd.width ?? null);
          return picked(single);
        }
        break;
      case 'transform': {
        const moved = r.selection.find((s) => s.id === single);
        if (single !== null && cmd.transform.kind === 'move' && moved) {
          const at = topLeft(moved);
          this.push('slide.move', cur.slide, single, at.x, at.y);
          return picked(single);
        }
        break;
      }
      case 'setNotes':
        this.push('slide.setNotes', cmd.slide ?? cur.slide, cmd.text);
        this.slideCur = slideSelOf(r);
        return;
      default:
        break;
    }
    this.slideFold();
    this.push('slide.command', cmd);
    this.slideBase = slideSelOf(r);
    this.slideCur = this.slideBase;
  }

  slidePicture(r: SlideEditResult | null | undefined): void {
    if (!this.active || !r || r.error) return;
    this.flush();
    this.push('note', 'picture not recorded');
    this.slideBase = slideSelOf(r);
    this.slideCur = this.slideBase;
  }

  /** Brings a replay to the user's slide / selection / text caret. */
  private slideFold() {
    const b = this.slideBase;
    const c = this.slideCur;
    if (b.slide !== c.slide) this.push('slide.command', { cmd: 'goTo', slide: c.slide });
    if (c.text) {
      const t = this.slideBase.text;
      if (
        !t ||
        t.id !== c.text.id ||
        t.start !== c.text.start ||
        t.end !== c.text.end ||
        b.slide !== c.slide
      ) {
        this.push('slide.command', { cmd: 'textSelect', ...c.text });
      }
    } else if (!sameIds(b.ids, c.ids) || b.text || b.slide !== c.slide) {
      this.push('slide.command', { cmd: 'select', ids: c.ids });
    }
    this.slideBase = c;
  }
}

const topLeft = (s: SlideShapeInfo) => ({
  x: Math.round((s.rect.cx - s.rect.w / 2) * 100) / 100,
  y: Math.round((s.rect.cy - s.rect.h / 2) * 100) / 100,
});

/** Engine host whose document-changing calls `rec` records (BlueDoc and BlueSlide). */
export function recordEngine(engine: EngineHost, rec: ScriptRecorder): EngineHost {
  if (rec.app === 'doc') {
    rec.attachDoc(
      async () => (await engine.command({ cmd: 'state' })).selection ?? null,
      async (p) => {
        const r = await engine.command({ cmd: 'speechText', from: p, to: p });
        const para = r.speech?.paras?.find((x) => x.p === p);
        return para ? para.text : null;
      },
    );
  }
  if (rec.app === 'slide') rec.attachSlide(() => engine.slideCommand({ cmd: 'state' }));
  const overrides: Partial<EngineHost> = {
    command: async (cmd: TimedCommand): Promise<EditResult> => {
      if (rec.app === 'doc') await rec.beforeDoc(cmd);
      const r = await engine.command(cmd);
      if (rec.app === 'doc') rec.afterDoc(cmd, r);
      return r;
    },
    insertImage: async (bytes: Uint8Array, name: string): Promise<EditResult> => {
      const r = await engine.insertImage(bytes, name);
      if (rec.app === 'doc') rec.docPicture(r);
      return r;
    },
    slideCommand: async (cmd: SlideCommand): Promise<SlideEditResult> => {
      if (rec.app === 'slide') rec.beforeSlide(cmd);
      const r = await engine.slideCommand(cmd);
      if (rec.app === 'slide') rec.afterSlide(cmd, r);
      return r;
    },
    slideInsertPicture: async (bytes: Uint8Array, name: string): Promise<SlideEditResult> => {
      const r = await engine.slideInsertPicture(bytes, name);
      if (rec.app === 'slide') rec.slidePicture(r);
      return r;
    },
  };
  return overlay(engine, overrides);
}

/** Sheet host whose document-changing calls `rec` records. */
export function recordSheetHost(sheet: SheetHost, rec: ScriptRecorder): SheetHost {
  const after =
    (method: string) =>
    async (...args: unknown[]): Promise<unknown> => {
      const r: unknown = await (sheet[method as keyof SheetHost] as (...a: unknown[]) => unknown)(
        ...args,
      );
      rec.sheetOther(method, args);
      return r;
    };
  const overrides: Partial<SheetHost> = {
    setInputs: async (s: number, cells: [number, number, string][]) => {
      await sheet.setInputs(s, cells);
      rec.sheetInputs(s, cells);
    },
    exec: async <T = unknown>(cmd: SheetCommand): Promise<T> => {
      const r = await sheet.exec<T>(cmd);
      rec.sheetExec(cmd);
      return r;
    },
    structural: after('structural') as SheetHost['structural'],
    undo: after('undo') as SheetHost['undo'],
    redo: after('redo') as SheetHost['redo'],
    addSheet: after('addSheet') as SheetHost['addSheet'],
    renameSheet: after('renameSheet') as SheetHost['renameSheet'],
    deleteSheet: after('deleteSheet') as SheetHost['deleteSheet'],
  };
  return overlay(sheet, overrides);
}

/** `target` with some methods replaced (other members bound to the target). */
function overlay<H extends object>(target: H, overrides: Partial<H>): H {
  return new Proxy(target, {
    get(t, prop) {
      if (Object.prototype.hasOwnProperty.call(overrides, prop)) {
        return (overrides as Record<PropertyKey, unknown>)[prop];
      }
      const v: unknown = Reflect.get(t, prop, t);
      return typeof v === 'function' ? (v as (...a: unknown[]) => unknown).bind(t) : v;
    },
  });
}
