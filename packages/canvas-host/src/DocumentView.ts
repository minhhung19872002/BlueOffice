import type {
  CaretRect,
  DocInfo,
  SpellMark,
  EngineHost,
  PageDisplayList,
  SelRect,
} from '@blueoffice/platform';
import { ImeInput, type ImeEvent } from './ime';
import { GlyphStore, ImageStore, PX_PER_PT, renderPage } from './renderer';

/** Pointer input on a page, in points relative to the page's top-left corner. */
export interface PagePointer {
  type: 'down' | 'drag' | 'up';
  page: number;
  x: number;
  y: number;
  shift: boolean;
  /** Click count (1 caret, 2 word, 3 paragraph). */
  count: number;
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
  }) => void;
  /** The pages moved on screen (scroll, zoom, resize, relayout): rulers follow them. */
  onViewport?: () => void;
}

export const MIN_ZOOM = 0.25;
export const MAX_ZOOM = 5;

interface PageSlot {
  el: HTMLDivElement;
  overlay: HTMLDivElement;
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
  color: string;
  /** Name shown next to the caret. */
  name: string;
  /** Accessible description (localised by the editor). */
  label: string;
}

export class DocumentView {
  readonly scroller: HTMLDivElement;
  private readonly content: HTMLDivElement;
  private readonly glyphs: GlyphStore;
  private readonly images: ImageStore;
  private dragging = false;
  private caret: CaretRect | null = null;
  private peers: PeerMark[] = [];
  /** Misspelt words (red wavy underline, never printed). */
  private spelling: SpellMark[] = [];
  private readonly ime: ImeInput;
  private slots: PageSlot[] = [];
  private lists = new Map<number, Promise<PageDisplayList>>();
  private info: DocInfo | null = null;
  private zoomValue = 1;
  private frame = 0;
  private currentPage = -1;
  private readonly opts: Required<
    Omit<
      DocumentViewOptions,
      'onPageChange' | 'onZoomChange' | 'onInput' | 'onPointer' | 'onViewport' | 'onContextMenu'
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
    this.ime = new ImeInput(this.scroller, (e) => this.opts.onInput?.(e));
    this.scroller.addEventListener('scroll', this.schedule);
    this.scroller.addEventListener('wheel', this.onWheel, { passive: false });
    this.scroller.addEventListener('mousedown', this.onMouseDown);
    this.scroller.addEventListener('contextmenu', this.onContextMenu);
    window.addEventListener('mousemove', this.onMouseMove);
    window.addEventListener('mouseup', this.onMouseUp);
    window.addEventListener('resize', this.schedule);
    if (!document.getElementById('bo-caret-style')) {
      const st = document.createElement('style');
      st.id = 'bo-caret-style';
      st.textContent =
        '@keyframes bo-blink{0%,49%{opacity:1}50%,100%{opacity:0}}.bo-caret{position:absolute;width:1.5px;background:#000;animation:bo-blink 1.06s step-end infinite;pointer-events:none}.bo-sel{position:absolute;background:rgba(51,120,220,.32);pointer-events:none}.bo-peer-sel{position:absolute;opacity:.18;pointer-events:none}.bo-peer-caret{position:absolute;width:2px;pointer-events:none}.bo-peer-name{position:absolute;left:0;bottom:100%;padding:0 4px;border-radius:3px 3px 3px 0;color:#fff;font:600 11px/16px system-ui,sans-serif;white-space:nowrap}.bo-spell{position:absolute;height:4px;pointer-events:none;background:url("data:image/svg+xml,%3Csvg xmlns=%27http://www.w3.org/2000/svg%27 width=%274%27 height=%274%27%3E%3Cpath d=%27M0 3 L1 1 L2 1 L3 3 L4 3%27 fill=%27none%27 stroke=%27%23d32f2f%27 stroke-width=%271%27/%3E%3C/svg%3E") repeat-x}.bo-spell.bo-grammar{background-image:url("data:image/svg+xml,%3Csvg xmlns=%27http://www.w3.org/2000/svg%27 width=%274%27 height=%274%27%3E%3Cpath d=%27M0 3 L1 1 L2 1 L3 3 L4 3%27 fill=%27none%27 stroke=%27%231a5fd0%27 stroke-width=%271%27/%3E%3C/svg%3E")}';
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
    this.schedule();
  }

  private makeSlot(i: number): PageSlot {
    const el = document.createElement('div');
    el.className = 'bo-page';
    el.dataset.page = String(i + 1);
    el.setAttribute('aria-label', `page ${i + 1}`);
    el.style.cssText =
      'position:relative;background:#fff;box-shadow:0 1px 3px rgba(0,0,0,.25),0 0 0 1px rgba(0,0,0,.06);flex:none;cursor:text;';
    const overlay = document.createElement('div');
    overlay.className = 'bo-overlay';
    overlay.style.cssText = 'position:absolute;inset:0;z-index:2;pointer-events:none;';
    el.appendChild(overlay);
    this.content.appendChild(el);
    return { el, overlay, canvas: null, renderedAt: null };
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
        const c = document.createElement('div');
        c.className = 'bo-caret';
        c.dataset.testid = 'caret';
        c.style.left = `${caret.x * k}px`;
        c.style.top = `${caret.y * k}px`;
        c.style.height = `${caret.h * k}px`;
        slot.overlay.appendChild(c);
      }
    }
    this.drawPeers(k);
    this.drawSpelling(k);
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
    this.opts.onContextMenu({ ...p, clientX: e.clientX, clientY: e.clientY });
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
    if (p) this.opts.onPointer?.({ type: 'up', ...p, shift: e.shiftKey, count: 1 });
  };

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

  /** Fit page width into the viewport. */
  fitWidth(): void {
    const w = this.info?.pages[0]?.width;
    if (!w) return;
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
    this.scroller.remove();
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
