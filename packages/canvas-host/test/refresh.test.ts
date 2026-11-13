// @vitest-environment happy-dom
import { describe, expect, it, vi } from 'vitest';
import type { EngineHost } from '@blueoffice/platform';
import { DocumentView } from '../src/DocumentView';

type Info = Parameters<DocumentView['setDocument']>[0];

const info = (pages: number): Info =>
  ({
    name: 'a.docx',
    kind: 'doc',
    pageCount: pages,
    pages: Array.from({ length: pages }, () => ({ width: 595, height: 842 })),
    wordCount: 0,
  }) as Info;

describe('DocumentView.refresh', () => {
  it('preserves fractional CSS page dimensions instead of rounding before rasterization', () => {
    const host = document.createElement('div');
    const view = new DocumentView(host, {} as EngineHost);
    view.setDocument(info(1));
    const page = host.querySelector<HTMLElement>('.bo-page')!;
    expect(Number.parseFloat(page.style.width)).toBeCloseTo((595 * 96) / 72, 6);
    expect(Number.parseFloat(page.style.height)).toBeCloseTo((842 * 96) / 72, 6);
    view.dispose();
  });
  it.each(
    [1, 1.25, 1.5, 1.75, 2].flatMap((dpr) => [0.5, 1, 2, 4].map((zoom) => [dpr, zoom] as const)),
  )('sizes the page backing store for DPR %s and zoom %s', async (dpr, zoom) => {
    const deviceRatio = vi.spyOn(window, 'devicePixelRatio', 'get').mockReturnValue(dpr);
    const frame = vi.spyOn(window, 'requestAnimationFrame').mockReturnValue(42);
    const context = { save() {}, restore() {}, setTransform() {}, fillRect() {}, fillStyle: '' };
    const getContext = vi
      .spyOn(HTMLCanvasElement.prototype, 'getContext')
      .mockReturnValue(context as never);
    const host = document.createElement('div');
    const view = new DocumentView(host, {
      getPage: async () => ({ width: 595, height: 842, commands: [] }),
      getGlyphOutlines: async () => [],
      getMedia: async () => new Uint8Array(),
    } as unknown as EngineHost);
    view.setDocument(info(1));
    view.setZoom(zoom);
    await (view as unknown as { update: () => Promise<void> }).update();
    const page = host.querySelector<HTMLElement>('.bo-page')!;
    const canvas = page.querySelector('canvas')!;
    expect(canvas.width).toBe(Math.round(Number.parseFloat(page.style.width) * dpr));
    expect(canvas.height).toBe(Math.round(Number.parseFloat(page.style.height) * dpr));
    view.dispose();
    getContext.mockRestore();
    frame.mockRestore();
    deviceRatio.mockRestore();
  });
  it('releases the backing store of a page far outside the viewport', () => {
    const host = document.createElement('div');
    const view = new DocumentView(
      host,
      {
        getPage: () => new Promise(() => {}),
      } as never as EngineHost,
      { overscan: 0 },
    );
    view.setDocument(info(8));
    const internals = view as unknown as {
      slots: { el: HTMLElement; canvas: HTMLCanvasElement | null }[];
      scroller: HTMLElement;
      lists: Map<number, Promise<unknown>>;
      update: () => Promise<void>;
    };
    internals.slots.forEach((slot, i) => {
      Object.defineProperty(slot.el, 'offsetTop', { value: i * 100 });
      Object.defineProperty(slot.el, 'offsetHeight', { value: 90 });
    });
    Object.defineProperty(internals.scroller, 'clientHeight', { value: 90 });
    internals.scroller.scrollTop = 700;
    const canvas = document.createElement('canvas');
    canvas.width = 1000;
    canvas.height = 1000;
    internals.slots[0]!.el.appendChild(canvas);
    internals.slots[0]!.canvas = canvas;
    const cached = new Promise<unknown>(() => {});
    internals.lists.set(0, cached);
    internals.lists.set(7, cached);
    void internals.update();
    expect(canvas.width).toBe(0);
    expect(canvas.height).toBe(0);
    expect(internals.slots[0]!.canvas).toBeNull();
    expect(internals.lists.has(0)).toBe(false);
    expect(internals.lists.has(7)).toBe(true);
    view.dispose();
  });
  it('paints a changed page without waiting for an animation frame', async () => {
    const frame = vi.spyOn(window, 'requestAnimationFrame').mockReturnValue(42);
    const context = { save() {}, restore() {}, setTransform() {}, fillRect() {}, fillStyle: '' };
    const getContext = vi
      .spyOn(HTMLCanvasElement.prototype, 'getContext')
      .mockReturnValue(context as never);
    const host = document.createElement('div');
    const view = new DocumentView(host, {
      getPage: async () => ({ width: 595, height: 842, commands: [] }),
      getGlyphOutlines: async () => [],
      getMedia: async () => new Uint8Array(),
      command: async () => ({}),
    } as unknown as EngineHost);
    view.setDocument(info(1));
    view.refresh(info(1), [0]);
    await vi.waitFor(() =>
      expect(host.querySelector('.bo-page')?.getAttribute('data-rendered')).toBe('true'),
    );
    view.dispose();
    frame.mockRestore();
    getContext.mockRestore();
  });
  it('does not paint an outdated page when a later edit resolves first', async () => {
    const frame = vi.spyOn(window, 'requestAnimationFrame').mockReturnValue(42);
    const colors: string[] = [];
    const context = {
      save() {},
      restore() {},
      setTransform() {},
      fillRect() {},
      set fillStyle(value: string) {
        colors.push(value);
      },
    };
    const getContext = vi
      .spyOn(HTMLCanvasElement.prototype, 'getContext')
      .mockReturnValue(context as never);
    const requests: ((page: unknown) => void)[] = [];
    const host = document.createElement('div');
    const view = new DocumentView(host, {
      getPage: () => new Promise((resolve) => requests.push(resolve)),
      getGlyphOutlines: async () => [],
      getMedia: async () => new Uint8Array(),
      command: async () => ({}),
    } as unknown as EngineHost);
    view.setDocument(info(1));
    view.refresh(info(1), [0]);
    view.refresh(info(1), [0]);
    expect(requests).toHaveLength(2);
    const page = (color: number[]) => ({
      width: 595,
      height: 842,
      commands: [{ op: 'fillRect', x: 0, y: 0, w: 2, h: 2, color }],
    });
    requests[1]!(page([255, 0, 0, 255]));
    await vi.waitFor(() =>
      expect(host.querySelector('.bo-page')?.getAttribute('data-rendered')).toBe('true'),
    );
    requests[0]!(page([0, 0, 255, 255]));
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(colors).toContain('rgba(255,0,0,1.000)');
    expect(colors).not.toContain('rgba(0,0,255,1.000)');
    view.dispose();
    frame.mockRestore();
    getContext.mockRestore();
  });
  it('re-arms the pixel-density watcher when moving between screens and removes it on dispose', () => {
    const media: { query: string; listener?: EventListener; removed: boolean }[] = [];
    const match = vi.spyOn(window, 'matchMedia').mockImplementation((query) => {
      const entry: (typeof media)[number] = { query, removed: false };
      media.push(entry);
      return {
        addEventListener: (_type: string, listener: EventListener) => {
          entry.listener = listener;
        },
        removeEventListener: () => {
          entry.removed = true;
        },
      } as unknown as MediaQueryList;
    });
    const frame = vi.spyOn(window, 'requestAnimationFrame').mockReturnValue(42);
    const host = document.createElement('div');
    const view = new DocumentView(host, {} as EngineHost);
    expect(media[0]?.query).toContain('dppx');
    media[0]?.listener?.(new Event('change'));
    expect(media).toHaveLength(2);
    expect(media[0]?.removed).toBe(true);
    expect(frame).toHaveBeenCalled();
    view.dispose();
    expect(media[1]?.removed).toBe(true);
    match.mockRestore();
    frame.mockRestore();
  });
  it('keeps the slots of unchanged pages when the page count changes', () => {
    const host = document.createElement('div');
    document.body.appendChild(host);
    // Changed pages in view are asked for at once (typing latency); they never arrive here.
    const v = new DocumentView(host, {
      getPage: () => new Promise(() => {}),
    } as never as EngineHost);
    v.setDocument(info(2));
    const first = host.querySelector('.bo-page');
    v.refresh(info(3), [1, 2]);
    const pages = host.querySelectorAll('.bo-page');
    expect(pages).toHaveLength(3);
    // Page 1 did not change: its element (and canvas) is kept, not rebuilt.
    expect(pages[0]).toBe(first);
    expect(pages[2]?.getAttribute('aria-label')).toBe('page 3');
    v.refresh(info(1), [0]);
    expect(host.querySelectorAll('.bo-page')).toHaveLength(1);
    expect(host.querySelector('.bo-page')).toBe(first);
    v.dispose();
  });
});
