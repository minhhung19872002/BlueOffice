// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  STATUS_ITEMS,
  StatusBar,
  StatusButtons,
  StatusZoom,
  isStatusItemShown,
  loadStatusPrefs,
  sanitizeStatusPrefs,
  saveStatusPrefs,
  setStatusItem,
  shownStatusItems,
  statusBarKey,
  useStatusBarPrefs,
  type SettingStore,
  type StatusBarApp,
} from '../src';

afterEach(cleanup);
beforeEach(() => localStorage.clear());

describe('status bar items', () => {
  it('each app has its own items with the expected defaults', () => {
    const ids = (app: StatusBarApp) => STATUS_ITEMS[app].map((d) => d.id);
    expect(ids('doc')).toEqual(
      expect.arrayContaining([
        'page',
        'words',
        'proofing',
        'language',
        'trackChanges',
        'capsLock',
        'overtype',
        'viewButtons',
        'zoomSlider',
        'zoom',
      ]),
    );
    expect(ids('sheet')).toEqual(
      expect.arrayContaining([
        'cellMode',
        'average',
        'count',
        'numericalCount',
        'min',
        'max',
        'sum',
        'macro',
        'filter',
        'viewButtons',
        'zoom',
      ]),
    );
    expect(ids('slide')).toEqual(
      expect.arrayContaining([
        'slide',
        'language',
        'notes',
        'comments',
        'viewButtons',
        'zoom',
        'fit',
      ]),
    );
    for (const app of ['doc', 'sheet', 'slide'] as const)
      expect(new Set(ids(app)).size).toBe(ids(app).length);
    // Defaults: sum on, minimum / maximum / numerical count off; Caps Lock and overtype off.
    expect(shownStatusItems('sheet', {})).toEqual(
      expect.arrayContaining(['cellMode', 'average', 'count', 'sum']),
    );
    expect(isStatusItemShown('sheet', {}, 'min')).toBe(false);
    expect(isStatusItemShown('doc', {}, 'overtype')).toBe(false);
    expect(isStatusItemShown('doc', {}, 'nope')).toBe(false);
  });

  it('stores only the differences from the defaults', () => {
    let p = setStatusItem('sheet', {}, 'min');
    expect(p).toEqual({ shown: ['min'] });
    p = setStatusItem('sheet', p, 'sum');
    expect(p).toEqual({ shown: ['min'], hidden: ['sum'] });
    expect(isStatusItemShown('sheet', p, 'min')).toBe(true);
    expect(isStatusItemShown('sheet', p, 'sum')).toBe(false);
    // Back to the defaults: nothing left to store.
    p = setStatusItem('sheet', p, 'min');
    p = setStatusItem('sheet', p, 'sum', true);
    expect(p).toEqual({});
    // Setting an item to the value it has, or an unknown item, changes nothing.
    expect(setStatusItem('sheet', {}, 'sum', true)).toEqual({});
    expect(setStatusItem('sheet', { shown: ['max'] }, 'x')).toEqual({ shown: ['max'] });
  });

  it('sanitizes stored values', () => {
    expect(sanitizeStatusPrefs('doc', null)).toEqual({});
    expect(sanitizeStatusPrefs('doc', 'x')).toEqual({});
    expect(
      sanitizeStatusPrefs('doc', {
        // `page` is on by default (cannot be in `shown`), `sum` belongs to another app.
        shown: ['overtype', 'overtype', 'page', 'sum', 3],
        hidden: ['words', 'capsLock'],
      }),
    ).toEqual({ shown: ['overtype'], hidden: ['words'] });
  });
});

describe('status bar persistence', () => {
  it('uses localStorage per app when the platform has no setting store', async () => {
    await saveStatusPrefs({}, 'doc', { hidden: ['words'] });
    await saveStatusPrefs({}, 'sheet', { shown: ['max'] });
    expect(localStorage.getItem(statusBarKey('doc'))).toBe('{"hidden":["words"]}');
    expect(await loadStatusPrefs({}, 'doc')).toEqual({ hidden: ['words'] });
    expect(await loadStatusPrefs({}, 'sheet')).toEqual({ shown: ['max'] });
    expect(await loadStatusPrefs({}, 'slide')).toEqual({});
    // Defaults again: the key is removed.
    await saveStatusPrefs({}, 'doc', {});
    expect(localStorage.getItem(statusBarKey('doc'))).toBeNull();
    localStorage.setItem(statusBarKey('doc'), '{broken');
    expect(await loadStatusPrefs({}, 'doc')).toEqual({});
  });

  it('uses the platform setting store (desktop), like the ribbon customisation', async () => {
    const data = new Map<string, string>();
    const store: SettingStore = {
      getSetting: async (k) => data.get(k) ?? null,
      setSetting: async (k, v) => {
        if (v === null) data.delete(k);
        else data.set(k, v);
      },
    };
    await saveStatusPrefs(store, 'slide', { hidden: ['fit'] });
    expect(data.get('blueoffice.statusbar.slide')).toBe('{"hidden":["fit"]}');
    expect(localStorage.length).toBe(0);
    expect(await loadStatusPrefs(store, 'slide')).toEqual({ hidden: ['fit'] });
    await saveStatusPrefs(store, 'slide', {});
    expect(data.size).toBe(0);
  });
});

function Bar({ store }: { store: SettingStore }) {
  const sb = useStatusBarPrefs(store, 'sheet');
  return (
    <StatusBar
      customize={{
        label: 'Customize status bar',
        items: STATUS_ITEMS.sheet.map((d) => ({
          id: d.id,
          label: d.id,
          checked: sb.shown(d.id),
          ...(d.id === 'sum' ? { value: '120' } : {}),
        })),
        onToggle: sb.toggle,
        resetLabel: 'Reset',
        onReset: sb.reset,
      }}
      left={<button type="button">Ready</button>}
      right={
        <>
          {sb.shown('sum') && <span data-testid="sum">Sum: 120</span>}
          {sb.shown('max') && <span data-testid="max">Max: 50</span>}
        </>
      }
    />
  );
}

describe('StatusBar customisation', () => {
  it('right click opens a checklist; choices apply at once and are remembered', async () => {
    const { unmount } = render(<Bar store={{}} />);
    expect(screen.getByTestId('sum')).toBeTruthy();
    expect(screen.queryByTestId('max')).toBeNull();
    fireEvent.contextMenu(screen.getByTestId('status-bar'), { clientX: 300, clientY: 700 });
    const menu = screen.getByRole('menu', { name: 'Customize status bar' });
    expect(menu).toBeTruthy();
    const sum = screen.getByTestId('status-item-sum');
    expect(sum.getAttribute('role')).toBe('menuitemcheckbox');
    expect(sum.getAttribute('aria-checked')).toBe('true');
    expect(sum.textContent).toContain('120');
    expect(screen.getByTestId('status-item-max').getAttribute('aria-checked')).toBe('false');
    // The checklist stays open while items are switched.
    fireEvent.click(sum);
    fireEvent.click(screen.getByTestId('status-item-max'));
    expect(screen.getByRole('menu')).toBeTruthy();
    expect(screen.queryByTestId('sum')).toBeNull();
    expect(screen.getByTestId('max')).toBeTruthy();
    expect(screen.getByTestId('status-item-sum').getAttribute('aria-checked')).toBe('false');
    await waitFor(() =>
      expect(JSON.parse(localStorage.getItem('blueoffice.statusbar.sheet') ?? '{}')).toEqual({
        shown: ['max'],
        hidden: ['sum'],
      }),
    );
    fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
    expect(screen.queryByRole('menu')).toBeNull();

    // "Reload": a new bar reads the stored choices.
    unmount();
    render(<Bar store={{}} />);
    await waitFor(() => expect(screen.getByTestId('max')).toBeTruthy());
    expect(screen.queryByTestId('sum')).toBeNull();
    // Reset brings the defaults back and clears the stored value.
    fireEvent.contextMenu(screen.getByTestId('status-bar'));
    fireEvent.click(screen.getByTestId('status-item-reset'));
    expect(screen.getByTestId('sum')).toBeTruthy();
    expect(screen.queryByTestId('max')).toBeNull();
    await waitFor(() => expect(localStorage.getItem('blueoffice.statusbar.sheet')).toBeNull());
  });

  it('Shift+F10 and the Menu key open it; arrows and Space work the checklist', () => {
    render(<Bar store={{}} />);
    const ready = screen.getByRole('button', { name: 'Ready' });
    ready.focus();
    fireEvent.keyDown(ready, { key: 'F10', shiftKey: true });
    const menu = screen.getByRole('menu');
    expect(document.activeElement).toBe(menu);
    // The first item is active; End goes to "Reset", Home back, ArrowDown to the second.
    const first = STATUS_ITEMS.sheet[0]!.id;
    const second = STATUS_ITEMS.sheet[1]!.id;
    expect(screen.getByTestId(`status-item-${first}`).dataset.active).toBe('true');
    fireEvent.keyDown(menu, { key: 'End' });
    expect(screen.getByTestId('status-item-reset').dataset.active).toBe('true');
    fireEvent.keyDown(menu, { key: 'Home' });
    fireEvent.keyDown(menu, { key: 'ArrowDown' });
    expect(screen.getByTestId(`status-item-${second}`).dataset.active).toBe('true');
    const before = screen.getByTestId(`status-item-${second}`).getAttribute('aria-checked');
    fireEvent.keyDown(menu, { key: ' ' });
    expect(screen.getByTestId(`status-item-${second}`).getAttribute('aria-checked')).not.toBe(
      before,
    );
    // Escape closes and gives the focus back to the bar.
    fireEvent.keyDown(menu, { key: 'Escape' });
    expect(screen.queryByRole('menu')).toBeNull();
    expect(document.activeElement).toBe(screen.getByTestId('status-bar'));
    fireEvent.keyDown(screen.getByTestId('status-bar'), { key: 'ContextMenu' });
    expect(screen.getByRole('menu')).toBeTruthy();
    // A click elsewhere closes it.
    act(() => {
      fireEvent.mouseDown(document.body);
    });
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('without customisation there is no checklist', () => {
    render(<StatusBar left={<span>x</span>} />);
    fireEvent.contextMenu(screen.getByTestId('status-bar'));
    expect(screen.queryByRole('menu')).toBeNull();
  });
});

describe('status bar controls', () => {
  const labels = { slider: 'Zoom', zoomIn: 'Zoom in', zoomOut: 'Zoom out', percent: 'Zoom 100%' };

  it('zoom slider, − / + and the percentage', () => {
    const onZoom = vi.fn();
    const onPercent = vi.fn();
    render(
      <StatusZoom
        zoom={1}
        min={0.5}
        max={2}
        onZoom={onZoom}
        onPercent={onPercent}
        labels={labels}
      />,
    );
    const slider = screen.getByRole('slider', { name: 'Zoom' }) as HTMLInputElement;
    expect(slider.value).toBe('100');
    expect(slider.min).toBe('50');
    expect(slider.max).toBe('200');
    fireEvent.change(slider, { target: { value: '150' } });
    expect(onZoom).toHaveBeenLastCalledWith(1.5);
    fireEvent.click(screen.getByRole('button', { name: 'Zoom in' }));
    expect(onZoom).toHaveBeenLastCalledWith(1.1);
    fireEvent.click(screen.getByRole('button', { name: 'Zoom out' }));
    expect(onZoom).toHaveBeenLastCalledWith(0.9);
    expect(screen.getByTestId('status-zoom').textContent).toBe('100%');
    fireEvent.click(screen.getByTestId('status-zoom'));
    expect(onPercent).toHaveBeenCalled();
  });

  it('zoom stops at the ends; the parts can be hidden separately', () => {
    const onZoom = vi.fn();
    const { rerender } = render(
      <StatusZoom zoom={2} min={0.5} max={2} onZoom={onZoom} labels={labels} />,
    );
    expect((screen.getByRole('button', { name: 'Zoom in' }) as HTMLButtonElement).disabled).toBe(
      true,
    );
    rerender(
      <StatusZoom zoom={2} min={0.5} max={2} onZoom={onZoom} labels={labels} slider={false} />,
    );
    expect(screen.queryByRole('slider')).toBeNull();
    expect(screen.getByTestId('status-zoom').textContent).toBe('200%');
    rerender(
      <StatusZoom zoom={2} min={0.5} max={2} onZoom={onZoom} labels={labels} percent={false} />,
    );
    expect(screen.queryByTestId('status-zoom')).toBeNull();
    expect(screen.getByRole('slider')).toBeTruthy();
  });

  it('view buttons are a labelled group of toggle buttons', () => {
    const onClick = vi.fn();
    render(
      <StatusButtons
        label="Views"
        buttons={[
          { id: 'normal', label: 'Normal', pressed: true, onClick },
          { id: 'layout', label: 'Page layout', pressed: false, onClick },
        ]}
      />,
    );
    expect(screen.getByRole('group', { name: 'Views' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Normal' }).getAttribute('aria-pressed')).toBe(
      'true',
    );
    fireEvent.click(screen.getByRole('button', { name: 'Page layout' }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('exclusive buttons (view modes) are a radio group with one checked', () => {
    const onClick = vi.fn();
    render(
      <StatusButtons
        label="Views"
        exclusive
        buttons={[
          { id: 'normal', label: 'Normal', pressed: true, onClick },
          { id: 'layout', label: 'Page layout', pressed: false, onClick },
        ]}
      />,
    );
    expect(screen.getByRole('radiogroup', { name: 'Views' })).toBeTruthy();
    expect(screen.queryByRole('button')).toBeNull();
    expect(screen.getByRole('radio', { name: 'Normal' }).getAttribute('aria-checked')).toBe('true');
    const other = screen.getByRole('radio', { name: 'Page layout' });
    expect(other.getAttribute('aria-checked')).toBe('false');
    fireEvent.click(other);
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
