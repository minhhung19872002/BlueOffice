// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import type { EngineHost, FormatMark } from '@blueoffice/platform';
import { DocumentView } from '../src/DocumentView';
import { drawFormattingMarks, markSymbol, type MarkCanvas } from '../src/marks';

const mark = (kind: FormatMark['kind'], x: number, w = 4): FormatMark => ({
  kind,
  x,
  w,
  top: 70,
  h: 16,
  baseline: 82,
  size: 12,
});

function fakeCanvas() {
  const calls: string[] = [];
  const ctx: MarkCanvas = {
    save: () => calls.push('save'),
    restore: () => calls.push('restore'),
    setTransform: (a) => calls.push(`scale ${a}`),
    beginPath: () => {},
    moveTo: () => {},
    lineTo: () => {},
    stroke: () => calls.push('stroke'),
    fillRect: () => calls.push('dot'),
    fillText: (t, x) => calls.push(`text ${t} ${x.toFixed(1)}`),
    setLineDash: () => {},
    fillStyle: '',
    strokeStyle: '',
    lineWidth: 1,
    font: '',
  };
  return { ctx, calls };
}

describe('formatting marks', () => {
  it('draws ¶ at paragraph ends, dots for spaces, arrows for tabs', () => {
    const { ctx, calls } = fakeCanvas();
    drawFormattingMarks(
      ctx,
      [mark('space', 80), mark('tab', 90, 30), mark('lineBreak', 130), mark('paragraph', 140, 0)],
      2,
    );
    expect(calls[1]).toBe('scale 2');
    expect(calls).toContain('dot');
    expect(calls.some((c) => c.startsWith('text →'))).toBe(true);
    expect(calls.some((c) => c.startsWith('text ↵'))).toBe(true);
    expect(calls).toContain('text ¶ 140.5');
    expect(calls.at(-1)).toBe('restore');
  });

  it('draws page breaks as a rule and has a symbol for each visible kind', () => {
    const { ctx, calls } = fakeCanvas();
    drawFormattingMarks(ctx, [mark('pageBreak', 72, 0)], 1);
    expect(calls).toContain('stroke');
    expect(markSymbol('paragraph')).toBe('¶');
    expect(markSymbol('tab')).toBe('→');
    expect(markSymbol('pageBreak')).toBe('');
  });
});

describe('DocumentView page modes', () => {
  const info = {
    name: 'a.docx',
    kind: 'doc',
    pageCount: 2,
    pages: [
      { width: 595, height: 842 },
      { width: 595, height: 842 },
    ],
    wordCount: 0,
  } as Parameters<DocumentView['setDocument']>[0];

  it('stacks pages without gaps in web and draft views, dashed page boundaries in draft', () => {
    const host = document.createElement('div');
    document.body.appendChild(host);
    const v = new DocumentView(host, {} as EngineHost);
    v.setDocument(info);
    const pages = () => [...host.querySelectorAll<HTMLElement>('.bo-page')];
    expect(pages()[0]?.style.boxShadow).not.toBe('none');
    v.setPageMode('draft');
    expect(v.pageMode).toBe('draft');
    expect(pages()[0]?.style.boxShadow).toBe('none');
    expect(pages()[1]?.style.borderTop).toContain('dashed');
    v.setPageMode('web');
    expect(pages()[1]?.style.borderTop).toBe('');
    const content = host.querySelector<HTMLElement>('.bo-doc-scroller > div');
    expect(content?.style.gap).toBe('0px');
    v.setPageMode('print');
    expect(content?.style.gap).toBe('16px');
    expect(v.pageCount).toBe(2);
    v.dispose();
  });
});
