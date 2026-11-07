// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { OpenedFile, Platform } from '@blueoffice/platform';
import { BlueSlideEditor, fit, formatElapsed, showAction, step } from '../src';

afterEach(cleanup);

describe('show logic', () => {
  it('maps keys and steps through the deck', () => {
    expect(showAction('ArrowRight')).toBe('next');
    expect(showAction(' ')).toBe('next');
    expect(showAction('PageUp')).toBe('prev');
    expect(showAction('Escape')).toBe('exit');
    expect(showAction('b')).toBe('black');
    expect(showAction('x')).toBeNull();
    expect(step(2, 3, 'next')).toBe(3); // end-of-show screen
    expect(step(3, 3, 'next')).toBe(3);
    expect(step(0, 3, 'prev')).toBe(0);
    expect(step(1, 3, 'last')).toBe(2);
  });

  it('formats time and fits slides', () => {
    expect(formatElapsed(65_000)).toBe('01:05');
    expect(formatElapsed(3_723_000)).toBe('1:02:03');
    expect(fit({ width: 960, height: 540 }, { width: 800, height: 800 })).toEqual({
      width: 800,
      height: 450,
    });
    expect(fit({ width: 0, height: 0 }, { width: 1, height: 1 })).toEqual({ width: 0, height: 0 });
  });
});

function fakePlatform() {
  let openHandler: ((f: OpenedFile) => void) | null = null;
  const saved: string[] = [];
  const engine = {
    backend: 'wasm',
    init: async () => {},
    open: vi.fn(async (_b: Uint8Array, name: string) => ({
      name,
      pageCount: 3,
      pages: [0, 1, 2].map(() => ({ width: 960, height: 540 })),
      wordCount: 0,
      kind: 'slide' as const,
    })),
    getPage: async () => ({ width: 960, height: 540, commands: [] }),
    getGlyphOutlines: async () => [],
    getMedia: async () => new Uint8Array(),
    getSlideNotes: vi.fn(async (i: number) => `Ghi chú ${i + 1}`),
    savePptx: vi.fn(async () => new Uint8Array([80, 75])),
    exportPdf: async () => new Uint8Array([37]),
  };
  const p = {
    kind: 'web',
    engine,
    files: {
      openFile: async () => ({ name: 'Báo cáo.pptx', bytes: new Uint8Array([1]) }),
      saveFile: async (_b: Uint8Array, n: string) => {
        saved.push(n);
        return n;
      },
    },
    print: { printPdf: async () => {} },
    window: { setTitle: vi.fn(), openNew: async () => {} },
    shell: {
      recentFiles: async () => [],
      addRecentFile: async () => {},
      onOpenRequest: (h: (f: OpenedFile) => void) => {
        openHandler = h;
        return () => {};
      },
    },
  } as unknown as Platform;
  return { p, engine, saved, open: (f: OpenedFile) => openHandler?.(f) };
}

describe('BlueSlideEditor', () => {
  it('opens a deck, shows thumbnails and notes, navigates and saves', async () => {
    const { p, engine, saved } = fakePlatform();
    render(<BlueSlideEditor platform={p} language="vi-VN" />);
    fireEvent.click(screen.getByText('Mở'));
    await screen.findByText('Ghi chú 1');
    expect(engine.open).toHaveBeenCalledWith(expect.any(Uint8Array), 'Báo cáo.pptx');
    expect(screen.getAllByRole('img', { name: /Slide \d \/ 3/ }).length).toBeGreaterThanOrEqual(3);
    fireEvent.keyDown(window, { key: 'ArrowDown' });
    await screen.findByText('Ghi chú 2');
    expect(screen.getByText('Slide 2 / 3')).toBeTruthy();
    fireEvent.keyDown(window, { key: 's', ctrlKey: true });
    await waitFor(() => expect(saved).toEqual(['Báo cáo.pptx']));
  });

  it('runs the presenter view with next slide, notes and timer, Esc exits', async () => {
    const { p } = fakePlatform();
    render(<BlueSlideEditor platform={p} language="en-US" />);
    fireEvent.click(screen.getByText('Open'));
    await screen.findByText('Ghi chú 1');
    fireEvent.click(screen.getByText('Presenter view'));
    await screen.findByText('Next slide', { selector: 'h3' });
    expect(screen.getByText('00:00')).toBeTruthy();
    fireEvent.keyDown(window, { key: 'ArrowRight' });
    await screen.findByText('Ghi chú 2');
    fireEvent.keyDown(window, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByText('Next slide', { selector: 'h3' })).toBeNull());
    expect(screen.getByText('Slide 2 of 3')).toBeTruthy();
  });

  it('switches to the slide sorter and back', async () => {
    const { p } = fakePlatform();
    render(<BlueSlideEditor platform={p} language="en-US" />);
    fireEvent.click(screen.getByText('Open'));
    await screen.findByText('Ghi chú 1');
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'View' }));
    fireEvent.click(await screen.findByText('Slide sorter'));
    const items = await screen.findAllByRole('img', { name: /Slide \d of 3/ });
    fireEvent.doubleClick(items[2]!.parentElement!);
    await screen.findByText('Ghi chú 3');
  });
});
