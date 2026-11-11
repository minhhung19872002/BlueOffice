// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import type { EngineHost } from '@blueoffice/platform';
import { DocumentView } from '../src/DocumentView';

describe('DocumentView collaborators', () => {
  it('draws coloured carets with names and selections, kept across own selection changes', () => {
    const host = document.createElement('div');
    document.body.appendChild(host);
    const v = new DocumentView(host, {} as EngineHost);
    v.setDocument({
      name: 'a.docx',
      kind: 'doc',
      pageCount: 1,
      pages: [{ width: 595, height: 842 }],
      wordCount: 0,
    } as Parameters<DocumentView['setDocument']>[0]);
    v.setPeers([
      {
        caret: { page: 0, x: 72, y: 72, h: 14 },
        rects: [{ page: 0, x: 72, y: 90, w: 100, h: 14 }],
        color: '#15803d',
        name: 'Trần Thị Bình',
        label: 'Trần Thị Bình đang sửa',
      },
    ]);
    const caret = host.querySelector<HTMLElement>('[data-testid="peer-caret"]');
    expect(caret?.getAttribute('aria-label')).toBe('Trần Thị Bình đang sửa');
    expect(caret?.textContent).toBe('Trần Thị Bình');
    expect(host.querySelectorAll('.bo-peer-sel')).toHaveLength(1);
    // The own caret moving redraws the overlay; collaborators stay.
    v.setSelection({ page: 0, x: 10, y: 10, h: 14 }, [], false);
    expect(host.querySelectorAll('[data-testid="peer-caret"]')).toHaveLength(1);
    v.setPeers([]);
    expect(host.querySelectorAll('[data-testid="peer-caret"]')).toHaveLength(0);
    v.dispose();
  });

  it('keeps one caret element and one equation box across redraws', () => {
    const host = document.createElement('div');
    document.body.appendChild(host);
    const v = new DocumentView(host, {} as EngineHost);
    v.setDocument({
      name: 'a.docx',
      kind: 'doc',
      pageCount: 1,
      pages: [{ width: 595, height: 842 }],
      wordCount: 0,
    } as Parameters<DocumentView['setDocument']>[0]);
    const zone = { page: 0, x: 50, y: 50, w: 40, h: 30 };
    v.setMathMarks({ zone, placeholders: [] } as unknown as Parameters<
      DocumentView['setMathMarks']
    >[0]);
    v.setSelection({ page: 0, x: 10, y: 10, h: 14 }, [], false);
    const caret = host.querySelector('[data-testid="caret"]');
    const box = host.querySelector('[data-testid="bo-math-zone"]');
    // A background redraw (spelling marks) and a caret move keep the same nodes.
    v.setSpelling([]);
    v.setSelection({ page: 0, x: 20, y: 10, h: 14 }, [], false);
    expect(host.querySelectorAll('[data-testid="caret"]')).toHaveLength(1);
    expect(host.querySelector('[data-testid="caret"]')).toBe(caret);
    expect(host.querySelector('[data-testid="bo-math-zone"]')).toBe(box);
    expect((caret as HTMLElement).style.left).not.toBe('');
    // A selection hides the caret.
    v.setSelection(null, [{ page: 0, x: 10, y: 10, w: 30, h: 14 }], false);
    expect(host.querySelector('[data-testid="caret"]')).toBeNull();
    v.dispose();
  });

  it('draws a margin bar beside the paragraph of each collaborator, side by side', () => {
    const host = document.createElement('div');
    document.body.appendChild(host);
    const v = new DocumentView(host, {} as EngineHost);
    v.setDocument({
      name: 'a.docx',
      kind: 'doc',
      pageCount: 1,
      pages: [{ width: 595, height: 842 }],
      wordCount: 0,
    } as Parameters<DocumentView['setDocument']>[0]);
    const para = { page: 0, x: 72, y: 100, h: 42 };
    v.setPeers([
      {
        caret: { page: 0, x: 90, y: 114, h: 14 },
        rects: [],
        para,
        color: '#15803d',
        name: 'Bình',
        label: 'Bình đang sửa',
        paraLabel: 'Bình đang sửa đoạn này',
      },
      {
        caret: { page: 0, x: 120, y: 100, h: 14 },
        rects: [],
        para,
        color: '#7c3aed',
        name: 'Chi',
        label: 'Chi đang sửa',
      },
      { caret: null, rects: [], para: null, color: '#be185d', name: 'Dũng', label: 'Dũng' },
    ]);
    const bars = [...host.querySelectorAll<HTMLElement>('[data-testid="peer-bar"]')];
    expect(bars).toHaveLength(2);
    expect(bars[0]!.getAttribute('aria-label')).toBe('Bình đang sửa đoạn này');
    expect(bars[1]!.getAttribute('aria-label')).toBe('Chi đang sửa');
    // Same paragraph: the second bar sits further out in the margin, without a second tag.
    const left = (e: HTMLElement) => parseFloat(e.style.left);
    expect(left(bars[1]!)).toBeLessThan(left(bars[0]!));
    const caret = host.querySelector<HTMLElement>('[data-testid="peer-caret"]')!;
    expect(left(bars[0]!)).toBeLessThan(left(caret));
    expect(bars[0]!.style.top).toBe(bars[1]!.style.top);
    // The bar spans the paragraph (42 pt tall, 100 pt from the top).
    expect(parseFloat(bars[0]!.style.height) / parseFloat(bars[0]!.style.top)).toBeCloseTo(0.42);
    expect(bars[0]!.textContent).toBe('Bình');
    expect(bars[1]!.textContent).toBe('');
    v.dispose();
  });
});
