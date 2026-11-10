// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  CONTEXT_MENUS,
  ContextMenu,
  DOC_MENUS,
  SHEET_MENUS,
  SLIDE_MENUS,
  isContextMenuKey,
  isEntry,
  resolveMenu,
  ribbonMenuLookup,
  type MenuEntry,
  type MenuLine,
  type RibbonTab,
} from '../src';

afterEach(cleanup);

const entry = (id: string, enabled = true): MenuEntry => ({
  id,
  label: id,
  enabled,
  onSelect: vi.fn(),
});
const all = (id: string) => entry(id);
const ids = (lines: MenuLine[]) => lines.map((l) => (isEntry(l) ? l.id : '-'));

describe('context menu specs (CORE-UI-003)', () => {
  it('drops unknown commands and collapses separators', () => {
    const known = new Set(['a', 'c']);
    const lines = resolveMenu(['-', 'a', 'x', '-', '-', 'b', '-', 'c', '-'], (id) =>
      known.has(id) ? entry(id) : undefined,
    );
    expect(ids(lines)).toEqual(['a', '-', 'c']);
  });

  it('offers the commands of what is under the pointer, per app', () => {
    expect(ids(resolveMenu(DOC_MENUS.text, all)).slice(0, 3)).toEqual([
      'edit.cut',
      'edit.copy',
      'edit.paste',
    ]);
    expect(ids(resolveMenu(DOC_MENUS.table, all))).toContain('table.insertRowAbove');
    expect(ids(resolveMenu(DOC_MENUS.table, all))).toContain('table.mergeCells');
    expect(ids(resolveMenu(DOC_MENUS.picture, all))).toContain('picture.change');
    expect(ids(resolveMenu(DOC_MENUS.header, all))).toContain('hf.close');
    expect(ids(resolveMenu(DOC_MENUS.comment, all))).toEqual(
      expect.arrayContaining(['review.deleteComment', 'review.resolveComment']),
    );
    expect(ids(resolveMenu(DOC_MENUS.comment, all))).not.toContain('edit.paste');
    expect(ids(resolveMenu(SHEET_MENUS.rowHeader, all))).toContain('cells.insertRow');
    expect(ids(resolveMenu(SHEET_MENUS.rowHeader, all))).not.toContain('cells.insertCol');
    expect(ids(resolveMenu(SHEET_MENUS.columnHeader, all))).toContain('cells.hideCols');
    expect(ids(resolveMenu(SHEET_MENUS.sheetTab, all))).toEqual(
      expect.arrayContaining(['sheet.rename', 'sheet.delete', 'sheet.tabColor']),
    );
    expect(ids(resolveMenu(SHEET_MENUS.chart, all))).toContain('chart.edit');
    expect(ids(resolveMenu(SLIDE_MENUS.thumbnail, all))).toContain('slide.duplicate');
    expect(ids(resolveMenu(SLIDE_MENUS.shape, all))).toContain('arrange.front');
    expect(ids(resolveMenu(SLIDE_MENUS.table, all))).toContain('table.insertColLeft');
    expect(ids(resolveMenu(SLIDE_MENUS.canvas, all))).not.toContain('edit.cut');
  });

  it('every menu of every app has commands and no duplicate', () => {
    for (const menus of Object.values(CONTEXT_MENUS))
      for (const spec of Object.values(menus)) {
        const list = spec.filter((x) => x !== '-');
        expect(list.length).toBeGreaterThan(2);
        expect(new Set(list).size).toBe(list.length);
      }
  });

  it('resolves labels, state and shortcuts from the ribbon, then the fallback', () => {
    const tabs: RibbonTab[] = [
      {
        id: 'home',
        label: 'Home',
        groups: [
          {
            id: 'clip',
            label: 'Clipboard',
            items: [
              { id: 'edit.copy', label: 'Sao chép', shortcut: 'Ctrl+C', onSelect: vi.fn() },
              { id: 'edit.paste', label: 'Dán', enabled: false, onSelect: vi.fn() },
              { id: 'font', label: 'font', render: 'x', onSelect: vi.fn() },
            ],
          },
        ],
      },
    ];
    const fallback = vi.fn((id: string) => (id === 'edit.cut' ? entry('edit.cut') : undefined));
    const look = ribbonMenuLookup(tabs, fallback);
    expect(look('edit.copy')).toMatchObject({ label: 'Sao chép', shortcut: 'Ctrl+C' });
    expect(look('edit.paste')?.enabled).toBe(false);
    expect(look('font')).toBeUndefined();
    expect(look('edit.cut')?.id).toBe('edit.cut');
  });
});

describe('ContextMenu', () => {
  const lines = (): MenuLine[] => [
    entry('a'),
    entry('b', false),
    { separator: true },
    { ...entry('c'), checked: true, shortcut: 'Ctrl+K' },
  ];

  it('is a menu of items, disabled ones marked', () => {
    render(<ContextMenu x={10} y={10} items={lines()} label="Menu" onClose={vi.fn()} />);
    const menu = screen.getByRole('menu', { name: 'Menu' });
    expect(menu).toBeTruthy();
    expect(screen.getAllByRole('menuitem')).toHaveLength(2);
    expect(screen.getByRole('menuitemcheckbox').getAttribute('aria-checked')).toBe('true');
    expect(screen.getByRole('menuitem', { name: 'b' }).getAttribute('aria-disabled')).toBe('true');
    expect(screen.getByRole('separator')).toBeTruthy();
  });

  it('arrows skip disabled items, Enter runs, Escape closes', () => {
    const items = lines();
    const onClose = vi.fn();
    render(<ContextMenu x={10} y={10} items={items} label="Menu" onClose={onClose} />);
    const menu = screen.getByRole('menu');
    expect(document.activeElement?.getAttribute('data-menu-command')).toBe('a');
    fireEvent.keyDown(menu, { key: 'ArrowDown' });
    expect(document.activeElement?.getAttribute('data-menu-command')).toBe('c');
    fireEvent.keyDown(menu, { key: 'ArrowDown' });
    expect(document.activeElement?.getAttribute('data-menu-command')).toBe('a');
    fireEvent.keyDown(menu, { key: 'End' });
    fireEvent.keyDown(menu, { key: 'Enter' });
    expect((items[3] as MenuEntry).onSelect).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(menu, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('a disabled item does not run; a click outside closes', () => {
    const items = lines();
    const onClose = vi.fn();
    render(<ContextMenu x={10} y={10} items={items} label="Menu" onClose={onClose} />);
    fireEvent.click(screen.getByRole('menuitem', { name: 'b' }));
    expect((items[1] as MenuEntry).onSelect).not.toHaveBeenCalled();
    fireEvent.mouseDown(document.body);
    expect(onClose).toHaveBeenCalled();
  });

  it('Shift+F10 and the context-menu key open menus', () => {
    const k = { ctrlKey: false, altKey: false, metaKey: false };
    expect(isContextMenuKey({ key: 'F10', shiftKey: true, ...k })).toBe(true);
    expect(isContextMenuKey({ key: 'ContextMenu', shiftKey: false, ...k })).toBe(true);
    expect(isContextMenuKey({ key: 'F10', shiftKey: false, ...k })).toBe(false);
  });

  it('opens a submenu by pointer or arrows; its commands close the whole menu', () => {
    const pick = vi.fn();
    const onClose = vi.fn();
    const items: MenuLine[] = [
      entry('edit.copy'),
      {
        id: 'more',
        label: 'More',
        submenu: [{ heading: 'Words' }, { id: 'w.glad', label: 'glad', onSelect: pick }],
        submenuTestId: 'sub',
        onSelect: vi.fn(),
      },
    ];
    render(<ContextMenu x={10} y={10} items={items} label="Menu" onClose={onClose} />);
    const more = screen.getByText('More').closest('button')!;
    expect(more.getAttribute('aria-haspopup')).toBe('menu');
    expect(more.getAttribute('aria-expanded')).toBe('false');
    fireEvent.mouseEnter(more);
    expect(screen.getByTestId('sub')).toBeTruthy();
    expect(more.getAttribute('aria-expanded')).toBe('true');
    // Back with ArrowLeft (the parent menu stays open), in again with ArrowRight.
    fireEvent.keyDown(window, { key: 'ArrowLeft' });
    expect(screen.queryByTestId('sub')).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.keyDown(window, { key: 'ArrowRight' });
    expect(screen.getByTestId('sub')).toBeTruthy();
    // A click inside the submenu is not a click elsewhere.
    fireEvent.mouseDown(screen.getByText('glad'));
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.keyDown(window, { key: 'Enter' });
    expect(pick).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
