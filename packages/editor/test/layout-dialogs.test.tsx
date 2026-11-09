// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { EditCommand, EditResult, PageLayoutState } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  columnsPreset,
  ColumnsDialog,
  DropCapDialog,
  equalColumns,
  HyphenationDialog,
  isEqual,
  LineNumbersDialog,
  lineNumberingCommand,
  presetOf,
  TextDirectionDialog,
} from '../src/LayoutDialogs';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

/** 6.5 inch of text. */
const TW = 9360;

const STATE: PageLayoutState = {
  columns: equalColumns(1, 720, TW),
  textWidth: TW,
  hyphenation: { auto: false, zone: null, consecutiveLimit: null, noCaps: false },
  suppressLineNumbers: false,
  suppressAutoHyphens: false,
};

/** A `run` that answers `pageLayout` with `state` and records the other commands. */
function engine(state: PageLayoutState = STATE) {
  const sent: EditCommand[] = [];
  const run = vi.fn(async (c: EditCommand) => {
    if (c.cmd === 'pageLayout') return { pageLayout: state } as EditResult;
    sent.push(c);
    return {} as EditResult;
  });
  return { run, sent };
}

describe('column presets', () => {
  it('equal columns share the text width', () => {
    const c = equalColumns(3, 360, TW);
    // (9360 - 2 × 360) / 3 = 2880.
    expect(c.widths.map((w) => w.width)).toEqual([2880, 2880, 2880]);
    expect(c.widths.map((w) => w.space)).toEqual([360, 360, 0]);
    expect(isEqual(c)).toBe(true);
  });

  it('a narrow left column is a third of the text', () => {
    const c = columnsPreset('left', TW);
    // (9360 - 720) / 3 = 2880, the rest 5760.
    expect(c.widths).toEqual([
      { width: 2880, space: 720 },
      { width: 5760, space: 0 },
    ]);
    expect(isEqual(c)).toBe(false);
    expect(presetOf(c, TW)).toBe('left');
    expect(presetOf(columnsPreset('right', TW), TW)).toBe('right');
    expect(presetOf(equalColumns(2, 720, TW), TW)).toBe('two');
  });
});

describe('ColumnsDialog', () => {
  it('sends a preset with the separator, from the cursor on', async () => {
    const { run, sent } = engine();
    const onClose = vi.fn();
    wrap(<ColumnsDialog run={run} language="en-US" onClose={onClose} />);
    fireEvent.click(await screen.findByTestId('columns-preset-three'));
    fireEvent.click(screen.getByTestId('columns-separator'));
    fireEvent.change(screen.getByTestId('columns-scope'), { target: { value: 'forward' } });
    fireEvent.click(screen.getByTestId('columns-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      cmd: 'setColumns',
      scope: 'forward',
      columns: { count: 3, space: 720, separator: true, widths: [] },
    });
    expect(onClose).toHaveBeenCalled();
  });

  it('sends the widths of unequal columns', async () => {
    const { run, sent } = engine();
    wrap(<ColumnsDialog run={run} language="en-US" onClose={() => {}} />);
    fireEvent.click(await screen.findByTestId('columns-preset-right'));
    expect(screen.getByTestId('columns-col-1')).toBeTruthy();
    // 1 inch for the first column.
    fireEvent.change(screen.getByTestId('columns-width-0'), { target: { value: '1' } });
    fireEvent.click(screen.getByTestId('columns-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({
      cmd: 'setColumns',
      scope: 'section',
      columns: { count: 2, widths: [{ width: 1440 }, { width: 2880 }] },
    });
  });
});

describe('LineNumbersDialog', () => {
  it('stores the first number less one', () => {
    expect(lineNumberingCommand('newPage', 1, 5, null, 'section')).toEqual({
      cmd: 'setLineNumbering',
      scope: 'section',
      numbering: { countBy: 5, start: 0, distance: null, restart: 'newPage' },
    });
    expect(lineNumberingCommand('none', 1, 1, null, 'document')).toMatchObject({
      numbering: null,
    });
  });

  it('reads the current numbering and skips the paragraph', async () => {
    const { run, sent } = engine({
      ...STATE,
      lineNumbers: { countBy: 2, start: 9, distance: 283, restart: 'newSection' },
    });
    wrap(<LineNumbersDialog run={run} language="en-US" onClose={() => {}} />);
    const start = (await screen.findByTestId('line-numbers-start')) as HTMLInputElement;
    await waitFor(() => expect(start.value).toBe('10'));
    fireEvent.change(screen.getByTestId('line-numbers-mode'), {
      target: { value: 'continuous' },
    });
    fireEvent.click(screen.getByTestId('line-numbers-suppress'));
    fireEvent.click(screen.getByTestId('line-numbers-ok'));
    await waitFor(() => expect(sent).toHaveLength(2));
    expect(sent[0]).toEqual({
      cmd: 'setLineNumbering',
      scope: 'section',
      numbering: { countBy: 2, start: 9, distance: 283, restart: 'continuous' },
    });
    expect(sent[1]).toEqual({
      cmd: 'formatParagraph',
      format: { kind: 'suppressLineNumbers', on: true },
    });
  });
});

describe('HyphenationDialog', () => {
  it('turns automatic hyphenation on with a limit', async () => {
    const { run, sent } = engine();
    wrap(<HyphenationDialog run={run} language="en-US" onClose={() => {}} />);
    fireEvent.click(await screen.findByTestId('hyphenation-auto'));
    fireEvent.click(screen.getByTestId('hyphenation-caps'));
    fireEvent.change(screen.getByTestId('hyphenation-limit'), { target: { value: '2' } });
    fireEvent.click(screen.getByTestId('hyphenation-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      cmd: 'setHyphenation',
      hyphenation: { auto: true, zone: null, consecutiveLimit: 2, noCaps: true },
    });
  });
});

describe('TextDirectionDialog', () => {
  it('sends the direction for every section', async () => {
    const { run, sent } = engine();
    wrap(<TextDirectionDialog run={run} onClose={() => {}} />);
    fireEvent.click(await screen.findByTestId('text-direction-btLr'));
    fireEvent.change(screen.getByTestId('text-direction-scope'), {
      target: { value: 'document' },
    });
    fireEvent.click(screen.getByTestId('text-direction-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({ cmd: 'setSectionTextDirection', dir: 'btLr', scope: 'document' });
  });
});

describe('DropCapDialog', () => {
  it('makes, then removes, a large initial', async () => {
    const { run, sent } = engine();
    wrap(<DropCapDialog run={run} language="en-US" onClose={() => {}} />);
    fireEvent.click(await screen.findByTestId('drop-cap-margin'));
    fireEvent.change(screen.getByTestId('drop-cap-font'), { target: { value: 'Georgia' } });
    fireEvent.change(screen.getByTestId('drop-cap-lines'), { target: { value: '2' } });
    fireEvent.click(screen.getByTestId('drop-cap-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      cmd: 'setDropCap',
      dropCap: { position: 'margin', font: 'Georgia', lines: 2, distance: 0 },
    });
    cleanup();
    const e = engine({
      ...STATE,
      dropCap: { position: 'drop', font: null, lines: 3, distance: 0 },
    });
    wrap(<DropCapDialog run={e.run} language="en-US" onClose={() => {}} />);
    const none = (await screen.findByTestId('drop-cap-none')) as HTMLInputElement;
    await waitFor(() =>
      expect((screen.getByTestId('drop-cap-drop') as HTMLInputElement).checked).toBe(true),
    );
    fireEvent.click(none);
    fireEvent.click(screen.getByTestId('drop-cap-ok'));
    await waitFor(() => expect(e.sent).toHaveLength(1));
    expect(e.sent[0]).toEqual({ cmd: 'setDropCap', dropCap: null });
  });
});
