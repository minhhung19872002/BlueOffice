// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { EditCommand, EditResult, ListLevelSpec } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  defaultLevels,
  DefineListDialog,
  formatNumber,
  levelLabel,
  MultilevelMenu,
  SetNumberingDialog,
} from '../src/ListDialogs';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

describe('list labels', () => {
  it('formats numbers like the engine (§17.18.59)', () => {
    expect(formatNumber(4, 'lowerRoman')).toBe('iv');
    expect(formatNumber(28, 'upperLetter')).toBe('BB');
    expect(formatNumber(3, 'decimalZero')).toBe('03');
    expect(formatNumber(3, 'bullet')).toBe('');
  });

  it('replaces %1…%9 with the levels’ numbers', () => {
    const levels = defaultLevels();
    expect(levelLabel(levels, 0)).toBe('1.');
    expect(levelLabel(levels, 2, [2, 3, 1])).toBe('2.3.1.');
    levels[1] = { ...levels[1]!, format: 'lowerLetter', text: '%2)' };
    expect(levelLabel(levels, 1, [1, 2])).toBe('b)');
  });
});

describe('define new multilevel list dialog', () => {
  it('edits a level, links headings and previews the labels', () => {
    let sent: { levels: ListLevelSpec[]; apply: boolean } | null = null;
    wrap(
      <DefineListDialog
        onClose={() => {}}
        onApply={(levels, apply) => {
          sent = { levels, apply };
        }}
      />,
    );
    const preview = screen.getByTestId('list-preview');
    expect(preview.querySelector('[data-level="2"]')?.textContent).toContain('1.1.');
    // Level 3: lower-case letters "a)".
    fireEvent.click(screen.getByTestId('list-level-3'));
    fireEvent.change(screen.getByTestId('list-format'), { target: { value: 'lowerLetter' } });
    fireEvent.change(screen.getByTestId('list-text'), { target: { value: '%3)' } });
    fireEvent.change(screen.getByTestId('list-start'), { target: { value: '2' } });
    expect(preview.querySelector('[data-level="3"]')?.textContent).toContain('b)');
    fireEvent.click(screen.getByTestId('list-link-headings'));
    expect(preview.querySelector('[data-level="1"]')?.textContent).toContain('Heading 1');
    fireEvent.click(screen.getByTestId('list-apply'));
    fireEvent.click(screen.getByTestId('define-list-ok'));
    expect(sent).not.toBeNull();
    const s = sent as unknown as { levels: ListLevelSpec[]; apply: boolean };
    expect(s.apply).toBe(false);
    expect(s.levels).toHaveLength(9);
    expect(s.levels[2]).toMatchObject({
      format: 'lowerLetter',
      text: '%3)',
      start: 2,
      style: 'Heading3',
    });
    expect(s.levels[8]?.style).toBe('Heading9');
  });

  it('restart after a level and legal-style numbering (§17.9.10, §17.9.4)', () => {
    let levels: ListLevelSpec[] = [];
    wrap(<DefineListDialog onClose={() => {}} onApply={(l) => (levels = l)} />);
    // Level 1 has no "restart after" choice.
    expect(screen.queryByTestId('list-restart')).toBeNull();
    fireEvent.change(screen.getByTestId('list-format'), { target: { value: 'upperRoman' } });
    fireEvent.click(screen.getByTestId('list-level-3'));
    const restart = screen.getByTestId('list-restart') as HTMLSelectElement;
    // The level above (default), levels 1–2, never.
    expect(restart.options).toHaveLength(4);
    fireEvent.change(restart, { target: { value: '1' } });
    fireEvent.click(screen.getByTestId('list-legal'));
    const preview = screen.getByTestId('list-preview');
    expect(preview.querySelector('[data-level="2"]')?.textContent).toContain('I.1.');
    expect(preview.querySelector('[data-level="3"]')?.textContent).toContain('1.1.1.');
    fireEvent.click(screen.getByTestId('define-list-ok'));
    expect(levels[2]).toMatchObject({ restart: 1, legal: true });
    expect(levels[1]?.restart ?? null).toBeNull();
    expect(levels[1]?.legal ?? false).toBe(false);
  });

  it('switching to a bullet sets the bullet character', () => {
    let levels: ListLevelSpec[] = [];
    wrap(<DefineListDialog onClose={() => {}} onApply={(l) => (levels = l)} />);
    fireEvent.change(screen.getByTestId('list-format'), { target: { value: 'bullet' } });
    fireEvent.click(screen.getByTestId('define-list-ok'));
    expect(levels[0]).toMatchObject({ format: 'bullet', text: '•', style: null });
  });
});

describe('set numbering value dialog', () => {
  it('restarts at a value', async () => {
    const commands: EditCommand[] = [];
    const run = vi.fn(async (c: EditCommand) => {
      commands.push(c);
      return {} as EditResult;
    });
    const onClose = vi.fn();
    wrap(<SetNumberingDialog run={run} onClose={onClose} />);
    fireEvent.change(screen.getByTestId('numbering-value'), { target: { value: '5' } });
    fireEvent.click(screen.getByTestId('set-numbering-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(commands[0]).toEqual({
      cmd: 'setListNumbering',
      numbering: { kind: 'restart', value: 5 },
    });
  });

  it('continues the previous list and reports when not in a list', async () => {
    const run = vi.fn(async () => ({ error: 'unsupported: not in a list' }) as EditResult);
    const onClose = vi.fn();
    wrap(<SetNumberingDialog run={run} onClose={onClose} />);
    fireEvent.click(screen.getByTestId('numbering-continue'));
    expect((screen.getByTestId('numbering-value') as HTMLInputElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId('set-numbering-ok'));
    await screen.findByTestId('numbering-error');
    expect(run).toHaveBeenCalledWith({ cmd: 'setListNumbering', numbering: { kind: 'continue' } });
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe('multilevel list menu', () => {
  it('runs the picked command', () => {
    const define = vi.fn();
    wrap(
      <MultilevelMenu
        disabled={false}
        entries={[
          { id: 'format.defineList', label: 'Define', enabled: true, onSelect: define },
          { id: 'format.continueNumbering', label: 'Continue', enabled: false, onSelect: vi.fn() },
        ]}
      />,
    );
    fireEvent.click(screen.getByTestId('multilevel-list'));
    const item = document.querySelector('[data-command="format.defineList"]') as HTMLButtonElement;
    fireEvent.click(item);
    expect(define).toHaveBeenCalled();
  });
});
