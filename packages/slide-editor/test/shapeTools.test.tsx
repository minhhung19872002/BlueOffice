// @vitest-environment happy-dom
import { CommandRegistry } from '@blueoffice/editor';
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { SlideCommand, SlideEditResult, SlideFreeformInfo } from '@blueoffice/platform';
import { Ribbon } from '@blueoffice/ui';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { EditPointsOverlay, dragEdit, nearestSegment, nearestSite } from '../src/editPoints';
import {
  MERGE_OPS,
  registerShapeTools,
  shapeFormatTab,
  type ShapeToolState,
} from '../src/shapeFormat';

afterEach(cleanup);

const i18n = createI18n('vi-VN');
const t = i18n.t.bind(i18n) as unknown as Parameters<typeof shapeFormatTab>[0]['t'];

/** A 100 × 100 square at (100, 100) with a curved left side. */
const square: SlideFreeformInfo = {
  id: 7,
  paths: [
    {
      points: [
        { x: 100, y: 100, kind: 'corner', handles: [[100, 133], null] },
        { x: 200, y: 100, kind: 'corner', handles: [null, null] },
        { x: 200, y: 200, kind: 'corner', handles: [null, null] },
        { x: 100, y: 200, kind: 'corner', handles: [null, [100, 167]] },
      ],
      segments: [
        { curved: false, mid: [150, 100] },
        { curved: false, mid: [200, 150] },
        { curved: false, mid: [150, 200] },
        { curved: true, mid: [100, 150] },
      ],
      d: 'M100 100L200 100L200 200L100 200C100 167 100 133 100 100Z',
    },
  ],
};

describe('shape tool helpers', () => {
  it('finds the connection site a connector end snaps to', () => {
    const sites: [number, number][] = [
      [0, 0],
      [10, 0],
      [20, 0],
    ];
    expect(nearestSite(sites, 9, 1, 5)).toEqual([10, 0]);
    // 6 units from the nearest site: outside a tolerance of 5.
    expect(nearestSite(sites, 26, 0, 5)).toBeNull();
    expect(nearestSite([], 0, 0, 5)).toBeNull();
  });

  it('picks the segment under the pointer and builds drag edits', () => {
    expect(nearestSegment(square, 101, 140)).toEqual({ path: 0, segment: 3, curved: true });
    expect(nearestSegment(square, 150, 98)).toEqual({ path: 0, segment: 0, curved: false });
    expect(dragEdit({ path: 0, point: 2 }, 5, 6)).toEqual({
      kind: 'move',
      path: 0,
      point: 2,
      x: 5,
      y: 6,
    });
    expect(dragEdit({ path: 1, point: 0, side: 1 }, 5, 6)).toEqual({
      kind: 'handle',
      path: 1,
      point: 0,
      side: 1,
      x: 5,
      y: 6,
    });
  });
});

describe('shape tool commands', () => {
  function setup() {
    const sent: SlideCommand[] = [];
    const editing = vi.fn();
    const reg = new CommandRegistry<ShapeToolState>();
    registerShapeTools(reg, (c) => sent.push(c), editing);
    return { sent, editing, reg };
  }

  it('maps the commands onto engine commands', async () => {
    const { sent, editing, reg } = setup();
    const one: ShapeToolState = { isShape: true };
    const many: ShapeToolState = { mergeable: true };
    expect(reg.isEnabled('shape.editPoints', one)).toBe(true);
    expect(reg.isEnabled('shape.editPoints', many)).toBe(false);
    expect(reg.isEnabled('shape.merge.union', one)).toBe(false);
    expect(reg.isEnabled('shape.merge.union', many)).toBe(true);
    await reg.execute('shape.editPoints', one);
    expect(sent.at(-1)).toEqual({ cmd: 'convertToFreeform' });
    expect(editing).toHaveBeenLastCalledWith(true);
    // Toggling while editing closes the point editor.
    await reg.execute('shape.editPoints', { isShape: true, editingPoints: true });
    expect(editing).toHaveBeenLastCalledWith(false);
    for (const op of MERGE_OPS) {
      await reg.execute(`shape.merge.${op}`, many);
      expect(sent.at(-1)).toEqual({ cmd: 'mergeShapes', op });
    }
  });

  it('shows the dropdowns in the shape tab', () => {
    const { reg } = setup();
    const state: ShapeToolState = { mergeable: true };
    const exec = vi.fn();
    const tab = shapeFormatTab({
      t,
      state,
      enabled: (id) => reg.isEnabled(id, state),
      exec,
    });
    render(
      <I18nextProvider i18n={i18n}>
        <Ribbon tabs={[tab]} value="shapeFormat" />
      </I18nextProvider>,
    );
    expect(tab.label).toBe('Hình');
    fireEvent.click(screen.getByTestId('shape-merge-menu'));
    const subtract = screen.getByTestId('menu-shape.merge.subtract');
    expect(subtract.textContent).toBe('Trừ bớt');
    fireEvent.click(subtract);
    expect(exec).toHaveBeenCalledWith('shape.merge.subtract');
    // Edit points needs a single shape.
    expect(
      (screen.getByTestId('shape-edit-menu') as HTMLButtonElement).disabled ||
        screen.getByTestId('shape-edit-menu').getAttribute('aria-disabled') === 'true',
    ).toBe(true);
  });
});

describe('point editor overlay', () => {
  function setup() {
    const sent: SlideCommand[] = [];
    const run = vi.fn(async (c: SlideCommand) => {
      sent.push(c);
      return null as SlideEditResult | null;
    });
    const onExit = vi.fn();
    const labels = {
      overlay: 'editor',
      point: (n: number) => `P${n}`,
      handle: 'h',
      addHint: 'add',
      deletePoint: 'del',
      corner: 'corner',
      smooth: 'smooth',
      symmetric: 'sym',
      straight: 'straight',
      curved: 'curved',
    };
    const view = render(
      <EditPointsOverlay
        slide={{ width: 400, height: 300 }}
        width={400}
        freeform={square}
        run={run}
        onExit={onExit}
        labels={labels}
      />,
    );
    // The overlay maps client pixels 1:1 onto slide points.
    const svg = view.container.querySelector('svg')!;
    svg.getBoundingClientRect = () =>
      ({ left: 0, top: 0, width: 400, height: 300, right: 400, bottom: 300 }) as DOMRect;
    return { sent, run, onExit };
  }

  it('draws the anchors and control points', () => {
    setup();
    expect(screen.getAllByTestId(/^edit-point-0-/)).toHaveLength(4);
    expect(screen.getByTestId('edit-control-0-0-0')).toBeTruthy();
    expect(screen.getByTestId('edit-control-0-3-1')).toBeTruthy();
    expect(screen.queryByTestId('edit-control-0-1-0')).toBeNull();
  });

  it('drags a point as one undo step and deletes it with the keyboard', async () => {
    const { sent } = setup();
    const p = screen.getByTestId('edit-point-0-2');
    await act(async () => {
      fireEvent.pointerDown(p, { button: 0, clientX: 200, clientY: 200, pointerId: 1 });
      fireEvent.pointerMove(p, { clientX: 230, clientY: 210, pointerId: 1 });
    });
    expect(sent[0]).toMatchObject({
      cmd: 'editPoint',
      edit: { kind: 'move', path: 0, point: 2, x: expect.closeTo(230, 6), y: 210 },
      merge: false,
    });
    await act(async () => {
      fireEvent.pointerMove(p, { clientX: 240, clientY: 220, pointerId: 1 });
      fireEvent.pointerUp(p, { clientX: 250, clientY: 220, pointerId: 1 });
    });
    const last = sent.at(-1);
    expect(last).toMatchObject({ cmd: 'editPoint', merge: true });
    expect(last && 'edit' in last ? last.edit : null).toMatchObject({
      x: expect.closeTo(250, 6),
      y: expect.closeTo(220, 6),
    });
    // The dragged point is selected: Delete removes it.
    fireEvent.keyDown(screen.getByTestId('edit-points'), { key: 'Delete' });
    expect(sent.at(-1)).toEqual({
      cmd: 'editPoint',
      edit: { kind: 'delete', path: 0, point: 2 },
    });
  });

  it('adds a point on the outline and changes types from the context menu', () => {
    const { sent, onExit } = setup();
    fireEvent.pointerDown(screen.getByTestId('edit-points-outline-0'), {
      button: 0,
      clientX: 150,
      clientY: 101,
    });
    expect(sent.at(-1)).toMatchObject({
      cmd: 'editPoint',
      edit: { kind: 'add', x: expect.closeTo(150, 6), y: expect.closeTo(101, 6), tolerance: 10 },
    });
    fireEvent.contextMenu(screen.getByTestId('edit-point-0-0'), { clientX: 100, clientY: 100 });
    fireEvent.click(screen.getByTestId('edit-points-smooth'));
    expect(sent.at(-1)).toEqual({
      cmd: 'editPoint',
      edit: { kind: 'pointType', path: 0, point: 0, pointType: 'smooth' },
    });
    fireEvent.contextMenu(screen.getByTestId('edit-points-outline-0'), {
      clientX: 101,
      clientY: 150,
    });
    fireEvent.click(screen.getByTestId('edit-points-straight'));
    expect(sent.at(-1)).toEqual({
      cmd: 'editPoint',
      edit: { kind: 'segment', path: 0, segment: 3, curved: false },
    });
    // Escape leaves the point editor.
    fireEvent.keyDown(screen.getByTestId('edit-points'), { key: 'Escape' });
    expect(onExit).toHaveBeenCalled();
  });
});
