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
    void internals.update();
    expect(canvas.width).toBe(0);
    expect(canvas.height).toBe(0);
    expect(internals.slots[0]!.canvas).toBeNull();
    view.dispose();
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
