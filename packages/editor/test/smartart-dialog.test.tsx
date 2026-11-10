// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { itemsOf, SmartArtDialog, textOf, type SmartArtDraft } from '../src/SmartArtDialog';

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
