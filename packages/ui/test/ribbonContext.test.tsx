// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  decideActiveTab,
  EMPTY_TRACKER,
  getRibbonPrefs,
  Ribbon,
  setRibbonPrefs,
  simplifiedItems,
  type RibbonContext,
  type RibbonGroup,
  type RibbonTab,
} from '../src';

afterEach(cleanup);
beforeEach(() => setRibbonPrefs({ simplified: false, collapsed: false }));

const noop = () => {};
const group = (id: string, n = 2): RibbonGroup => ({
  id,
  label: `G ${id}`,
  items: Array.from({ length: n }, (_, i) => ({
    id: `${id}.c${i}`,
    label: `${id} ${i}`,
    onSelect: noop,
  })),
});
const home: RibbonTab = { id: 'home', label: 'Home', groups: [group('home')] };
const insert: RibbonTab = { id: 'insert', label: 'Insert', groups: [group('insert')] };
const ctx = (id: string, context: RibbonContext): RibbonTab => ({
  id,
  label: id,
  context,
  groups: [group(id)],
});
const labels = {
  more: 'More',
  contexts: { table: 'Table tools', picture: 'Picture tools' },
  simplified: 'Simplified ribbon',
  collapse: 'Collapse',
  expand: 'Expand',
};

/** Ribbon with the tab state kept by a parent, as the editors do. */
function Host({ table, picture }: { table?: boolean; picture?: boolean }) {
  const [tab, setTab] = useState('home');
  const tabs = [
    home,
    insert,
    ...(table ? [ctx('tableDesign', 'table'), ctx('tableLayout', 'table')] : []),
    ...(picture ? [ctx('pictureFormat', 'picture')] : []),
  ];
  return (
    <>
      <Ribbon tabs={tabs} value={tab} onValueChange={setTab} labels={labels} />
      <output data-testid="tab">{tab}</output>
    </>
  );
}

const selected = () =>
  screen.getAllByRole('tab').find((t) => t.getAttribute('aria-selected') === 'true')?.textContent;

describe('decideActiveTab', () => {
  const tabs = [home, insert, ctx('tableDesign', 'table')];
  it('activates a context once when it appears, keeps the user choice after', () => {
    const first = decideActiveTab(tabs, 'insert', EMPTY_TRACKER);
    expect(first.value).toBe('tableDesign');
    expect(first.activated).toBe(true);
    expect(first.tracker).toEqual({ present: ['table'], lastPlain: 'insert' });
    // Same selection, user went back to Home: no second activation.
    const again = decideActiveTab(tabs, 'home', first.tracker);
    expect(again.value).toBe('home');
    expect(again.activated).toBe(false);
  });
  it('only shows the contexts left out of autoActivate, and keeps a tab opened on its context', () => {
    const shown = decideActiveTab(tabs, 'home', EMPTY_TRACKER, ['picture']);
    expect(shown.value).toBe('home');
    expect(shown.tracker.present).toEqual(['table']);
    expect(decideActiveTab(tabs, 'tableDesign', EMPTY_TRACKER).value).toBe('tableDesign');
  });
  it('falls back to the last regular tab when the context goes away', () => {
    const t1 = decideActiveTab(tabs, 'insert', EMPTY_TRACKER).tracker;
    const gone = decideActiveTab([home, insert], 'tableDesign', t1);
    expect(gone.value).toBe('insert');
    expect(gone.tracker.present).toEqual([]);
    // Unknown tab and nothing remembered: the first tab.
    expect(decideActiveTab([home], 'x', EMPTY_TRACKER).value).toBe('home');
  });
});

describe('Ribbon contextual tabs', () => {
  it('shows, auto-activates and hides contextual tabs with their header label', () => {
    const { rerender, container } = render(<Host />);
    expect(screen.queryByRole('tab', { name: 'tableDesign' })).toBeNull();
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Insert' }));
    expect(selected()).toBe('Insert');

    rerender(<Host table />);
    // Grouped after the regular tabs under the "Table tools" header.
    const band = container.querySelector('[data-context-group="table"]')!;
    expect(band.textContent).toContain('Table tools');
    expect(band.querySelectorAll('[role="tab"]')).toHaveLength(2);
    expect(selected()).toBe('tableDesign');
    expect(screen.getByTestId('tab').textContent).toBe('tableDesign');
    expect(container.querySelector('[data-group="tableDesign"]')).not.toBeNull();

    // The user goes back to Home while the table stays selected: kept.
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Home' }));
    rerender(<Host table />);
    expect(selected()).toBe('Home');

    // A second context appears: it activates; the first one stays visible.
    rerender(<Host table picture />);
    expect(selected()).toBe('pictureFormat');
    expect(container.querySelector('[data-context-group="picture"]')).not.toBeNull();

    // Selection gone: tabs hidden, back to the last regular tab.
    rerender(<Host />);
    expect(screen.queryByRole('tab', { name: 'pictureFormat' })).toBeNull();
    expect(container.querySelector('[data-context-group]')).toBeNull();
    expect(selected()).toBe('Home');
    expect(screen.getByTestId('tab').textContent).toBe('home');

    // Selected again: a new appearance activates again.
    rerender(<Host table />);
    expect(selected()).toBe('tableDesign');
  });

  it('works uncontrolled and keeps contextual tabs reachable by keyboard', () => {
    const tabs = (table: boolean) => [
      home,
      insert,
      ...(table ? [ctx('tableDesign', 'table')] : []),
    ];
    const { rerender } = render(<Ribbon tabs={tabs(false)} labels={labels} />);
    expect(selected()).toBe('Home');
    rerender(<Ribbon tabs={tabs(true)} labels={labels} />);
    expect(selected()).toBe('tableDesign');
    const all = screen.getAllByRole('tab');
    expect(all.map((t) => t.textContent)).toEqual(['Home', 'Insert', 'tableDesign']);
    // One tab list: arrow keys move through regular and contextual tabs alike.
    expect(screen.getAllByRole('tablist')).toHaveLength(1);
    expect(all[2]!.closest('[role="tablist"]')).toBe(all[0]!.closest('[role="tablist"]'));
    rerender(<Ribbon tabs={tabs(false)} labels={labels} />);
    expect(selected()).toBe('Home');
  });
});

describe('simplified and collapsed ribbon', () => {
  it('keeps the flagged (else first) commands of each group in one row, the rest in "…"', () => {
    const g = group('font', 5);
    expect(simplifiedItems(g).shown.map((i) => i.id)).toEqual(['font.c0', 'font.c1', 'font.c2']);
    const flagged: RibbonGroup = {
      ...g,
      items: g.items.map((i, n) => ({ ...i, simplified: n === 4 })),
    };
    expect(simplifiedItems(flagged).shown.map((i) => i.id)).toEqual(['font.c4']);
    expect(simplifiedItems(flagged).rest).toHaveLength(4);

    const tab: RibbonTab = { id: 'home', label: 'Home', groups: [g] };
    const { container } = render(<Ribbon tabs={[tab]} labels={labels} />);
    expect(container.querySelector('[data-simplified]')).toBeNull();
    fireEvent.click(screen.getByTestId('ribbon-simplified'));
    expect(getRibbonPrefs().simplified).toBe(true);
    expect(screen.getByTestId('ribbon-simplified').getAttribute('aria-pressed')).toBe('true');
    const panel = container.querySelector('[role="tabpanel"]')!;
    expect(panel.getAttribute('data-simplified')).toBe('true');
    expect(panel.querySelectorAll('button[data-command]')).toHaveLength(3);
    // The other commands are in the group's "…" menu.
    fireEvent.click(screen.getByTestId('ribbon-group-more-font'));
    const menu = screen.getByTestId('ribbon-group-more-font-panel');
    expect(menu.querySelectorAll('button[data-command]')).toHaveLength(2);
  });

  it('remembers the mode and collapses / expands with Ctrl+F1', () => {
    let ran = 0;
    const tab: RibbonTab = {
      id: 'home',
      label: 'Home',
      groups: [
        { id: 'g', label: 'G', items: [{ id: 'x.run', label: 'Run', onSelect: () => (ran += 1) }] },
      ],
    };
    setRibbonPrefs({ simplified: true });
    expect(JSON.parse(localStorage.getItem('bo.ribbon')!)).toEqual({
      simplified: true,
      collapsed: false,
    });
    const { container } = render(<Ribbon tabs={[tab]} labels={labels} />);
    expect(container.querySelector('[role="tabpanel"]')).not.toBeNull();
    act(() => {
      fireEvent.keyDown(window, { key: 'F1', ctrlKey: true });
    });
    expect(getRibbonPrefs().collapsed).toBe(true);
    expect(container.querySelector('[role="tabpanel"]')).toBeNull();
    expect(screen.getByTestId('ribbon-collapse').getAttribute('aria-label')).toBe('Expand');
    // Clicking a tab shows its commands over the document; running one closes it.
    const homeTab = screen.getByRole('tab', { name: 'Home' });
    fireEvent.mouseDown(homeTab);
    fireEvent.click(homeTab);
    expect(screen.getByTestId('ribbon-peek')).toBeTruthy();
    fireEvent.click(container.querySelector('button[data-command="x.run"]')!);
    expect(ran).toBe(1);
    expect(screen.queryByTestId('ribbon-peek')).toBeNull();
    act(() => {
      fireEvent.keyDown(window, { key: 'F1', ctrlKey: true });
    });
    expect(getRibbonPrefs().collapsed).toBe(false);
    expect(container.querySelector('[role="tabpanel"]')).not.toBeNull();
  });
});
