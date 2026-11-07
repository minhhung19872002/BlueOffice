// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { EditCommand, FormatState } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MARGINS, PAPER } from '../src/DocDialogs';
import { ColorPicker, FontFamilyPicker, FontSizeBox, HIGHLIGHT_PALETTE } from '../src/FontControls';
import { Rulers, setupOf } from '../src/Ruler';

afterEach(cleanup);

const i18n = createI18n('vi-VN');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const format = (patch: Partial<FormatState> = {}): FormatState => ({
  bold: false,
  italic: false,
  underline: false,
  strike: false,
  superscript: false,
  subscript: false,
  font: 'Times New Roman',
  sizeHalfPoints: 26,
  color: null,
  highlight: null,
  align: 'Left',
  style: null,
  bullets: false,
  numbering: false,
  canUndo: false,
  canRedo: false,
  indentLeft: 0,
  indentRight: 0,
  firstLine: 0,
  tabs: [],
  section: {
    index: 0,
    count: 1,
    width: 11906,
    height: 16838,
    marginTop: 1134,
    marginBottom: 1134,
    marginLeft: 1701,
    marginRight: 851,
    header: 720,
    footer: 720,
    columns: 1,
    columnSpace: 720,
    titlePage: false,
    evenAndOdd: false,
  },
  ...patch,
});

describe('font controls', () => {
  it('lists substituted fonts and applies the picked family', async () => {
    const commands: EditCommand[] = [];
    const run = vi.fn(async (c: EditCommand) => {
      commands.push(c);
    });
    wrap(
      <FontFamilyPicker
        format={format()}
        fonts={[
          { name: 'Tinos', substitute: null, available: true },
          { name: 'Times New Roman', substitute: 'Tinos', available: false },
        ]}
        run={run}
        disabled={false}
        onDone={() => {}}
      />,
    );
    const select = screen.getByTestId('font-family') as HTMLSelectElement;
    expect(select.value).toBe('Times New Roman');
    expect(screen.getByText('Times New Roman → Tinos')).toBeTruthy();
    fireEvent.change(select, { target: { value: 'Tinos' } });
    expect(commands[0]).toEqual({ cmd: 'formatText', format: { kind: 'font', name: 'Tinos' } });
  });

  it('size box accepts typed sizes in half points', () => {
    const run = vi.fn(async () => {});
    wrap(<FontSizeBox format={format()} run={run} disabled={false} onDone={() => {}} />);
    const input = screen.getByTestId('font-size') as HTMLInputElement;
    expect(input.value).toBe('13');
    fireEvent.change(input, { target: { value: '10,5' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(run).toHaveBeenCalledWith({
      cmd: 'formatText',
      format: { kind: 'size', halfPoints: 21 },
    });
    run.mockClear();
    fireEvent.change(input, { target: { value: 'abc' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(run).not.toHaveBeenCalled();
  });

  it('colour picker: palette, automatic and custom hex', () => {
    const pick = vi.fn();
    wrap(
      <ColorPicker
        kind="highlight"
        label="Tô sáng"
        testId="hl"
        value={null}
        disabled={false}
        onPick={pick}
      />,
    );
    fireEvent.click(screen.getByTestId('hl'));
    const swatches = screen.getByTestId('hl-panel').querySelectorAll('[data-color]');
    // "No colour" + the 15 highlight colours of ST_HighlightColor.
    expect(swatches.length).toBe(1 + HIGHLIGHT_PALETTE.flat().length);
    fireEvent.click(screen.getByTestId('hl-panel').querySelector('[data-color="FFFF00"]')!);
    expect(pick).toHaveBeenLastCalledWith('FFFF00');
    fireEvent.click(screen.getByTestId('hl'));
    fireEvent.change(screen.getByTestId('hl-custom'), { target: { value: '#1d4f91' } });
    fireEvent.submit(screen.getByTestId('hl-custom').closest('form')!);
    expect(pick).toHaveBeenLastCalledWith('1D4F91');
  });
});

describe('rulers and page setup', () => {
  it('page presets follow Nghị định 30/2020 and the paper sizes', () => {
    const [top, bottom, left, right] = MARGINS.nd30!;
    const mm = (tw: number) => tw / 56.692;
    expect(mm(top)).toBeGreaterThanOrEqual(20);
    expect(mm(bottom)).toBeLessThanOrEqual(25);
    expect(mm(left)).toBeGreaterThanOrEqual(30);
    expect(mm(right)).toBeGreaterThanOrEqual(15);
    expect(PAPER.a4).toEqual([11906, 16838]);
    expect(setupOf(format())?.marginLeft).toBe(1701);
    expect(setupOf(null)).toBeNull();
  });

  it('shows margins, indents and tab stops; the tab kind cycles', () => {
    const run = vi.fn(async () => {});
    wrap(
      <Rulers
        box={{ x: 100, y: 40, w: 794, h: 1123 }}
        format={format({ indentLeft: 567, firstLine: -283, tabs: [{ pos: 2835, align: 'right' }] })}
        run={run}
        language="vi-VN"
        disabled={false}
      />,
    );
    expect(screen.getByTestId('ruler-margin-left').getAttribute('data-twips')).toBe('1701');
    expect(screen.getByTestId('ruler-left-indent').getAttribute('data-twips')).toBe('567');
    expect(screen.getByTestId('ruler-first-line').getAttribute('data-twips')).toBe('-283');
    expect(screen.getByTestId('ruler-tab').getAttribute('data-kind')).toBe('right');
    const kind = screen.getByTestId('ruler-tab-type');
    expect(kind.getAttribute('data-kind')).toBe('left');
    fireEvent.click(kind);
    expect(kind.getAttribute('data-kind')).toBe('center');
  });
});
