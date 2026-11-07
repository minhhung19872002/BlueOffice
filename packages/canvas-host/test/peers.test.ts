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
});
