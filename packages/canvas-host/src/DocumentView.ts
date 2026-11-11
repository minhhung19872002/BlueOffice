import type {
  CaretRect,
  DocInfo,
  MarkKind,
  PageMarkup,
  SpellMark,
  EngineHost,
  MathState,
  PageDisplayList,
  SelRect,
} from '@blueoffice/platform';
import { ImeInput, type ImeEvent } from './ime';
import { GlyphStore, ImageStore, PX_PER_PT, renderPage } from './renderer';
import { drawMarkup, type MarkupSelect } from './markup';
import { drawFormattingMarks } from './marks';
import { DocMirror, type MirrorLabels } from './a11y';

/**
 * How pages are shown: printed pages with gaps and shadows; web layout (the
 * engine lays text out to the window width, strips stacked without gaps);
 * draft (no page decorations, page boundaries as dashed lines).
 */
export type PageMode = 'print' | 'web' | 'draft';

/** Pointer input on a page, in points relative to the page's top-left corner. */
export interface PagePointer {
  type: 'down' | 'drag' | 'up';
  page: number;
  x: number;
  y: number;
  shift: boolean;
  /** Click count (1 caret, 2 word, 3 paragraph). */
  count: number;
  /** Ctrl (Cmd) held: a click selects the sentence, or follows a hyperlink. */
  ctrl?: boolean;
  /** Viewport point of the pointer (mouse-up: where the mini toolbar shows). */
  clientX?: number;
  clientY?: number;
}

export interface DocumentViewOptions {
  /** Gap between pages in CSS pixels. */
  pageGap?: number;
  /** Extra pages rendered above/below the viewport. */
  overscan?: number;
  onPageChange?: (page: number) => void;
  onZoomChange?: (zoom: number) => void;
  onInput?: (e: ImeEvent) => void;
  onPointer?: (e: PagePointer) => void;
  /** Right click on a page (points relative to the page). */
  onContextMenu?: (e: {
    page: number;
    x: number;
    y: number;
    clientX: number;
    clientY: number;
    /** Element under the pointer (a review balloon, the page). */
    target: EventTarget | null;
  }) => void;
  /** The pages moved on screen (scroll, zoom, resize, relayout): rulers follow them. */
  onViewport?: () => void;
  /** A review balloon was clicked (BlueDoc markup area). */
  onMarkupSelect?: (m: MarkupSelect) => void;
  /** Localised name of a change kind shown in its balloon. */
  markupLabel?: (kind: MarkKind, id: number) => string;
  /** Accessible name of the text input that receives typing (default "Document"). */
  inputLabel?: string;
  /** Accessible name of page `n` (1-based; default "page n"). */
  pageLabel?: (n: number) => string;
  /** Texts of the screen-reader mirror of the pages (default English). */
  mirrorLabels?: MirrorLabels;
}

export const MIN_ZOOM = 0.25;
export const MAX_ZOOM = 5;

interface PageSlot {
  el: HTMLDivElement;
  overlay: HTMLDivElement;
  /** Review markup (change bars, markup area, balloons); kept across selections. */
  markup: HTMLDivElement;
  /** Width of the page's markup area (points). */
  markupWidth: number;
  canvas: HTMLCanvasElement | null;
  renderedAt: string | null;
}

/**
 * Virtualized, zoomable view of a paginated document. Only pages near the
 * viewport get a canvas; display lists are fetched from the engine on demand.
 */
/** A collaborator's caret / selection to draw over the pages. */
export interface PeerMark {
  caret: CaretRect | null;
  rects: SelRect[];
  /** The paragraph the collaborator's caret is in (bar in the left margin). */
  para?: CaretRect | null;
  color: string;
  /** Name shown next to the caret. */
  name: string;
  /** Accessible description (localised by the editor). */
  label: string;
  /** Accessible description of the margin bar (default: `label`). */
  paraLabel?: string;
}

export class DocumentView {
  readonly scroller: HTMLDivElement;
  private readonly content: HTMLDivElement;
  private readonly glyphs: GlyphStore;
  private readonly images: ImageStore;
  private dragging = false;
  private caret: CaretRect | null = null;
  /** The caret element, reused by every `setSelection`. */
  private caretEl: HTMLDivElement | null = null;
  /** The equation box while editing in place, reused by every redraw. */
  private mathZoneEl: HTMLDivElement | null = null;
  /** Interim dictation text shown at the caret (not in the document; CORE-LANG-002). */
  private dictationPreview: string | null = null;
  private peers: PeerMark[] = [];
  /** Misspelt words (red wavy underline, never printed). */
  private spelling: SpellMark[] = [];
  /** Find "highlight all" boxes (never printed). */
  private findMarks: SelRect[] = [];
  /** Editable regions of a protected document the user may edit (CORE-SEC-001). */
  private regionMarks: SelRect[] = [];
  /** The equation being edited in place: its box and empty slots (CORE-MATH-010). */
  private mathMarks: MathState | null = null;
  /** Read aloud: the paragraph and the word being spoken (CORE-LANG-003). */
  private speechMarks: { word: SelRect[]; para: SelRect[] } = { word: [], para: [] };
  /** The content control around the caret: its extent and title (never printed). */
  private controlFrame: {
    rects: SelRect[];
    title: string | null;
    appearance: string | null;
    color: string | null;
  } = { rects: [], title: null, appearance: null, color: null };
  private readonly ime: ImeInput;
  /** Screen-reader mirror of the text of the pages in view (WCAG 1.3.1 / 4.1.2). */
  readonly a11y: DocMirror;
  private slots: PageSlot[] = [];
  private lists = new Map<number, Promise<PageDisplayList>>();
  private info: DocInfo | null = null;
  private zoomValue = 1;
  private frame = 0;
  private currentPage = -1;
  private mode: PageMode = 'print';
  private readonly opts: Required<
    Omit<
      DocumentViewOptions,
      | 'onPageChange'
      | 'onZoomChange'
      | 'onInput'
      | 'onPointer'
      | 'onViewport'
      | 'onContextMenu'
      | 'onMarkupSelect'
      | 'markupLabel'
      | 'inputLabel'
      | 'pageLabel'
      | 'mirrorLabels'
    >
  > &
    DocumentViewOptions;

  constructor(
    private readonly host: HTMLElement,
    private readonly engine: EngineHost,
    opts: DocumentViewOptions = {},
  ) {
    this.opts = { pageGap: 16, overscan: 1, ...opts };
    this.glyphs = new GlyphStore(engine);
    this.images = new ImageStore(engine);
    this.scroller = document.createElement('div');
    this.scroller.className = 'bo-doc-scroller';
    this.scroller.tabIndex = 0;
    this.scroller.setAttribute('role', 'document');
    this.scroller.style.cssText =
      'position:relative;overflow:auto;width:100%;height:100%;outline:none;';
    this.content = document.createElement('div');
    this.content.style.cssText = `position:relative;display:flex;flex-direction:column;align-items:center;gap:${this.opts.pageGap}px;padding:${this.opts.pageGap}px 0;min-width:min-content;`;
    this.scroller.appendChild(this.content);
    host.appendChild(this.scroller);
    this.ime = new ImeInput(this.scroller, (e) => this.opts.onInput?.(e), opts.inputLabel);
    this.a11y = new DocMirror(this.scroller, engine, opts.mirrorLabels, this.ime.element);
    this.scroller.addEventListener('scroll', this.schedule);
    this.scroller.addEventListener('wheel', this.onWheel, { passive: false });
    this.scroller.addEventListener('mousedown', this.onMouseDown);
    this.scroller.addEventListener('contextmenu', this.onContextMenu);
    window.addEventListener('mousemove', this.onMouseMove);
    window.addEventListener('mouseup', this.onMouseUp);
    window.addEventListener('resize', this.schedule);
    // Caret: one pixel wide, blinking 530 ms on / 530 ms off (the system default word processors
    // use). Selection: a neutral grey that darkens the text under it like a multiply blend
    // (black stays black, white paper becomes light grey), as word processors show it.
    if (!document.getElementById('bo-caret-style')) {
      const st = document.createElement('style');
      st.id = 'bo-caret-style';
      st.textContent =
        '@keyframes bo-blink{0%,49%{opacity:1}50%,100%{opacity:0}}.bo-caret{position:absolute;width:1px;background:#000;animation:bo-blink 1.06s step-end infinite;pointer-events:none}.bo-sel{position:absolute;background:rgba(0,0,0,.2);pointer-events:none}.bo-find{position:absolute;background:rgba(255,200,0,.45);pointer-events:none}.bo-cc-frame{position:absolute;box-sizing:border-box;border:1px solid rgba(80,120,190,.75);pointer-events:none}.bo-cc-tag{position:absolute;box-sizing:border-box;padding:0 4px;border:1px solid rgba(80,120,190,.75);border-radius:3px;background:#eef3fb;color:#1f3d6e;font:10px system-ui,sans-serif;white-space:nowrap;pointer-events:none}.bo-cc-title{position:absolute;left:-1px;bottom:100%;padding:0 6px;border:1px solid rgba(80,120,190,.75);border-bottom:none;border-radius:3px 3px 0 0;background:#eef3fb;color:#1f3d6e;font:11px/16px system-ui,sans-serif;white-space:nowrap}.bo-speech-para{position:absolute;background:rgba(51,120,220,.08);pointer-events:none}.bo-speech-word{position:absolute;background:rgba(51,120,220,.3);box-shadow:inset 0 -2px 0 rgba(30,90,190,.85);pointer-events:none}.bo-region{position:absolute;background:rgba(255,236,150,.4);box-shadow:inset 0 -1px 0 rgba(170,130,0,.7);pointer-events:none}.bo-math-zone{position:absolute;background:rgba(0,0,0,.05);pointer-events:none}.bo-math-slot{position:absolute;box-sizing:border-box;border:1px dotted #777;pointer-events:none}.bo-peer-sel{position:absolute;opacity:.18;pointer-events:none}.bo-peer-caret{position:absolute;width:2px;pointer-events:none}.bo-peer-name{position:absolute;left:0;bottom:100%;padding:0 4px;border-radius:3px 3px 3px 0;color:#fff;font:600 11px/16px system-ui,sans-serif;white-space:nowrap}.bo-peer-bar{position:absolute;width:3px;border-radius:2px;pointer-events:none}.bo-peer-bar-name{position:absolute;right:6px;top:0;padding:0 4px;border-radius:3px;color:#fff;font:600 10px/14px system-ui,sans-serif;white-space:nowrap;opacity:.9}.bo-spell{position:absolute;height:4px;pointer-events:none;background:url("data:image/svg+xml,%3Csvg xmlns=%27http://www.w3.org/2000/svg%27 width=%274%27 height=%274%27%3E%3Cpath d=%27M0 3 L1 1 L2 1 L3 3 L4 3%27 fill=%27none%27 stroke=%27%23d32f2f%27 stroke-width=%271%27/%3E%3C/svg%3E") repeat-x}.bo-spell.bo-grammar{background-image:url("data:image/svg+xml,%3Csvg xmlns=%27http://www.w3.org/2000/svg%27 width=%274%27 height=%274%27%3E%3Cpath d=%27M0 3 L1 1 L2 1 L3 3 L4 3%27 fill=%27none%27 stroke=%27%231a5fd0%27 stroke-width=%271%27/%3E%3C/svg%3E")}';
      document.head.appendChild(st);
    }
  }

  get zoom(): number {
    return this.zoomValue;
  }

  /** Show a newly opened document. */
  setDocument(info: DocInfo): void {
    this.images.clear();
    this.buildSlots(info);
    this.scroller.scrollTop = 0;
    this.a11y.invalidate();
    this.schedule();
  }

  /** The caret moved (`edited`: by an edit): the screen-reader mirror follows it. */
  setAccessibleCaret(focus: { p: number; o: number }, edited: boolean): void {
    this.a11y.select(focus, edited);
  }

  private makeSlot(i: number): PageSlot {
    const el = document.createElement('div');
    el.className = 'bo-page';
    el.dataset.page = String(i + 1);
    // A labelled group (an aria-label needs a role): the page's pictures and marks inside.
    el.setAttribute('role', 'group');
    el.setAttribute('aria-label', this.opts.pageLabel?.(i + 1) ?? `page ${i + 1}`);
    el.style.cssText =
      'position:relative;background:#fff;box-shadow:0 1px 3px rgba(0,0,0,.25),0 0 0 1px rgba(0,0,0,.06);flex:none;cursor:text;';
    this.stylePage(el, i);
    const overlay = document.createElement('div');
    overlay.className = 'bo-overlay';
    overlay.style.cssText = 'position:absolute;inset:0;z-index:2;pointer-events:none;';
    el.appendChild(overlay);
    const markup = document.createElement('div');
    markup.className = 'bo-markup';
    markup.style.cssText = 'position:absolute;left:0;top:0;z-index:3;pointer-events:none;';
    el.appendChild(markup);
    this.content.appendChild(el);
    return { el, overlay, markup, markupWidth: 0, canvas: null, renderedAt: null };
  }

  private buildSlots(info: DocInfo): void {
    this.info = info;
    this.lists.clear();
    this.content.replaceChildren();
    this.slots = info.pages.map((_, i) => this.makeSlot(i));
    this.currentPage = -1;
    this.layoutSlots();
  }

  /**
   * Repaint after an edit: only the `dirty` pages (whose content changed, as
   * reported by the engine's incremental layout) are fetched and drawn again.
   * A changed page count adds or removes slots at the end and keeps the others
   * with their display lists and canvases (scroll position is kept).
   */
  refresh(info: DocInfo, dirty: number[]): void {
    const old = this.info;
    if (!old) {
      this.buildSlots(info);
    } else {
      const top = this.scroller.scrollTop;
      this.info = info;
      while (this.slots.length > info.pages.length) {
        this.slots.pop()?.el.remove();
        this.lists.delete(this.slots.length);
      }
      while (this.slots.length < info.pages.length) {
        this.slots.push(this.makeSlot(this.slots.length));
      }
      info.pages.forEach((p, i) => {
        const o = old.pages[i];
        if (o && (o.width !== p.width || o.height !== p.height)) this.invalidate(i);
      });
      if (old.pages.length !== info.pages.length) this.currentPage = -1;
      this.layoutSlots();
      this.scroller.scrollTop = top;
    }
    for (const i of dirty) this.invalidate(i);
    // Typing latency: ask for the changed pages in view at once (before the next frame and
    // before the screen-reader mirror's query), so they are ready when the frame paints.
    if (dirty.length && this.slots.length) {
      const [first, last] = this.visibleRange();
      for (const i of dirty) if (i >= first && i <= last) void this.pageList(i).catch(() => {});
    }
    if (dirty.length) this.a11y.invalidate();
    this.schedule();
  }

  /** Forget page `i`'s display list and drawing. */
  private invalidate(i: number): void {
    this.lists.delete(i);
    const s = this.slots[i];
    if (s) s.renderedAt = null;
  }

  /** Show the caret (collapsed selection) or the selection rectangles. */
  setSelection(caret: CaretRect | null, rects: SelRect[], scrollIntoView = true): void {
    this.caret = caret;
    this.lastRects = rects;
    for (const s of this.slots) s.overlay.replaceChildren();
    const k = PX_PER_PT * this.zoomValue;
    this.drawSpeechMarks(k);
    this.drawFindMarks(k);
    this.drawRegionMarks(k);
    this.drawMathMarks(k);
    this.drawControlFrame(k);
    for (const r of rects) {
      const slot = this.slots[r.page];
      if (!slot) continue;
      const d = document.createElement('div');
      d.className = 'bo-sel';
      d.style.cssText = `left:${r.x * k}px;top:${r.y * k}px;width:${r.w * k}px;height:${r.h * k}px;`;
      slot.overlay.appendChild(d);
    }
    if (caret && rects.length === 0) {
      const slot = this.slots[caret.page];
      if (slot) {
        // One caret element, moved: what reads it (tests, assistive tools) never holds a
        // node that a later selection update has thrown away.
        const c = (this.caretEl ??= document.createElement('div'));
        c.className = 'bo-caret';
        c.dataset.testid = 'caret';
        c.style.left = `${caret.x * k}px`;
        c.style.top = `${caret.y * k}px`;
        c.style.height = `${caret.h * k}px`;
        slot.overlay.appendChild(c);
        // The caret shows at once where it moved to (the blink starts over).
        for (const a of c.getAnimations?.() ?? []) a.currentTime = 0;
      }
    }
    this.drawPeers(k);
    this.drawSpelling(k);
    this.drawDictationPreview(k);
    if (caret) {
      const slot = this.slots[caret.page];
      if (slot) {
        const x = this.content.offsetLeft + slot.el.offsetLeft + caret.x * k;
        const y = this.content.offsetTop + slot.el.offsetTop + caret.y * k;
        this.ime.moveTo(x, y, caret.h * k);
        if (scrollIntoView) {
          const h = caret.h * k;
          const st = this.scroller.scrollTop;
          const vh = this.scroller.clientHeight;
          if (y < st) this.scroller.scrollTop = y - 24;
          else if (y + h > st + vh) this.scroller.scrollTop = y + h - vh + 24;
        }
      }
    }
  }

  /**
   * Dictation: the interim text, underlined, at the caret — only drawn, not in the document
   * (`null` hides it).
   */
  setDictationPreview(text: string | null): void {
    if (text === this.dictationPreview) return;
    this.dictationPreview = text || null;
    this.setSelection(this.caret, this.lastRects, false);
  }

  private drawDictationPreview(k: number): void {
    const caret = this.caret;
    const text = this.dictationPreview;
    if (!caret || !text) return;
    const slot = this.slots[caret.page];
    if (!slot) return;
    const d = document.createElement('div');
    d.className = 'bo-dictation-preview';
    d.dataset.testid = 'dictation-preview';
    d.textContent = text;
    const h = caret.h * k;
    d.style.cssText =
      `position:absolute;left:${caret.x * k}px;top:${caret.y * k}px;height:${h}px;` +
      `line-height:${h}px;font-size:${Math.max(8, h * 0.72)}px;white-space:pre;` +
      'pointer-events:none;color:#555;background:rgba(255,255,255,0.85);' +
      'text-decoration:underline dotted;text-underline-offset:2px;padding:0 1px;';
    slot.overlay.appendChild(d);
  }

  /**
   * The frame of the content control around the caret, with its title in a tab above it
   * (ECMA-376 Part 1, §17.5.2.1 alias); `[]` hides it.
   */
  setControlFrame(
    rects: SelRect[],
    title: string | null,
    look: { appearance?: string | null; color?: string | null } = {},
  ): void {
    const appearance = look.appearance ?? null;
    const color = look.color ?? null;
    const same =
      title === this.controlFrame.title &&
      appearance === this.controlFrame.appearance &&
      color === this.controlFrame.color &&
      rects.length === this.controlFrame.rects.length &&
      rects.every((r, i) => {
        const o = this.controlFrame.rects[i];
        return !!o && o.page === r.page && o.x === r.x && o.y === r.y && o.w === r.w && o.h === r.h;
      });
    if (same) return;
    this.controlFrame = { rects, title, appearance, color };
    this.setSelection(this.caret, this.lastRects, false);
  }

  /**
   * [MS-DOCX] w15:appearance: `hidden` draws nothing, `tags` a start and an end tag (with the
   * title) instead of the box; w15:color colours the frame.
   */
  private drawControlFrame(k: number): void {
    const { appearance, color } = this.controlFrame;
    if (appearance === 'hidden') return;
    const hex = color && /^[0-9A-Fa-f]{6}$/.test(color) ? `#${color}` : null;
    if (appearance === 'tags') {
      const rects = this.controlFrame.rects;
      const first = rects[0];
      const last = rects[rects.length - 1];
      if (!first || !last) return;
      const tag = (r: SelRect, end: boolean) => {
        const slot = this.slots[r.page];
        if (!slot) return;
        const d = document.createElement('div');
        d.className = 'bo-cc-tag';
        d.dataset.testid = end ? 'control-tag-end' : 'control-tag-start';
        d.textContent = `${end ? '/' : ''}${this.controlFrame.title ?? ''}`;
        const h = r.h * k;
        d.style.cssText =
          `top:${r.y * k}px;height:${h}px;line-height:${h}px;` +
          (end ? `left:${(r.x + r.w) * k + 1}px;` : `right:calc(100% - ${r.x * k - 1}px);`) +
          (hex ? `border-color:${hex};color:${hex};` : '');
        slot.overlay.appendChild(d);
      };
      tag(first, false);
      tag(last, true);
      return;
    }
    // One box per page around the control's rectangles.
    const boxes = new Map<number, { x0: number; y0: number; x1: number; y1: number }>();
    for (const r of this.controlFrame.rects) {
      const b = boxes.get(r.page);
      boxes.set(
        r.page,
        b
          ? {
              x0: Math.min(b.x0, r.x),
              y0: Math.min(b.y0, r.y),
              x1: Math.max(b.x1, r.x + r.w),
              y1: Math.max(b.y1, r.y + r.h),
            }
          : { x0: r.x, y0: r.y, x1: r.x + r.w, y1: r.y + r.h },
      );
    }
    let first = true;
    for (const [page, b] of boxes) {
      const slot = this.slots[page];
      if (!slot) continue;
      const d = document.createElement('div');
      d.className = 'bo-cc-frame';
      d.dataset.testid = 'control-frame';
      const pad = 2;
      d.style.cssText =
        `left:${b.x0 * k - pad}px;top:${b.y0 * k - pad}px;` +
        `width:${(b.x1 - b.x0) * k + 2 * pad}px;height:${(b.y1 - b.y0) * k + 2 * pad}px;` +
        (hex ? `border-color:${hex};` : '');
      if (hex) d.dataset.color = hex;
      if (first && this.controlFrame.title) {
        const tab = document.createElement('div');
        tab.className = 'bo-cc-title';
        tab.dataset.testid = 'control-frame-title';
        tab.textContent = this.controlFrame.title;
        if (hex) tab.style.borderColor = hex;
        d.appendChild(tab);
      }
      first = false;
      slot.overlay.appendChild(d);
    }
  }

  /** Highlight every find match (drawn under the selection); `[]` clears. */
  setFindHighlights(rects: SelRect[]): void {
    // Clearing nothing (after every edit) does not redraw the selection overlay.
    if (rects.length === 0 && this.findMarks.length === 0) return;
    this.findMarks = rects;
    this.setSelection(this.caret, this.lastRects, false);
  }

  private drawFindMarks(k: number): void {
    for (const r of this.findMarks) {
      const slot = this.slots[r.page];
      if (!slot) continue;
      const d = document.createElement('div');
      d.className = 'bo-find';
      d.dataset.testid = 'find-highlight';
      d.style.cssText = `left:${r.x * k}px;top:${r.y * k}px;width:${r.w * k}px;height:${r.h * k}px;`;
      slot.overlay.appendChild(d);
    }
  }

  /**
   * Read aloud: highlight the word being spoken (and its paragraph lightly), scrolling the word
   * into view when `follow`; empty arrays clear.
   */
  setSpeechHighlights(word: SelRect[], para: SelRect[], follow = true): void {
    this.speechMarks = { word, para };
    this.setSelection(this.caret, this.lastRects, false);
    const r = word[0];
    const slot = r && this.slots[r.page];
    if (!follow || !r || !slot) return;
    const k = PX_PER_PT * this.zoomValue;
    const y = this.content.offsetTop + slot.el.offsetTop + r.y * k;
    const h = r.h * k;
    const st = this.scroller.scrollTop;
    const vh = this.scroller.clientHeight;
    if (y < st || y + h > st + vh) this.scroller.scrollTop = Math.max(0, y - vh / 3);
  }

  private drawSpeechMarks(k: number): void {
    const draw = (rects: SelRect[], cls: string) => {
      for (const r of rects) {
        const slot = this.slots[r.page];
        if (!slot) continue;
        const d = document.createElement('div');
        d.className = cls;
        d.dataset.testid = cls;
        d.style.cssText = `left:${r.x * k}px;top:${r.y * k}px;width:${r.w * k}px;height:${r.h * k}px;`;
        slot.overlay.appendChild(d);
      }
    };
    draw(this.speechMarks.para, 'bo-speech-para');
    draw(this.speechMarks.word, 'bo-speech-word');
  }

  /** Highlight the editable regions of a protected document; `[]` clears. */
  setRegionHighlights(rects: SelRect[]): void {
    this.regionMarks = rects;
    this.setSelection(this.caret, this.lastRects, false);
  }

  private drawRegionMarks(k: number): void {
    for (const r of this.regionMarks) {
      const slot = this.slots[r.page];
      if (!slot) continue;
      const d = document.createElement('div');
      d.className = 'bo-region';
      d.dataset.testid = 'region-highlight';
      d.style.cssText = `left:${r.x * k}px;top:${r.y * k}px;width:${r.w * k}px;height:${r.h * k}px;`;
      slot.overlay.appendChild(d);
    }
  }

  /** The equation being edited in place: a light box around it and dotted empty slots; `null`
   * clears (drawn with the selection overlay, never printed). */
  setMathMarks(m: MathState | null): void {
    this.mathMarks = m;
  }

  private drawMathMarks(k: number): void {
    const m = this.mathMarks;
    if (!m) return;
    const draw = (r: SelRect, cls: string, reuse?: HTMLDivElement) => {
      const slot = this.slots[r.page];
      if (!slot) return;
      const d = reuse ?? document.createElement('div');
      d.className = cls;
      d.dataset.testid = cls;
      d.style.cssText = `left:${r.x * k}px;top:${r.y * k}px;width:${r.w * k}px;height:${r.h * k}px;`;
      slot.overlay.appendChild(d);
    };
    // The zone box is kept across redraws like the caret.
    draw(m.zone, 'bo-math-zone', (this.mathZoneEl ??= document.createElement('div')));
    for (const p of m.placeholders) draw(p, 'bo-math-slot');
  }

  /** Underline misspelt words (drawn with the selection overlay). */
  setSpelling(marks: SpellMark[]): void {
    this.spelling = marks;
    this.setSelection(this.caret, this.lastRects, false);
  }

  private drawSpelling(k: number): void {
    for (const m of this.spelling) {
      const slot = this.slots[m.page];
      if (!slot) continue;
      const d = document.createElement('div');
      d.className = m.grammar ? 'bo-spell bo-grammar' : 'bo-spell';
      d.dataset.testid = m.grammar ? 'grammar-mark' : 'spell-mark';
      d.style.cssText = `left:${m.x * k}px;top:${m.y * k}px;width:${m.w * k}px;`;
      slot.overlay.appendChild(d);
    }
  }

  /** Show collaborators' carets and selections (drawn with the own selection). */
  setPeers(peers: PeerMark[]): void {
    this.peers = peers;
    this.setSelection(this.caret, this.lastRects, false);
  }

  private drawPeers(k: number): void {
    // Margin bars: one per collaborator, side by side when they share a paragraph.
    const bars = new Map<string, number>();
    for (const p of this.peers) {
      const b = p.para;
      const slot = b ? this.slots[b.page] : undefined;
      if (!b || !slot) continue;
      const at = `${b.page}:${Math.round(b.y)}`;
      const n = bars.get(at) ?? 0;
      bars.set(at, n + 1);
      const el = document.createElement('div');
      el.className = 'bo-peer-bar';
      el.dataset.testid = 'peer-bar';
      el.setAttribute('role', 'img');
      el.setAttribute('aria-label', p.paraLabel ?? p.label);
      el.title = p.paraLabel ?? p.label;
      const left = Math.max(2, b.x * k - 12 - n * 6);
      el.style.cssText = `left:${left}px;top:${b.y * k}px;height:${Math.max(b.h * k, 4)}px;background:${p.color};`;
      if (n === 0) {
        const tag = document.createElement('span');
        tag.className = 'bo-peer-bar-name';
        tag.textContent = p.name;
        tag.style.background = p.color;
        el.appendChild(tag);
      }
      slot.overlay.appendChild(el);
    }
    for (const p of this.peers) {
      for (const r of p.rects) {
        const slot = this.slots[r.page];
        if (!slot) continue;
        const d = document.createElement('div');
        d.className = 'bo-peer-sel';
        d.style.cssText = `left:${r.x * k}px;top:${r.y * k}px;width:${r.w * k}px;height:${r.h * k}px;background:${p.color};`;
        slot.overlay.appendChild(d);
      }
      const c = p.caret;
      const slot = c ? this.slots[c.page] : undefined;
      if (!c || !slot) continue;
      const el = document.createElement('div');
      el.className = 'bo-peer-caret';
      el.dataset.testid = 'peer-caret';
      el.setAttribute('role', 'img');
      el.setAttribute('aria-label', p.label);
      el.title = p.label;
      el.style.cssText = `left:${c.x * k}px;top:${c.y * k}px;height:${c.h * k}px;background:${p.color};`;
      const tag = document.createElement('span');
      tag.className = 'bo-peer-name';
      tag.textContent = p.name;
      tag.style.background = p.color;
      el.appendChild(tag);
      slot.overlay.appendChild(el);
    }
  }

  private pointAt(e: MouseEvent): { page: number; x: number; y: number } | null {
    const k = PX_PER_PT * this.zoomValue;
    let best: { page: number; x: number; y: number; d: number } | null = null;
    this.slots.forEach((s, i) => {
      const r = s.el.getBoundingClientRect();
      const d =
        e.clientY < r.top ? r.top - e.clientY : e.clientY > r.bottom ? e.clientY - r.bottom : 0;
      if (!best || d < best.d)
        best = { page: i, x: (e.clientX - r.left) / k, y: (e.clientY - r.top) / k, d };
    });
    if (!best) return null;
    const { page, x, y } = best;
    return { page, x, y };
  }

  private onContextMenu = (e: MouseEvent) => {
    if (!this.opts.onContextMenu) return;
    const p = this.pointAt(e);
    if (!p || !this.info) return;
    e.preventDefault();
    this.opts.onContextMenu({ ...p, clientX: e.clientX, clientY: e.clientY, target: e.target });
  };

  private onMouseDown = (e: MouseEvent) => {
    if (e.button !== 0) return;
    const p = this.pointAt(e);
    if (p && this.info) {
      e.preventDefault();
      this.dragging = true;
      this.opts.onPointer?.({
        type: 'down',
        ...p,
        shift: e.shiftKey,
        count: Math.min(3, e.detail || 1),
        ctrl: e.ctrlKey || e.metaKey,
      });
    }
    this.ime.focus();
  };

  private onMouseMove = (e: MouseEvent) => {
    if (!this.dragging || (e.buttons & 1) === 0) return;
    const p = this.pointAt(e);
    if (p) this.opts.onPointer?.({ type: 'drag', ...p, shift: true, count: 1 });
  };

  private onMouseUp = (e: MouseEvent) => {
    if (!this.dragging) return;
    this.dragging = false;
    const p = this.pointAt(e);
    if (p)
      this.opts.onPointer?.({
        type: 'up',
        ...p,
        shift: e.shiftKey,
        count: 1,
        clientX: e.clientX,
        clientY: e.clientY,
      });
  };

  /** Show pages as printed, as web layout strips or as draft (see {@link PageMode}). */
  setPageMode(mode: PageMode): void {
    if (mode === this.mode) return;
    this.mode = mode;
    this.content.dataset.mode = mode;
    const gap = mode === 'print' ? this.opts.pageGap : 0;
    this.content.style.gap = `${gap}px`;
    this.content.style.padding = mode === 'print' ? `${gap}px 0` : '0';
    this.slots.forEach((s, i) => this.stylePage(s.el, i));
    this.schedule();
  }

  get pageMode(): PageMode {
    return this.mode;
  }

  private stylePage(el: HTMLElement, i: number): void {
    const print = this.mode === 'print';
    el.style.boxShadow = print ? '0 1px 3px rgba(0,0,0,.25),0 0 0 1px rgba(0,0,0,.06)' : 'none';
    // Draft: a dashed line where a new page starts.
    el.style.borderTop = this.mode === 'draft' && i > 0 ? '1px dashed #9aa3b2' : '';
    el.style.boxSizing = 'content-box';
  }

  /** Width available to a web layout page, in points at the current zoom. */
  contentWidthPt(): number {
    // Room for a vertical scroll bar that the reflowed text may bring.
    return Math.max(1, this.scroller.clientWidth - 20) / (PX_PER_PT * this.zoomValue);
  }

  /** Number of pages shown. */
  get pageCount(): number {
    return this.slots.length;
  }

  /** Draw page `index` into `canvas`, `width` CSS pixels wide (page thumbnails). */
  async renderThumbnail(index: number, canvas: HTMLCanvasElement, width: number): Promise<void> {
    const list = await this.pageList(index);
    await Promise.all([this.glyphs.ensure(list), this.images.ensure(list)]);
    const dpr = window.devicePixelRatio || 1;
    const scale = (width / list.width) * dpr;
    canvas.width = Math.max(1, Math.round(list.width * scale));
    canvas.height = Math.max(1, Math.round(list.height * scale));
    canvas.style.width = `${width}px`;
    canvas.style.height = `${Math.round((width * list.height) / list.width)}px`;
    const ctx = canvas.getContext('2d');
    if (ctx) renderPage(ctx, list, this.glyphs, scale, this.images);
  }

  setZoom(z: number): void {
    const next = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, Math.round(z * 100) / 100));
    if (next === this.zoomValue) return;
    // Keep the point at the viewport centre stable.
    const cy =
      (this.scroller.scrollTop + this.scroller.clientHeight / 2) /
      Math.max(1, this.content.scrollHeight);
    this.zoomValue = next;
    this.layoutSlots();
    this.scroller.scrollTop = cy * this.content.scrollHeight - this.scroller.clientHeight / 2;
    this.redrawSelection();
    this.opts.onZoomChange?.(next);
    this.schedule();
  }

  /** Fit page width (with its markup area) into the viewport. */
  fitWidth(): void {
    const page = this.info?.pages[0]?.width;
    if (!page) return;
    const w = page + (this.slots[0]?.markupWidth ?? 0);
    this.setZoom((this.scroller.clientWidth - 2 * this.opts.pageGap - 16) / (w * PX_PER_PT));
  }

  /** Screen rectangle of a page (CSS pixels), or null when it does not exist. */
  pageRect(index: number): DOMRect | null {
    return this.slots[index]?.el.getBoundingClientRect() ?? null;
  }

  scrollToPage(index: number): void {
    this.slots[index]?.el.scrollIntoView({ block: 'start' });
  }

  focus(): void {
    this.ime.focus();
  }

  private lastRects: SelRect[] = [];

  private redrawSelection(): void {
    this.setSelection(this.caret, this.lastRects, false);
  }

  dispose(): void {
    cancelAnimationFrame(this.frame);
    window.removeEventListener('resize', this.schedule);
    window.removeEventListener('mousemove', this.onMouseMove);
    window.removeEventListener('mouseup', this.onMouseUp);
    this.ime.dispose();
    this.a11y.dispose();
    this.scroller.remove();
    // Decoded pictures of the closed document (CORE-PERF-001: no leak across documents).
    this.images.clear();
  }

  private onWheel = (e: WheelEvent) => {
    if (!e.ctrlKey) return;
    e.preventDefault();
    this.setZoom(this.zoomValue * (e.deltaY < 0 ? 1.1 : 1 / 1.1));
  };

  private layoutSlots(): void {
    if (!this.info) return;
    this.info.pages.forEach((p, i) => {
      const slot = this.slots[i];
      if (!slot) return;
      slot.el.style.width = `${Math.round(p.width * PX_PER_PT * this.zoomValue)}px`;
      slot.el.style.height = `${Math.round(p.height * PX_PER_PT * this.zoomValue)}px`;
      // The view's page grows by its markup area (the page itself keeps its size).
      slot.el.style.marginRight = `${Math.round(slot.markupWidth * PX_PER_PT * this.zoomValue)}px`;
    });
  }

  private schedule = () => {
    cancelAnimationFrame(this.frame);
    this.frame = requestAnimationFrame(() => void this.update());
  };

  private visibleRange(): [number, number] {
    const top = this.scroller.scrollTop;
    const bottom = top + this.scroller.clientHeight;
    let first = -1;
    let last = -1;
    this.slots.forEach((s, i) => {
      const y = s.el.offsetTop;
      const h = s.el.offsetHeight;
      if (y + h >= top && y <= bottom) {
        if (first < 0) first = i;
        last = i;
      }
    });
    if (first < 0) return [0, Math.min(this.slots.length - 1, 0)];
    return [first, last];
  }

  /** Icons (simple markup) whose balloon is open. */
  private openMarkup = new Set<string>();

  private paintMarkup(slot: PageSlot, w: number, h: number, markup: PageMarkup | undefined): void {
    const width = markup?.width ?? 0;
    if (width !== slot.markupWidth) {
      slot.markupWidth = width;
      this.layoutSlots();
    }
    slot.el.dataset.markup = width > 0 ? 'true' : 'false';
    const draw = () =>
      drawMarkup(slot.markup, markup, {
        k: PX_PER_PT * this.zoomValue,
        pageWidth: w,
        pageHeight: h,
        label: this.opts.markupLabel,
        open: this.openMarkup,
        onSelect: this.opts.onMarkupSelect,
        onToggle: draw,
      });
    draw();
  }

  private pageList(i: number): Promise<PageDisplayList> {
    let p = this.lists.get(i);
    if (!p) {
      p = this.engine.getPage(i);
      this.lists.set(i, p);
    }
    return p;
  }

  private async update(): Promise<void> {
    if (!this.info || this.slots.length === 0) return;
    this.opts.onViewport?.();
    const [first, last] = this.visibleRange();
    this.a11y.setRange(first, last);
    const from = Math.max(0, first - this.opts.overscan);
    const to = Math.min(this.slots.length - 1, last + this.opts.overscan);
    if (first !== this.currentPage) {
      this.currentPage = first;
      this.opts.onPageChange?.(first);
    }
    // Release canvases far from the viewport (memory).
    this.slots.forEach((s, i) => {
      if ((i < from - 2 || i > to + 2) && s.canvas) {
        s.canvas.remove();
        s.canvas = null;
        s.renderedAt = null;
      }
    });
    const dpr = window.devicePixelRatio || 1;
    const key = `${this.zoomValue}@${dpr}`;
    await Promise.all(
      Array.from({ length: to - from + 1 }, (_, k) => from + k).map(async (i) => {
        const slot = this.slots[i];
        if (!slot || slot.renderedAt === key) return;
        const list = await this.pageList(i);
        await Promise.all([this.glyphs.ensure(list), this.images.ensure(list)]);
        const scale = PX_PER_PT * this.zoomValue * dpr;
        const canvas = slot.canvas ?? document.createElement('canvas');
        canvas.width = Math.round(list.width * scale);
        canvas.height = Math.round(list.height * scale);
        canvas.style.cssText = 'position:absolute;inset:0;width:100%;height:100%;';
        const ctx = canvas.getContext('2d');
        if (!ctx) return;
        renderPage(ctx, list, this.glyphs, scale, this.images);
        if (list.marks?.length) drawFormattingMarks(ctx, list.marks, scale);
        slot.el.dataset.marks = String(list.marks?.length ?? 0);
        this.paintMarkup(slot, list.width, list.height, list.markup);
        if (!slot.canvas) {
          slot.el.insertBefore(canvas, slot.overlay);
          slot.canvas = canvas;
        }
        slot.renderedAt = key;
        slot.el.dataset.rendered = 'true';
      }),
    );
  }
}
