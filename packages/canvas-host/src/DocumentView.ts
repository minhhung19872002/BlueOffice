import type {
  CaretRect,
  DocInfo,
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
export class DocumentView {
  readonly scroller: HTMLDivElement;
  private readonly content: HTMLDivElement;
  private readonly glyphs: GlyphStore;
  private readonly images: ImageStore;
  private dragging = false;
  private caret: CaretRect | null = null;
  private readonly ime: ImeInput;
  private slots: PageSlot[] = [];
  private lists = new Map<number, Promise<PageDisplayList>>();
  private info: DocInfo | null = null;
  private zoomValue = 1;
  private frame = 0;
  private currentPage = -1;
  private readonly opts: Required<
    Omit<DocumentViewOptions, 'onPageChange' | 'onZoomChange' | 'onInput' | 'onPointer'>
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
    window.addEventListener('mousemove', this.onMouseMove);
    window.addEventListener('mouseup', this.onMouseUp);
    window.addEventListener('resize', this.schedule);
    if (!document.getElementById('bo-caret-style')) {
      const st = document.createElement('style');
      st.id = 'bo-caret-style';
      st.textContent =
        '@keyframes bo-blink{0%,49%{opacity:1}50%,100%{opacity:0}}.bo-caret{position:absolute;width:1.5px;background:#000;animation:bo-blink 1.06s step-end infinite;pointer-events:none}.bo-sel{position:absolute;background:rgba(51,120,220,.32);pointer-events:none}';
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

  private buildSlots(info: DocInfo): void {
    this.info = info;
    this.lists.clear();
    this.content.replaceChildren();
    this.slots = info.pages.map((_, i) => {
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
    });
    this.currentPage = -1;
    this.layoutSlots();
  }

  /**
   * Repaint after an edit: `dirty` pages are fetched again; a changed page
   * count or size rebuilds the page slots (scroll position is kept).
   */
  refresh(info: DocInfo, dirty: number[]): void {
    const old = this.info;
    const same =
      old &&
      old.pages.length === info.pages.length &&
      old.pages.every(
        (p, i) => p.width === info.pages[i]?.width && p.height === info.pages[i]?.height,
      );
    if (!same) {
      const top = this.scroller.scrollTop;
      this.buildSlots(info);
      this.scroller.scrollTop = top;
    } else {
      this.info = info;
      for (const i of dirty) {
        this.lists.delete(i);
        const s = this.slots[i];
        if (s) s.renderedAt = null;
      }
    }
    this.schedule();
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
