import type { DocInfo, EngineHost, PageDisplayList } from '@blueoffice/platform';
import { ImeInput, type ImeEvent } from './ime';
import { GlyphStore, PX_PER_PT, renderPage } from './renderer';

export interface DocumentViewOptions {
  /** Gap between pages in CSS pixels. */
  pageGap?: number;
  /** Extra pages rendered above/below the viewport. */
  overscan?: number;
  onPageChange?: (page: number) => void;
  onZoomChange?: (zoom: number) => void;
  onInput?: (e: ImeEvent) => void;
}

export const MIN_ZOOM = 0.25;
export const MAX_ZOOM = 5;

interface PageSlot {
  el: HTMLDivElement;
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
  private readonly ime: ImeInput;
  private slots: PageSlot[] = [];
  private lists = new Map<number, Promise<PageDisplayList>>();
  private info: DocInfo | null = null;
  private zoomValue = 1;
  private frame = 0;
  private currentPage = -1;
  private readonly opts: Required<
    Omit<DocumentViewOptions, 'onPageChange' | 'onZoomChange' | 'onInput'>
  > &
    DocumentViewOptions;

  constructor(
    private readonly host: HTMLElement,
    private readonly engine: EngineHost,
    opts: DocumentViewOptions = {},
  ) {
    this.opts = { pageGap: 16, overscan: 1, ...opts };
    this.glyphs = new GlyphStore(engine);
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
    this.scroller.addEventListener('mousedown', () => this.ime.focus());
    window.addEventListener('resize', this.schedule);
  }

  get zoom(): number {
    return this.zoomValue;
  }

  /** Show a newly opened document. */
  setDocument(info: DocInfo): void {
    this.info = info;
    this.lists.clear();
    this.content.replaceChildren();
    this.slots = info.pages.map((_, i) => {
      const el = document.createElement('div');
      el.className = 'bo-page';
      el.dataset.page = String(i + 1);
      el.setAttribute('aria-label', `page ${i + 1}`);
      el.style.cssText =
        'position:relative;background:#fff;box-shadow:0 1px 3px rgba(0,0,0,.25),0 0 0 1px rgba(0,0,0,.06);flex:none;';
      this.content.appendChild(el);
      return { el, canvas: null, renderedAt: null };
    });
    this.currentPage = -1;
    this.layoutSlots();
    this.scroller.scrollTop = 0;
    this.schedule();
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

  dispose(): void {
    cancelAnimationFrame(this.frame);
    window.removeEventListener('resize', this.schedule);
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
        await this.glyphs.ensure(list);
        const scale = PX_PER_PT * this.zoomValue * dpr;
        const canvas = slot.canvas ?? document.createElement('canvas');
        canvas.width = Math.round(list.width * scale);
        canvas.height = Math.round(list.height * scale);
        canvas.style.cssText = 'position:absolute;inset:0;width:100%;height:100%;';
        const ctx = canvas.getContext('2d');
        if (!ctx) return;
        renderPage(ctx, list, this.glyphs, scale);
        if (!slot.canvas) {
          slot.el.appendChild(canvas);
          slot.canvas = canvas;
        }
        slot.renderedAt = key;
        slot.el.dataset.rendered = 'true';
      }),
    );
  }
}
