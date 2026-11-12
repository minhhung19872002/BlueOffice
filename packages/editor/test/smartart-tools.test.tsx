// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { SlideSmartArtItem } from '@blueoffice/platform';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CommandRegistry } from '../src/commands';
import {
  PANE_COMMIT_MS,
  SMARTART_COMMANDS,
  SMARTART_RESET,
  SmartArtSelection,
  SmartArtTextPane,
  canSmartArtAction,
  paneKey,
  panePosition,
  registerSmartArtTools,
  smartArtAction,
  smartArtDesignGroups,
  smartArtDesignLabels,
  type SmartArtState,
  type SmartArtToolsApi,
} from '../src/SmartArtTools';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);
const t = (k: string, o?: Record<string, unknown>) => i18n.t(k, o);

const it_ = (text: string, level: number, assistant?: boolean): SlideSmartArtItem =>
  assistant ? { text, level, assistant } : { text, level };

/** An organisation chart: A ▸ (B ▸ C), D. */
const org: SlideSmartArtItem[] = [it_('A', 0), it_('B', 1), it_('C', 2), it_('D', 1)];

// source: black-box Office behaviour of the SmartArt Design tab (Add Shape After / Before / Above /
// Below / Add Assistant, Promote, Demote, Move Up / Down) — CORE-SMART-004.
describe('SmartArt Design actions', () => {
  it('adds shapes after, before, above, below and an assistant, selecting the new shape', () => {
    // After B: after B and its sub-shape C, at B's level.
    expect(smartArtAction(org, 1, 'addAfter')).toEqual({
      items: [it_('A', 0), it_('B', 1), it_('C', 2), it_('', 1), it_('D', 1)],
      node: 3,
    });
    // Before B: right before it.
    expect(smartArtAction(org, 1, 'addBefore')).toEqual({
      items: [it_('A', 0), it_('', 1), it_('B', 1), it_('C', 2), it_('D', 1)],
      node: 1,
    });
    // Above B: a new shape in B's place, B and C one level down under it.
    expect(smartArtAction(org, 1, 'addAbove')).toEqual({
      items: [it_('A', 0), it_('', 1), it_('B', 2), it_('C', 3), it_('D', 1)],
      node: 1,
    });
    // Below B: a last child of B.
    expect(smartArtAction(org, 1, 'addBelow')).toEqual({
      items: [it_('A', 0), it_('B', 1), it_('C', 2), it_('', 2), it_('D', 1)],
      node: 3,
    });
    // An assistant: first under the shape, marked.
    expect(smartArtAction(org, 0, 'addAssistant')).toEqual({
      items: [it_('A', 0), it_('', 1, true), it_('B', 1), it_('C', 2), it_('D', 1)],
      node: 1,
    });
    // No shape selected: after = at the end, before = at the start, at the top level.
    expect(smartArtAction(org, null, 'addAfter')?.items.at(-1)).toEqual(it_('', 0));
    expect(smartArtAction(org, null, 'addBefore')?.node).toBe(0);
    expect(smartArtAction(org, null, 'addBelow')).toBeNull();
  });

  it('promotes, demotes and moves a shape with its sub-shapes', () => {
    expect(smartArtAction(org, 1, 'promote')?.items).toEqual([
      it_('A', 0),
      it_('B', 0),
      it_('C', 1),
      it_('D', 1),
    ]);
    expect(smartArtAction(org, 3, 'demote')?.items).toEqual([
      it_('A', 0),
      it_('B', 1),
      it_('C', 2),
      it_('D', 2),
    ]);
    expect(smartArtAction(org, 3, 'up')).toEqual({
      items: [it_('A', 0), it_('D', 1), it_('B', 1), it_('C', 2)],
      node: 1,
    });
    expect(smartArtAction(org, 1, 'down')).toEqual({
      items: [it_('A', 0), it_('D', 1), it_('B', 1), it_('C', 2)],
      node: 2,
    });
    // Not applicable: promote a top shape, move the last shape down, demote a first child.
    expect(smartArtAction(org, 0, 'promote')).toBeNull();
    expect(smartArtAction(org, 3, 'down')).toBeNull();
    expect(smartArtAction(org, 1, 'demote')).toBeNull();
  });

  it('enables the buttons as the selection allows', () => {
    const s = { items: org, layout: 'hierarchy' as const };
    expect(canSmartArtAction(s, null, 'addAfter')).toBe(true);
    expect(canSmartArtAction(s, null, 'promote')).toBe(false);
    expect(canSmartArtAction(s, 1, 'promote')).toBe(true);
    expect(canSmartArtAction(s, 0, 'addAssistant')).toBe(true);
    // Assistants belong to organisation charts.
    expect(canSmartArtAction({ items: org, layout: 'process' }, 0, 'addAssistant')).toBe(false);
  });
});

// source: black-box Office text pane — Enter makes a new shape at the same level (splitting the
// line at the caret), Tab / Shift+Tab demote / promote, Backspace on an empty line deletes it.
describe('SmartArt text pane keys', () => {
  const list = [it_('Một', 0), it_('Hai', 0)];
  it('splits a line on Enter into a shape at the same level', () => {
    expect(paneKey(list, 0, 3, 'Enter')).toEqual({
      items: [it_('Một', 0), it_('', 0), it_('Hai', 0)],
      line: 1,
      caret: 0,
    });
    expect(paneKey(list, 0, 1, 'Enter')?.items.slice(0, 2)).toEqual([it_('M', 0), it_('ột', 0)]);
  });
  it('demotes and promotes with Tab / Shift+Tab', () => {
    const r = paneKey(list, 1, 0, 'Tab');
    expect(r?.items).toEqual([it_('Một', 0), it_('Hai', 1)]);
    expect(paneKey(r?.items ?? [], 1, 0, 'ShiftTab')?.items).toEqual(list);
  });
  it('deletes an empty line on Backspace and joins lines otherwise', () => {
    const withEmpty = [it_('Một', 0), it_('', 0), it_('Hai', 0)];
    expect(paneKey(withEmpty, 1, 0, 'Backspace')).toEqual({ items: list, line: 0, caret: 3 });
    expect(paneKey(list, 1, 0, 'Backspace')).toEqual({
      items: [it_('MộtHai', 0)],
      line: 0,
      caret: 3,
    });
    // Not at the start: the input deletes a character itself.
    expect(paneKey(list, 1, 2, 'Backspace')).toBeNull();
    expect(paneKey(list, 0, 3, 'Delete')?.items).toEqual([it_('MộtHai', 0)]);
  });
  it('sits left of the diagram, else right of it', () => {
    expect(panePosition({ left: 400, top: 100, width: 300, height: 200 }, 1200, 800).left).toBe(
      148,
    );
    expect(panePosition({ left: 50, top: 100, width: 300, height: 200 }, 1200, 800).left).toBe(362);
  });
});

describe('SmartArtSelection', () => {
  it('draws the frame and the current shape where they are, not in the way of the pointer', () => {
    wrap(
      <SmartArtSelection
        position="fixed"
        frame={{ left: 10, top: 20, width: 300, height: 120 }}
        node={{ left: 110, top: 50, width: 90, height: 40 }}
        nodeIndex={2}
      />,
    );
    const frame = screen.getByTestId('smartart-frame');
    expect(frame.style.position).toBe('fixed');
    expect([frame.style.left, frame.style.top, frame.style.width]).toEqual([
      '10px',
      '20px',
      '300px',
    ]);
    const node = screen.getByTestId('smartart-node-selection');
    expect(node.getAttribute('data-node')).toBe('2');
    expect([node.style.left, node.style.height]).toEqual(['110px', '40px']);
    expect(node.getAttribute('aria-hidden')).toBe('true');
  });

  it('marks no shape without a current one', () => {
    wrap(
      <SmartArtSelection
        frame={null}
        node={{ left: 0, top: 0, width: 1, height: 1 }}
        nodeIndex={null}
      />,
    );
    expect(screen.queryByTestId('smartart-node-selection')).toBeNull();
    expect(screen.queryByTestId('smartart-frame')).toBeNull();
  });
});

describe('SmartArtTextPane', () => {
  const labels = smartArtDesignLabels(t).pane;

  it('commits structural keys at once and typing after a pause', () => {
    vi.useFakeTimers();
    const onCommit = vi.fn();
    const onNode = vi.fn();
    wrap(
      <SmartArtTextPane
        items={list2()}
        node={null}
        onNode={onNode}
        onCommit={onCommit}
        onClose={() => undefined}
        anchor={() => null}
        labels={labels}
      />,
    );
    const first = screen.getByTestId('smartart-pane-line-0') as HTMLInputElement;
    first.focus();
    first.setSelectionRange(3, 3);
    fireEvent.keyDown(first, { key: 'Enter' });
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit.mock.calls[0]?.[0]).toEqual([it_('Một', 0), it_('', 0), it_('Hai', 0)]);
    const second = screen.getByTestId('smartart-pane-line-1') as HTMLInputElement;
    fireEvent.keyDown(second, { key: 'Tab' });
    expect(onCommit).toHaveBeenCalledTimes(2);
    expect(onCommit.mock.calls[1]?.[0]).toEqual([it_('Một', 0), it_('', 1), it_('Hai', 0)]);
    // Typing: one commit after the pause, not one per key.
    fireEvent.change(second, { target: { value: 'B' } });
    fireEvent.change(second, { target: { value: 'Ba' } });
    expect(onCommit).toHaveBeenCalledTimes(2);
    act(() => {
      vi.advanceTimersByTime(PANE_COMMIT_MS + 10);
    });
    expect(onCommit).toHaveBeenCalledTimes(3);
    expect(onCommit.mock.calls[2]?.[0]).toEqual([it_('Một', 0), it_('Ba', 1), it_('Hai', 0)]);
    expect(onNode).toHaveBeenCalledWith(1);
    vi.useRealTimers();
  });

  function list2() {
    return [it_('Một', 0), it_('Hai', 0)];
  }
});

describe('SmartArt Design tab', () => {
  const state: SmartArtState = {
    layout: 'hierarchy',
    items: org,
    colors: 'colorful',
    style: 'fill',
    rtl: false,
  };
  const api = (over: Partial<SmartArtToolsApi> = {}): SmartArtToolsApi => ({
    state: () => state,
    node: () => 1,
    setNode: vi.fn(),
    apply: vi.fn(),
    toShapes: vi.fn(),
    paneOpen: () => false,
    togglePane: vi.fn(),
    ...over,
  });

  it('registers one command per action, each applying one change', async () => {
    const reg = new CommandRegistry<{ on: boolean }>();
    const a = api();
    registerSmartArtTools(
      reg,
      () => a,
      (s) => s.on,
    );
    const on = { on: true };
    expect(reg.isEnabled(SMARTART_COMMANDS.action('addAssistant'), on)).toBe(true);
    expect(reg.isEnabled(SMARTART_COMMANDS.action('addAfter'), { on: false })).toBe(false);
    await reg.execute(SMARTART_COMMANDS.action('addBelow'), on);
    expect(a.apply).toHaveBeenCalledTimes(1);
    expect(a.apply).toHaveBeenLastCalledWith({
      items: [it_('A', 0), it_('B', 1), it_('C', 2), it_('', 2), it_('D', 1)],
    });
    expect(a.setNode).toHaveBeenLastCalledWith(3);
    await reg.execute(SMARTART_COMMANDS.rtl, on);
    expect(a.apply).toHaveBeenLastCalledWith({ rtl: true });
    await reg.execute(SMARTART_COMMANDS.reset, on);
    expect(a.apply).toHaveBeenLastCalledWith(SMARTART_RESET);
    // Convert to text only where the app has it (BlueSlide).
    expect(reg.isEnabled(SMARTART_COMMANDS.toText, on)).toBe(false);
  });

  it('lays out the groups and previews a layout while pointed at', () => {
    const preview = vi.fn();
    const a = api({ preview, loadPreviews: () => Promise.resolve([]) });
    const groups = smartArtDesignGroups({
      api: a,
      labels: smartArtDesignLabels(t).design,
      mk: (id, label) => ({ id, label, onSelect: () => undefined }),
      enabled: () => true,
    });
    expect(groups.map((g) => g.id)).toEqual([
      'smartartCreate',
      'smartartLayouts',
      'smartartStyles',
      'smartartReset',
    ]);
    expect(groups[0]?.items.map((i) => i.id)).toEqual([
      'smartart.addShape',
      'smartart.promote',
      'smartart.demote',
      'smartart.rtl',
      'smartart.up',
      'smartart.down',
      'smartart.textPane',
    ]);
    expect(groups[3]?.items.map((i) => i.id)).toEqual(['smartart.reset', 'smartart.toShapes']);
    wrap(<div>{groups[1]?.items[0]?.render}</div>);
    fireEvent.click(screen.getByTestId('smartart-layouts'));
    const cycle = screen.getByTestId('smartart-gallery-cycle');
    fireEvent.pointerEnter(cycle);
    expect(preview).toHaveBeenLastCalledWith({ layout: 'cycle' });
    fireEvent.click(cycle);
    expect(a.apply).toHaveBeenCalledWith({ layout: 'cycle' });
    expect(preview).toHaveBeenLastCalledWith(null);
  });
});
