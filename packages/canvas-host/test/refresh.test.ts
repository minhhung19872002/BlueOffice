// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
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
