// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { AutoCorrectOptions, EditCommand, EditResult } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  AUTOCORRECT_RULES,
  AutoCorrectDialog,
  autoCorrectCommand,
  loadAutoCorrect,
  saveAutoCorrect,
} from '../src/AutoCorrectDialog';

afterEach(cleanup);
beforeEach(() => localStorage.clear());

const i18n = createI18n('vi-VN');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const defaults = (): AutoCorrectOptions => ({
  enabled: true,
  replaceText: true,
  capitalizeSentences: true,
  capitalizeCells: true,
  twoInitialCaps: true,
  smartQuotes: true,
  dashes: true,
  ordinals: true,
  fractions: true,
  autoLists: true,
  autoLinks: true,
  entries: [
    { from: '(c)', to: '©' },
    { from: 'teh', to: 'the' },
  ],
  exceptions: ['v.v.', 'TP.'],
  capsExceptions: ['IDs'],
});

function setup(onSaved = vi.fn()) {
  const commands: EditCommand[] = [];
  let current = defaults();
  const run = vi.fn(async (c: EditCommand) => {
    commands.push(c);
    if (c.cmd === 'autoCorrect') {
      if (c.reset) current = defaults();
      else if (c.options) current = c.options;
      return { autocorrect: current } as EditResult;
    }
    return null;
  });
  const onClose = vi.fn();
  wrap(<AutoCorrectDialog run={run} onClose={onClose} onSaved={onSaved} />);
  return { commands, onClose, onSaved };
}

describe('AutoCorrectDialog', () => {
  it('shows the engine options and sends the edited ones', async () => {
    const { commands, onClose, onSaved } = setup();
    await screen.findByTestId('autocorrect-enabled');
    expect(commands[0]).toEqual({ cmd: 'autoCorrect' });
    for (const k of AUTOCORRECT_RULES)
      expect((screen.getByTestId(`autocorrect-${k}`) as HTMLInputElement).checked).toBe(true);
    // Turn a rule off, add an entry, remove one, edit exceptions.
    fireEvent.click(screen.getByTestId('autocorrect-smartQuotes'));
    fireEvent.change(screen.getByTestId('autocorrect-from'), { target: { value: 'bxd' } });
    fireEvent.change(screen.getByTestId('autocorrect-to'), { target: { value: 'Bộ Xây dựng' } });
    fireEvent.click(screen.getByTestId('autocorrect-add'));
    fireEvent.click(screen.getByLabelText('Xoá (c)'));
    fireEvent.change(screen.getByTestId('autocorrect-exceptions'), {
      target: { value: 'v.v.\n  TP. \n\nÔ.' },
    });
    expect(screen.getByTestId('autocorrect-entries').textContent).toContain('Bộ Xây dựng');
    fireEvent.click(screen.getByTestId('autocorrect-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const sent = commands.at(-1);
    expect(sent).toEqual({
      cmd: 'autoCorrect',
      options: {
        ...defaults(),
        smartQuotes: false,
        entries: [
          { from: 'teh', to: 'the' },
          { from: 'bxd', to: 'Bộ Xây dựng' },
        ],
        exceptions: ['v.v.', 'TP.', 'Ô.'],
      },
    });
    // Kept for the next documents.
    expect(loadAutoCorrect()?.smartQuotes).toBe(false);
    expect(onSaved).toHaveBeenCalledWith(loadAutoCorrect());
  });

  it('turning AutoCorrect off disables the rules; defaults forget the kept options', async () => {
    saveAutoCorrect({ ...defaults(), dashes: false });
    const { commands } = setup();
    fireEvent.click(await screen.findByTestId('autocorrect-enabled'));
    expect((screen.getByTestId('autocorrect-dashes') as HTMLInputElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId('autocorrect-reset'));
    await waitFor(() => expect(commands).toContainEqual({ cmd: 'autoCorrect', reset: true }));
    await waitFor(() =>
      expect((screen.getByTestId('autocorrect-enabled') as HTMLInputElement).checked).toBe(true),
    );
    fireEvent.click(screen.getByTestId('autocorrect-ok'));
    await waitFor(() => expect(loadAutoCorrect()).toBeNull());
  });

  it('builds the startup command from kept options', () => {
    expect(autoCorrectCommand(null)).toEqual({ cmd: 'autoCorrect', reset: true });
    const o = defaults();
    expect(autoCorrectCommand(o)).toEqual({ cmd: 'autoCorrect', options: o });
    localStorage.setItem('bo.doc.autocorrect', '{not json');
    expect(loadAutoCorrect()).toBeNull();
  });
});
