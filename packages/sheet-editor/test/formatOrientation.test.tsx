// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { CellFormatState, StylePatch } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FormatCellsDialog } from '../src/FormatCellsDialog';
import { VERTICAL_TEXT, degreesToRotation, rotationToDegrees } from '../src/orientation';

afterEach(cleanup);

const i18n = createI18n('en-US');

const state = (over: Partial<CellFormatState> = {}): CellFormatState => ({
  numFmt: 'General',
  fontName: 'Calibri',
  fontSize: 11,
  bold: false,
  italic: false,
  underline: '',
  strike: false,
  fontColor: '',
  fillColor: '',
  hAlign: '',
  vAlign: '',
  wrap: false,
  indent: 0,
  rotation: 0,
  borders: ['', '', '', ''],
  styleName: 'Normal',
  merged: false,
  locked: true,
  formulaHidden: false,
  ...over,
});

function open(initial: CellFormatState) {
  const onApply = vi.fn<(patch: StylePatch) => void>();
  render(
    <I18nextProvider i18n={i18n}>
      <FormatCellsDialog
        open
        initial={initial}
        presets={[]}
        onClose={() => {}}
        onApply={(p) => onApply(p)}
      />
    </I18nextProvider>,
  );
  fireEvent.click(screen.getByRole('tab', { name: 'Alignment' }));
  return onApply;
}

describe('text orientation mapping (§18.8.1 textRotation)', () => {
  it('maps dialog angles to stored values and back', () => {
    expect(degreesToRotation(45, false)).toBe(45);
    expect(degreesToRotation(90, false)).toBe(90);
    expect(degreesToRotation(-1, false)).toBe(91);
    expect(degreesToRotation(-45, false)).toBe(135);
    expect(degreesToRotation(-90, false)).toBe(180);
    expect(degreesToRotation(-120, false)).toBe(180);
    expect(degreesToRotation(30, true)).toBe(VERTICAL_TEXT);
    expect(rotationToDegrees(135)).toBe(-45);
    expect(rotationToDegrees(180)).toBe(-90);
    expect(rotationToDegrees(60)).toBe(60);
    expect(rotationToDegrees(VERTICAL_TEXT)).toBe(0);
  });
});

describe('Format Cells alignment tab', () => {
  it('applies a clockwise angle as textRotation 91–180', () => {
    const onApply = open(state());
    fireEvent.change(screen.getByTestId('format-cells-rotation'), { target: { value: '-45' } });
    fireEvent.click(screen.getByTestId('format-cells-ok'));
    expect(onApply).toHaveBeenCalledWith({ rotation: 135 });
  });

  it('shows a stored clockwise rotation as a negative angle', () => {
    open(state({ rotation: 120 }));
    expect((screen.getByTestId('format-cells-rotation') as HTMLInputElement).value).toBe('-30');
  });

  it('switches to stacked text and shrink to fit', () => {
    const onApply = open(state({ rotation: 30 }));
    fireEvent.click(screen.getByTestId('format-cells-vertical'));
    expect((screen.getByTestId('format-cells-rotation') as HTMLInputElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId('format-cells-shrink'));
    fireEvent.click(screen.getByTestId('format-cells-ok'));
    expect(onApply).toHaveBeenCalledWith({ rotation: 255, shrink: true });
  });

  it('disables shrink to fit while text wraps', () => {
    open(state({ wrap: true }));
    expect((screen.getByTestId('format-cells-shrink') as HTMLInputElement).disabled).toBe(true);
  });
});
