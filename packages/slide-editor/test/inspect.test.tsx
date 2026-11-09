// @vitest-environment happy-dom
import type { InspectReport, SlideCommand, SlideEditResult } from '@blueoffice/platform';
import { describe, expect, it, vi } from 'vitest';
import { slideCheckBridge } from '../src/inspect';

const report: InspectReport = {
  inspector: [],
  accessibility: [],
  issueCount: 0,
  compatibility: [],
};

describe('BlueSlide check document', () => {
  it('maps the dialog onto slide commands', async () => {
    const sent: SlideCommand[] = [];
    const run = vi.fn(async (c: SlideCommand) => {
      sent.push(c);
      return { inspection: report } as SlideEditResult;
    });
    const b = slideCheckBridge(run);
    expect(await b.inspect()).toEqual(report);
    await b.remove(['hiddenSlides', 'notes']);
    await b.keep(['comments']);
    await b.fix?.({ kind: 'altText', target: { kind: 'shape', slide: 1, id: 4 }, text: 'Logo' });
    b.goTo?.({ kind: 'shape', slide: 2, id: 7 });
    await vi.waitFor(() => expect(sent).toHaveLength(6));
    b.goTo?.({ kind: 'slide', slide: 0 });
    expect(sent).toEqual([
      { cmd: 'inspect' },
      { cmd: 'inspectRemove', categories: ['hiddenSlides', 'notes'] },
      { cmd: 'inspectKeep', categories: ['comments'] },
      {
        cmd: 'inspectFix',
        fix: { kind: 'altText', target: { kind: 'shape', slide: 1, id: 4 }, text: 'Logo' },
      },
      { cmd: 'goTo', slide: 2 },
      { cmd: 'select', ids: [7] },
      { cmd: 'goTo', slide: 0 },
    ]);
  });
});
