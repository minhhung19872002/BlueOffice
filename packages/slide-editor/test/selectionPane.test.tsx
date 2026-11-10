// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { SlideCommand } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SelectionPane, selectionAfterClick } from '../src/SelectionPane';

afterEach(cleanup);

const i18n = createI18n('en-US');

describe('SelectionPane', () => {
  it('Ctrl+click adds or removes; a plain click selects one', () => {
    expect(selectionAfterClick([1, 2], 3, false)).toEqual([3]);
    expect(selectionAfterClick([1], 3, true)).toEqual([1, 3]);
    expect(selectionAfterClick([1, 3], 3, true)).toEqual([1]);
  });

  it('lists objects front first, selects and reorders', () => {
    const run = vi.fn<(c: SlideCommand) => void>();
    render(
      <I18nextProvider i18n={i18n}>
        <SelectionPane
          objects={[
            { id: 2, name: 'Title 1' },
            { id: 3, name: 'Picture 2' },
          ]}
          selected={[2]}
          run={run}
        />
      </I18nextProvider>,
    );
    const items = screen.getAllByTestId('selection-item');
    expect(items.map((i) => i.textContent)).toEqual(['Picture 2', 'Title 1']);
    expect(items[1]!.getAttribute('aria-selected')).toBe('true');
    fireEvent.click(items[0]!, { ctrlKey: true });
    expect(run).toHaveBeenLastCalledWith({ cmd: 'select', ids: [2, 3] });
    fireEvent.click(screen.getByTestId('selection-forward'));
    expect(run).toHaveBeenLastCalledWith({ cmd: 'zOrder', to: 'forward' });
  });
});
