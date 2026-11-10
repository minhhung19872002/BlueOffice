// @vitest-environment happy-dom
import type { SlideShapeInfo } from '@blueoffice/platform';
import { OBJECT_COMMANDS, type RibbonTab } from '@blueoffice/ui';
import { describe, expect, it } from 'vitest';
import { DRAW_COMMANDS } from '../src/draw/drawTab';
import { slideObjectContexts, slideObjectTabs } from '../src/objectContexts';

// CORE-UI-008: BlueSlide's Equation, Object and Ink tabs.

const shape = (kind: SlideShapeInfo['kind'], equation?: boolean) => ({ kind, equation });
const t = (k: string) => k;
const item = (id: string, label: string) => ({ id, label, onSelect: () => {} });
const ids = (tab: RibbonTab | undefined) => tab?.groups.flatMap((g) => g.items.map((i) => i.id));

describe('slideObjectContexts', () => {
  it('follows the single selected shape and the Draw tool in use', () => {
    expect(slideObjectContexts(undefined, 'select')).toEqual([]);
    expect(slideObjectContexts([shape('shape')], 'select')).toEqual([]);
    expect(slideObjectContexts([shape('shape', true)], 'select')).toEqual(['equation']);
    expect(slideObjectContexts([shape('object')], 'select')).toEqual(['object']);
    // Several shapes: no object tab.
    expect(slideObjectContexts([shape('object'), shape('shape', true)], 'select')).toEqual([]);
    expect(slideObjectContexts([], 'pen')).toEqual(['ink']);
    expect(slideObjectContexts([shape('object')], 'eraserStroke')).toEqual(['object', 'ink']);
  });
});

describe('slideObjectTabs', () => {
  it('reuses the arrange / edit, object and draw commands', () => {
    const [eq, obj, ink] = slideObjectTabs(t, item, ['equation', 'object', 'ink']);
    expect(eq?.context).toBe('equation');
    expect(ids(eq)).toEqual(['arrange.front', 'arrange.back', 'edit.duplicate', 'edit.delete']);
    expect(obj?.context).toBe('object');
    for (const id of ids(obj) ?? []) expect(OBJECT_COMMANDS).toContain(id);
    expect(ink?.context).toBe('ink');
    expect(ids(ink)).toEqual([
      'draw.pen',
      'draw.highlighter',
      'draw.select',
      'draw.eraserStroke',
      'draw.eraserPoint',
      'draw.replay',
    ]);
    for (const id of ids(ink) ?? []) expect(DRAW_COMMANDS).toContain(id);
  });
});
