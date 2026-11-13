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
  it('paints the native RGBA raster unchanged', async () => {
    const frame = vi.spyOn(window, 'requestAnimationFrame').mockReturnValue(42);
    const putImageData = vi.fn();
    const getContext = vi
      .spyOn(HTMLCanvasElement.prototype, 'getContext')
      .mockReturnValue({ putImageData } as never);
    const host = document.createElement('div');
    const view = new DocumentView(host, {
      getPage: async () => ({ width: 595, height: 842, commands: [] }),
      getGlyphOutlines: async () => [],
      getMedia: async () => new Uint8Array(),
      getNativePageRaster: async () => ({
        width: 1,
        height: 1,
        pixels: new Uint8Array([3, 2, 1, 255]),
      }),
    } as unknown as EngineHost);
    view.setDocument(info(1));
    await (view as unknown as { update: () => Promise<void> }).update();
    expect(host.querySelector('.bo-page')?.getAttribute('data-renderer')).toBe('direct2d');
    expect([...putImageData.mock.calls[0]![0].data]).toEqual([3, 2, 1, 255]);
    view.dispose();
    getContext.mockRestore();
    frame.mockRestore();
  });

  it('replaces only the changed band of a natively rendered page', async () => {
    const frame = vi.spyOn(window, 'requestAnimationFrame').mockReturnValue(42);
    const putImageData = vi.fn();
    const getContext = vi
      .spyOn(HTMLCanvasElement.prototype, 'getContext')
      .mockReturnValue({ putImageData } as never);
    const bases: (number | undefined)[] = [];
    const host = document.createElement('div');
    const view = new DocumentView(host, {
      getPage: async () => ({ width: 595, height: 842, commands: [] }),
      getGlyphOutlines: async () => [],
      getMedia: async () => new Uint8Array(),
      getNativePageRaster: async (_page: unknown, _scale: number, base?: number) => {
        bases.push(base);
        return base === undefined
          ? { width: 4, height: 4, id: 1, pixels: new Uint8Array(64) }
          : {
              width: 4,
              height: 4,
              id: 2,
              band: { left: 0, top: 2, width: 4, height: 1 },
              pixels: new Uint8Array(16).fill(9),
            };
      },
    } as unknown as EngineHost);
    view.setDocument(info(1));
    const update = () => (view as unknown as { update: () => Promise<void> }).update();
    await update();
    const canvas = host.querySelector('canvas')!;
    expect([canvas.width, canvas.height]).toEqual([4, 4]);
    view.refresh(info(1), [0]);
    await update();
    expect(bases.slice(0, 2)).toEqual([undefined, 1]);
    // The band lands at its offset; the canvas keeps its size (and so its other pixels).
    const [image, x, y] = putImageData.mock.calls[1]!;
    expect([image.width, image.height, x, y]).toEqual([4, 1, 0, 2]);
    expect([canvas.width, canvas.height]).toEqual([4, 4]);
    view.dispose();
    getContext.mockRestore();
    frame.mockRestore();
  });

  it('discards a native image when a newer page has already rendered', async () => {
    const frame = vi.spyOn(window, 'requestAnimationFrame').mockReturnValue(42);
    const putImageData = vi.fn();
    const getContext = vi
      .spyOn(HTMLCanvasElement.prototype, 'getContext')
      .mockReturnValue({ putImageData } as never);
    const pending: ((r: { width: number; height: number; pixels: Uint8Array }) => void)[] = [];
    const host = document.createElement('div');
    const view = new DocumentView(host, {
      getPage: async () => ({ width: 595, height: 842, commands: [] }),
      getGlyphOutlines: async () => [],
      getMedia: async () => new Uint8Array(),
      getNativePageRaster: () => new Promise((resolve) => pending.push(resolve)),
    } as unknown as EngineHost);
    view.setDocument(info(1));
    const first = (view as unknown as { update: () => Promise<void> }).update();
    await vi.waitFor(() => expect(pending).toHaveLength(1));
    view.refresh(info(1), [0]);
    await vi.waitFor(() => expect(pending).toHaveLength(2));
    pending[1]!({ width: 1, height: 1, pixels: new Uint8Array([0, 255, 0, 255]) });
    await vi.waitFor(() => expect(putImageData).toHaveBeenCalledTimes(1));
    pending[0]!({ width: 1, height: 1, pixels: new Uint8Array([0, 0, 255, 255]) });
    await first;
    expect(putImageData).toHaveBeenCalledTimes(1);
    expect([...putImageData.mock.calls[0]![0].data]).toEqual([0, 255, 0, 255]);
    view.dispose();
    getContext.mockRestore();
    frame.mockRestore();
  });

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
    // Shown 1:1 on device pixels (no resampling of hinted text and hairlines).
    expect(Number.parseFloat(canvas.style.width) * dpr).toBeCloseTo(canvas.width, 3);
    expect(Number.parseFloat(canvas.style.height) * dpr).toBeCloseTo(canvas.height, 3);
    view.dispose();
    getContext.mockRestore();
    frame.mockRestore();
    deviceRatio.mockRestore();
  });
  it('moves the canvas onto the device pixel grid of a fractional page position', async () => {
    const deviceRatio = vi.spyOn(window, 'devicePixelRatio', 'get').mockReturnValue(1.25);
    const frame = vi.spyOn(window, 'requestAnimationFrame').mockReturnValue(42);
    const rect = vi
      .spyOn(HTMLElement.prototype, 'getBoundingClientRect')
      .mockReturnValue({ left: 100.3, top: 50.5, width: 0, height: 0 } as DOMRect);
    const getContext = vi
      .spyOn(HTMLCanvasElement.prototype, 'getContext')
      .mockReturnValue({ putImageData() {} } as never);
    const host = document.createElement('div');
    const view = new DocumentView(host, {
      getPage: async () => ({ width: 595, height: 842, commands: [] }),
      getGlyphOutlines: async () => [],
      getMedia: async () => new Uint8Array(),
      getNativePageRaster: async () => ({ width: 2, height: 2, pixels: new Uint8Array(16) }),
    } as unknown as EngineHost);
    view.setDocument(info(1));
    await (view as unknown as { update: () => Promise<void> }).update();
    const canvas = host.querySelector('canvas')!;
    const left = 100.3 + Number.parseFloat(canvas.style.left);
    const top = 50.5 + Number.parseFloat(canvas.style.top);
    expect(left * 1.25).toBeCloseTo(Math.round(left * 1.25), 6);
    expect(top * 1.25).toBeCloseTo(Math.round(top * 1.25), 6);
    expect(Number.parseFloat(canvas.style.width)).toBeCloseTo(2 / 1.25, 6);
    view.dispose();
    getContext.mockRestore();
    rect.mockRestore();
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
