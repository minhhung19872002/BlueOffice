import { describe, expect, it } from 'vitest';
import {
  formatKeyCommand,
  numberCategory,
  shownDecimals,
  stepDecimals,
  stepFontSize,
} from '../src/homeFormat';

describe('font size steps (Increase / Decrease font size)', () => {
  it('walks the size list', () => {
    expect(stepFontSize(11, 1)).toBe(12);
    expect(stepFontSize(12, 1)).toBe(14);
    expect(stepFontSize(14, -1)).toBe(12);
    expect(stepFontSize(11, -1)).toBe(10);
    // Between two sizes: the next one of the list.
    expect(stepFontSize(13, 1)).toBe(14);
    expect(stepFontSize(13, -1)).toBe(12);
  });
  it('steps by 10 above 72 and by 1 below 8', () => {
    expect(stepFontSize(72, 1)).toBe(80);
    expect(stepFontSize(80, 1)).toBe(90);
    expect(stepFontSize(80, -1)).toBe(72);
    expect(stepFontSize(8, -1)).toBe(7);
    expect(stepFontSize(1, -1)).toBe(1);
    expect(stepFontSize(405, 1)).toBe(409);
  });
});

describe('increase / decrease decimal', () => {
  it('starts a General cell from the decimals it shows', () => {
    expect(stepDecimals('General', 1, 1)).toBe('0.00');
    expect(stepDecimals('General', 1, -1)).toBe('0');
    expect(stepDecimals('General', 0, 1)).toBe('0.0');
    expect(stepDecimals('', 0, -1)).toBe('0');
  });
  it('adds or removes one decimal place in every section', () => {
    expect(stepDecimals('#,##0.00', 2, 1)).toBe('#,##0.000');
    expect(stepDecimals('#,##0.00', 2, -1)).toBe('#,##0.0');
    expect(stepDecimals('0.0', 1, -1)).toBe('0');
    expect(stepDecimals('0', 0, -1)).toBe('0');
    expect(stepDecimals('0%', 0, 1)).toBe('0.0%');
    expect(stepDecimals('0.00E+00', 2, 1)).toBe('0.000E+00');
    expect(stepDecimals('#,##0 [$₫-42A]', 0, 1)).toBe('#,##0.0 [$₫-42A]');
    expect(stepDecimals('_-* #,##0.00_-;-* #,##0.00_-;_-* "-"??_-;_-@_-', 2, -1)).toBe(
      '_-* #,##0.0_-;-* #,##0.0_-;_-* "-"?_-;_-@_-',
    );
    expect(stepDecimals('_-* #,##0.00_-;-* #,##0.00_-;_-* "-"??_-;_-@_-', 2, 1)).toBe(
      '_-* #,##0.000_-;-* #,##0.000_-;_-* "-"???_-;_-@_-',
    );
    // Fractions are left alone.
    expect(stepDecimals('# ?/?', 0, 1)).toBe('# ?/?');
  });
  it('reads the decimals a cell shows', () => {
    expect(shownDecimals('12,5', ',')).toBe(1);
    expect(shownDecimals('1.234,567', ',')).toBe(3);
    expect(shownDecimals('1,234.5', '.')).toBe(1);
    expect(shownDecimals('75', ',')).toBe(0);
  });
});

describe('number format box category', () => {
  it('names the category of a code', () => {
    expect(numberCategory('General')).toBe('general');
    expect(numberCategory('#,##0.00')).toBe('number');
    expect(numberCategory('0')).toBe('number');
    expect(numberCategory('#,##0 [$₫-42A]')).toBe('currencyVnd');
    expect(
      numberCategory('_-* #,##0 [$₫-42A]_-;-* #,##0 [$₫-42A]_-;_-* "-" [$₫-42A]_-;_-@_-'),
    ).toBe('accounting');
    expect(numberCategory('0%')).toBe('percent');
    expect(numberCategory('dd/mm/yyyy')).toBe('date');
    expect(numberCategory('d-mmm')).toBe('date');
    expect(numberCategory('dddd, dd mmmm yyyy')).toBe('dateLong');
    expect(numberCategory('h:mm')).toBe('time');
    expect(numberCategory('# ?/?')).toBe('fraction');
    expect(numberCategory('0.00E+00')).toBe('scientific');
    expect(numberCategory('@')).toBe('text');
  });
});

describe('formatting shortcuts by physical key', () => {
  const key = (code: string, k: string, mods: { ctrl?: boolean; shift?: boolean } = {}) =>
    formatKeyCommand({
      code,
      key: k,
      ctrlKey: mods.ctrl ?? true,
      metaKey: false,
      shiftKey: mods.shift ?? false,
      altKey: false,
    });
  it('maps the spreadsheet shortcuts', () => {
    expect(key('Digit2', '2')).toBe('format.bold');
    expect(key('Digit3', '3')).toBe('format.italic');
    expect(key('Digit4', '4')).toBe('format.underline');
    expect(key('Digit1', '!', { shift: true })).toBe('format.numNumber');
    expect(key('Digit4', '$', { shift: true })).toBe('format.numCurrency');
    expect(key('Backquote', '~', { shift: true })).toBe('format.numGeneral');
    expect(key('Digit7', '&', { shift: true })).toBe('format.borderOutline');
    expect(key('Minus', '_', { shift: true })).toBe('format.borderNone');
    expect(key('Equal', '+', { shift: true })).toBe('cells.insert');
    expect(key('NumpadAdd', '+')).toBe('cells.insert');
    expect(key('Equal', '=', { shift: true })).toBe('cells.insert');
    expect(key('Minus', '-')).toBe('cells.delete');
    expect(key('Digit9', '9')).toBe('cells.hideRows');
    expect(key('Digit0', ')', { shift: true })).toBe('cells.unhideCols');
    expect(key('F11', 'F11', { ctrl: false, shift: true })).toBe('sheet.insertBefore');
    expect(key('PageDown', 'PageDown')).toBe('sheet.next');
    // Ctrl+Shift+% stays with the registry (format.percent); plain keys do nothing.
    expect(key('Digit5', '%', { shift: true })).toBeNull();
    expect(key('KeyA', 'a', { ctrl: false })).toBeNull();
  });
});
