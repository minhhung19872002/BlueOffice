// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  SPECIAL_CHARACTERS,
  SymbolPicker,
  VIETNAMESE_BLOCK,
  foldText,
  loadRecentSymbols,
  moveSymbolIndex,
  pushRecentSymbol,
  searchSymbols,
  symbolBlocks,
  symbolEntry,
  type SpecialCharacterId,
  type SymbolPickerLabels,
} from '../src';
import { VI_KEYWORDS } from '../src/symbols/keywords';

afterEach(cleanup);
beforeEach(() => localStorage.clear());

const labels: SymbolPickerLabels = {
  title: 'Symbol',
  symbolsTab: 'Symbols',
  specialTab: 'Special',
  search: 'Search',
  block: 'Group',
  font: 'Font',
  recent: 'Recent',
  noRecent: 'None yet',
  noResults: 'Nothing found',
  grid: 'Characters',
  insert: 'Insert',
  close: 'Close',
  blockName: (_id, fallback) => fallback,
  special: Object.fromEntries(SPECIAL_CHARACTERS.map((s) => [s.id, `label ${s.id}`])) as Record<
    SpecialCharacterId,
    string
  >,
};

const chars = (q: string) => searchSymbols(q).map((e) => e.char);

describe('symbol catalog', () => {
  it('decodes UCD names', () => {
    expect(symbolEntry(0x20ab)?.name).toBe('DONG SIGN');
    expect(symbolEntry(0x1ea1)?.name).toBe('LATIN SMALL LETTER A WITH DOT BELOW');
    expect(symbolEntry(0x2192)?.name).toBe('RIGHTWARDS ARROW');
    // Controls are not offered.
    expect(symbolEntry(0x85)).toBeUndefined();
  });

  it('lists the Vietnamese letters first', () => {
    const vi = symbolBlocks()[0];
    expect(vi?.id).toBe(VIETNAMESE_BLOCK);
    const letters = vi?.entries.map((e) => e.char).join('') ?? '';
    expect(letters).toContain('ạ');
    expect(letters).toContain('Ữ');
    expect(letters).toContain('đ');
    expect(vi?.entries).toHaveLength(12 * 6 * 2 + 2);
  });

  it('finds by English name', () => {
    expect(chars('dong sign')[0]).toBe('₫');
    expect(chars('DONG')).toContain('₫');
    expect(chars('rightwards arrow')[0]).toBe('→');
    expect(chars('greek small letter alpha')[0]).toBe('α');
  });

  it('finds by code point', () => {
    expect(chars('U+20AB')).toEqual(['₫']);
    expect(chars('u+20ab')).toEqual(['₫']);
    expect(chars('20ab')[0]).toBe('₫');
    expect(chars('0x2192')[0]).toBe('→');
  });

  it('finds by the character itself', () => {
    expect(chars('€')[0]).toBe('€');
  });

  it('finds by Vietnamese keyword, with or without diacritics', () => {
    expect(chars('đồng')[0]).toBe('₫');
    expect(chars('mũi tên')).toContain('→');
    expect(chars('mui ten')).toContain('⇒');
    expect(chars('vô cực')).toContain('∞');
    expect(chars('hy lạp')).toContain('Ω');
    expect(chars('tiếng việt')).toContain('ỗ');
    expect(foldText('Mũi Tên  Đỏ')).toBe('mui ten do');
  });

  it('keyword characters all exist in the catalog', () => {
    const ids = new Set(symbolBlocks().map((b) => b.id));
    for (const k of VI_KEYWORDS) {
      for (const c of k.chars ?? '') expect(symbolEntry(c.codePointAt(0) ?? 0), c).toBeDefined();
      for (const b of k.blocks ?? []) expect(ids.has(b), b).toBe(true);
    }
  });

  it('keeps a recent list, newest first, without duplicates', () => {
    expect(loadRecentSymbols()).toEqual([]);
    pushRecentSymbol('₫');
    pushRecentSymbol('→');
    expect(pushRecentSymbol('₫')).toEqual(['₫', '→']);
    expect(loadRecentSymbols()).toEqual(['₫', '→']);
    localStorage.setItem('bo.symbols.recent', 'not json');
    expect(loadRecentSymbols()).toEqual([]);
  });

  it('moves the grid index with navigation keys', () => {
    // 40 cells, 16 per row.
    expect(moveSymbolIndex('ArrowRight', 0, 40, 16)).toBe(1);
    expect(moveSymbolIndex('ArrowLeft', 0, 40, 16)).toBe(0);
    expect(moveSymbolIndex('ArrowDown', 3, 40, 16)).toBe(19);
    expect(moveSymbolIndex('ArrowDown', 30, 40, 16)).toBe(30);
    expect(moveSymbolIndex('ArrowUp', 19, 40, 16)).toBe(3);
    expect(moveSymbolIndex('Home', 19, 40, 16)).toBe(16);
    expect(moveSymbolIndex('End', 17, 40, 16)).toBe(31);
    expect(moveSymbolIndex('End', 33, 40, 16)).toBe(39);
    expect(moveSymbolIndex('Home', 33, 40, 16, true)).toBe(0);
    expect(moveSymbolIndex('End', 3, 40, 16, true)).toBe(39);
    expect(moveSymbolIndex('PageDown', 0, 200, 16)).toBe(96);
    expect(moveSymbolIndex('PageUp', 20, 200, 16)).toBe(0);
    expect(moveSymbolIndex('a', 0, 40, 16)).toBeNull();
  });
});

describe('SymbolPicker', () => {
  it('inserts the searched character with Enter and remembers it', () => {
    const onInsert = vi.fn();
    render(<SymbolPicker labels={labels} onInsert={onInsert} fonts={['Arial']} font="Arial" />);
    const search = screen.getByTestId('symbol-search');
    fireEvent.change(search, { target: { value: 'U+20AB' } });
    expect(screen.getByTestId('symbol-info').textContent).toContain('DONG SIGN');
    fireEvent.keyDown(search, { key: 'Enter' });
    expect(onInsert).toHaveBeenCalledWith('₫', 'Arial');
    expect(screen.getAllByTestId('symbol-recent').map((b) => b.textContent)).toEqual(['₫']);
    expect(loadRecentSymbols()).toEqual(['₫']);
  });

  it('navigates the grid with the keyboard (ARIA grid, roving focus)', () => {
    const onInsert = vi.fn();
    render(<SymbolPicker labels={labels} onInsert={onInsert} columns={8} />);
    const grid = screen.getByRole('grid');
    // First group: Vietnamese letters a, à, á, ả, ã, ạ, ă, ằ | ắ …
    const cells = screen.getAllByRole('gridcell');
    expect(cells[0]?.getAttribute('aria-selected')).toBe('true');
    expect(cells[0]?.tabIndex).toBe(0);
    fireEvent.keyDown(grid, { key: 'ArrowRight' });
    fireEvent.keyDown(grid, { key: 'ArrowDown' });
    const sel = () => screen.getAllByRole('gridcell').find((c) => c.tabIndex === 0);
    expect(sel()?.getAttribute('data-index')).toBe('9');
    expect(document.activeElement).toBe(sel());
    fireEvent.keyDown(grid, { key: 'End' });
    expect(sel()?.getAttribute('data-index')).toBe('15');
    fireEvent.keyDown(grid, { key: 'Home' });
    expect(sel()?.getAttribute('data-index')).toBe('8');
    fireEvent.keyDown(grid, { key: 'Enter' });
    expect(onInsert).toHaveBeenCalledWith('ắ', undefined);
  });

  it('switches blocks and inserts with a double-click', () => {
    const onInsert = vi.fn();
    render(<SymbolPicker labels={labels} onInsert={onInsert} />);
    fireEvent.change(screen.getByTestId('symbol-block'), { target: { value: 'currency' } });
    const dong = screen
      .getAllByRole('gridcell')
      .find((c) => c.getAttribute('data-cp') === 'U+20AB');
    expect(dong).toBeDefined();
    if (!dong) return;
    fireEvent.mouseEnter(dong);
    expect(screen.getByTestId('symbol-info').textContent).toContain('U+20AB');
    fireEvent.doubleClick(dong);
    expect(onInsert).toHaveBeenCalledWith('₫', undefined);
  });

  it('shows a message when nothing matches', () => {
    render(<SymbolPicker labels={labels} onInsert={() => {}} />);
    fireEvent.change(screen.getByTestId('symbol-search'), { target: { value: 'zzzzqqq' } });
    expect(screen.getByText('Nothing found')).toBeTruthy();
  });

  it('inserts special characters from their tab', () => {
    const onInsert = vi.fn();
    render(<SymbolPicker labels={labels} onInsert={onInsert} />);
    fireEvent.click(screen.getByRole('tab', { name: 'Special' }));
    const list = screen.getByRole('listbox');
    expect(screen.getByText('label emDash')).toBeTruthy();
    fireEvent.keyDown(list, { key: 'ArrowDown' });
    fireEvent.keyDown(list, { key: 'Enter' });
    expect(onInsert).toHaveBeenCalledWith('–', undefined);
    fireEvent.click(screen.getByTestId('symbol-insert'));
    expect(onInsert).toHaveBeenLastCalledWith('–', undefined);
  });

  it('works without storage', () => {
    const spy = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('blocked');
    });
    const onInsert = vi.fn();
    render(<SymbolPicker labels={labels} onInsert={onInsert} />);
    fireEvent.keyDown(screen.getByRole('grid'), { key: 'Enter' });
    expect(onInsert).toHaveBeenCalledWith('a', undefined);
    expect(screen.getAllByTestId('symbol-recent')).toHaveLength(1);
    spy.mockRestore();
  });
});
