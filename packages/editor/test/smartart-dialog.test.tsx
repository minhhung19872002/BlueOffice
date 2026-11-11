// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  editOutline,
  itemsOf,
  nodeAtCaret,
  SMARTART_LAYOUTS,
  SMARTART_LIBRARY,
  SmartArtDialog,
  textOf,
  type SmartArtDraft,
} from '../src/SmartArtDialog';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const draft: SmartArtDraft = {
  layout: 'process',
  items: [
    { text: 'Một', level: 0 },
    { text: 'Hai', level: 1 },
  ],
  colors: 'colorful',
  style: 'fill',
};

describe('SmartArt text pane outline (CORE-SMART-005)', () => {
  it('reads levels from tabs and pairs of spaces', () => {
    expect(itemsOf('A\n\tB\n    C\n\n  D ')).toEqual([
      { text: 'A', level: 0 },
      { text: 'B', level: 1 },
      { text: 'C', level: 2 },
      { text: 'D', level: 1 },
    ]);
  });

  it('round-trips through text', () => {
    expect(itemsOf(textOf(draft.items))).toEqual(draft.items);
  });
});

describe('SmartArtDialog', () => {
  it('applies the layout, colours, look and outline', () => {
    const onApply = vi.fn();
    wrap(<SmartArtDialog mode="edit" initial={draft} onApply={onApply} onClose={() => {}} />);
    fireEvent.click(screen.getByTestId('smartart-layout-cycle'));
    fireEvent.change(screen.getByTestId('smartart-colors'), { target: { value: 'green' } });
    fireEvent.change(screen.getByTestId('smartart-style'), { target: { value: 'outline' } });
    fireEvent.change(screen.getByTestId('smartart-text'), { target: { value: 'Một\n\tHai\nBa' } });
    fireEvent.click(screen.getByTestId('smartart-ok'));
    expect(onApply).toHaveBeenCalledWith({
      layout: 'cycle',
      colors: 'green',
      style: 'outline',
      items: [
        { text: 'Một', level: 0 },
        { text: 'Hai', level: 1 },
        { text: 'Ba', level: 0 },
      ],
    });
  });

  it('cannot apply an empty outline', () => {
    wrap(<SmartArtDialog mode="insert" initial={draft} onApply={() => {}} onClose={() => {}} />);
    fireEvent.change(screen.getByTestId('smartart-text'), { target: { value: '  \n' } });
    expect((screen.getByTestId('smartart-ok') as HTMLButtonElement).disabled).toBe(true);
  });
});

// CORE-SMART-003: the layout library by category (names of `bo_pptx::diagram_layout::Layout`).
describe('SmartArt layout library', () => {
  it('has the eight categories, each starting with the layout named like it', () => {
    expect(SMARTART_LIBRARY.map((g) => g.category)).toEqual([
      'list',
      'process',
      'cycle',
      'hierarchy',
      'matrix',
      'pyramid',
      'relationship',
      'picture',
    ]);
    for (const g of SMARTART_LIBRARY) expect(g.layouts[0]).toBe(g.category);
    expect(new Set(SMARTART_LAYOUTS).size).toBe(17);
  });

  it('names every category and layout in both languages', () => {
    for (const lang of ['en-US', 'vi-VN'] as const) {
      const tr = createI18n(lang);
      for (const g of SMARTART_LIBRARY) {
        const c = `slideEditor.smartart.categories.${g.category}`;
        expect(tr.t(c)).not.toBe(c);
        for (const l of g.layouts) {
          const k = `slideEditor.smartart.layouts.${l}`;
          expect(tr.t(k)).not.toBe(k);
        }
      }
    }
  });

  it('offers every layout grouped under its category and applies the chosen one', () => {
    const onApply = vi.fn();
    wrap(<SmartArtDialog mode="insert" initial={draft} onApply={onApply} onClose={() => {}} />);
    for (const g of SMARTART_LIBRARY) {
      const group = screen.getByTestId(`smartart-category-${g.category}`);
      for (const l of g.layouts)
        expect(group.querySelector(`[data-testid="smartart-layout-${l}"]`)).not.toBeNull();
    }
    fireEvent.click(screen.getByTestId('smartart-layout-pictureStrip'));
    fireEvent.click(screen.getByTestId('smartart-ok'));
    expect(onApply.mock.calls[0]?.[0].layout).toBe('pictureStrip');
  });
});

// CORE-SMART-004: add / remove / promote / demote / move nodes of the outline.
describe('SmartArt node operations', () => {
  const outline = itemsOf('A\n\tA1\n\tA2\nB\n\tB1\nC');
  const text = (r: { items: ReturnType<typeof itemsOf> }) => textOf(r.items);

  it('adds a node after the node and its sub-nodes, at the same level', () => {
    const r = editOutline(outline, 0, 'add', 'Mới');
    expect(text(r)).toBe('A\n\tA1\n\tA2\nMới\nB\n\tB1\nC');
    expect(r.index).toBe(3);
    expect(text(editOutline(outline, 1, 'add', 'Mới'))).toBe('A\n\tA1\n\tMới\n\tA2\nB\n\tB1\nC');
    expect(editOutline([], 0, 'add', 'Mới')).toEqual({
      items: [{ text: 'Mới', level: 0 }],
      index: 0,
    });
  });

  it('removes a node and promotes its sub-nodes', () => {
    // B goes; its sub-node B1 takes B's level.
    expect(text(editOutline(outline, 3, 'remove'))).toBe('A\n\tA1\n\tA2\nB1\nC');
    expect(text(editOutline(outline, 0, 'remove'))).toBe('A1\nA2\nB\n\tB1\nC');
    const last = editOutline(outline, 5, 'remove');
    expect(text(last)).toBe('A\n\tA1\n\tA2\nB\n\tB1');
    expect(last.index).toBe(4);
  });

  it('promotes and demotes a node with its sub-nodes', () => {
    expect(text(editOutline(outline, 3, 'demote'))).toBe('A\n\tA1\n\tA2\n\tB\n\t\tB1\nC');
    expect(text(editOutline(outline, 1, 'promote'))).toBe('A\nA1\n\tA2\nB\n\tB1\nC');
    // Not possible: a top node cannot be promoted, the first node or a first sub-node demoted.
    expect(editOutline(outline, 0, 'promote').items).toEqual(outline);
    expect(editOutline(outline, 0, 'demote').items).toEqual(outline);
    expect(editOutline(outline, 1, 'demote').items).toEqual(outline);
  });

  it('moves a node among its siblings with its sub-nodes', () => {
    const up = editOutline(outline, 3, 'up');
    expect(text(up)).toBe('B\n\tB1\nA\n\tA1\n\tA2\nC');
    expect(up.index).toBe(0);
    const down = editOutline(outline, 0, 'down');
    expect(text(down)).toBe('B\n\tB1\nA\n\tA1\n\tA2\nC');
    expect(down.index).toBe(2);
    expect(text(editOutline(outline, 2, 'up'))).toBe('A\n\tA2\n\tA1\nB\n\tB1\nC');
    // Past the first or last sibling: unchanged.
    expect(editOutline(outline, 1, 'up').items).toEqual(outline);
    expect(editOutline(outline, 2, 'down').items).toEqual(outline);
    expect(editOutline(outline, 5, 'down').items).toEqual(outline);
  });

  it('finds the node at the caret, skipping blank lines', () => {
    const t = 'A\n\n\tA1\nB';
    expect(nodeAtCaret(t, 0)).toBe(0);
    expect(nodeAtCaret(t, 2)).toBe(0);
    expect(nodeAtCaret(t, 5)).toBe(1);
    expect(nodeAtCaret(t, t.length)).toBe(2);
  });

  it('runs from the node buttons on the node at the caret', () => {
    const onApply = vi.fn();
    wrap(
      <SmartArtDialog
        mode="edit"
        initial={{ ...draft, items: outline }}
        onApply={onApply}
        onClose={() => {}}
      />,
    );
    const pane = screen.getByTestId('smartart-text') as HTMLTextAreaElement;
    // Caret in "B" (line 4): demote it, then add a node after it and move that one up.
    const at = pane.value.indexOf('B');
    pane.setSelectionRange(at, at);
    fireEvent.click(screen.getByTestId('smartart-node-demote'));
    expect(pane.value).toBe('A\n\tA1\n\tA2\n\tB\n\t\tB1\nC');
    fireEvent.click(screen.getByTestId('smartart-node-add'));
    expect(pane.value).toBe('A\n\tA1\n\tA2\n\tB\n\t\tB1\n\tItem 7\nC');
    // The caret followed to the new node.
    expect(nodeAtCaret(pane.value, pane.selectionStart)).toBe(5);
    fireEvent.click(screen.getByTestId('smartart-node-up'));
    expect(pane.value).toBe('A\n\tA1\n\tA2\n\tItem 7\n\tB\n\t\tB1\nC');
    fireEvent.click(screen.getByTestId('smartart-node-remove'));
    fireEvent.click(screen.getByTestId('smartart-node-promote'));
    expect(pane.value).toBe('A\n\tA1\n\tA2\nB\n\tB1\nC');
    fireEvent.click(screen.getByTestId('smartart-ok'));
    expect(onApply.mock.calls[0]?.[0].items).toEqual(outline);
  });
});
