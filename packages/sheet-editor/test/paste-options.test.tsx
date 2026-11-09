// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SHEET_PASTE_CHOICES, SheetClipboardUi, sheetPasteCommand } from '../src/pasteOptions';

afterEach(cleanup);

const i18n = createI18n('en-US');
const base = { sheet: 0, row: 2, col: 1, range: { r0: 2, c0: 1, r1: 2, c1: 1 } };

describe('sheet paste options', () => {
  it('maps every choice to one engine paste', () => {
    expect(sheetPasteCommand('values', base)).toMatchObject({
      cmd: 'paste',
      row: 2,
      col: 1,
      mode: 'values',
      transpose: false,
      colWidths: false,
      noBorders: false,
    });
    expect(sheetPasteCommand('transpose', base)).toMatchObject({ mode: 'all', transpose: true });
    expect(sheetPasteCommand('colWidths', base)).toMatchObject({ mode: 'all', colWidths: true });
    expect(sheetPasteCommand('noBorders', base)).toMatchObject({ mode: 'all', noBorders: true });
    expect(SHEET_PASTE_CHOICES).toHaveLength(7);
  });

  it('shows the options after a paste and reports the choice', () => {
    const onChoose = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <SheetClipboardUi
          paste={{ n: 1, mode: 'all' }}
          anchor={() => ({ x: 10, y: 10 })}
          onChoose={onChoose}
          panel={false}
          onPasteItem={() => {}}
          onClosePanel={() => {}}
        />
      </I18nextProvider>,
    );
    fireEvent.click(screen.getByTestId('paste-options'));
    expect(screen.getAllByRole('menuitemradio')).toHaveLength(7);
    fireEvent.click(screen.getByRole('menuitemradio', { name: /Everything except borders/ }));
    expect(onChoose).toHaveBeenCalledWith('noBorders');
  });
});
