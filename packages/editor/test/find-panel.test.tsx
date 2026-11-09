// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { EditCommand, EditResult } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FindPanel } from '../src/Panels';

afterEach(cleanup);

const i18n = createI18n('vi-VN');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const result = (patch: Partial<EditResult>): EditResult =>
  ({ rects: [], dirty: [], ...patch }) as EditResult;

function setup() {
  const commands: EditCommand[] = [];
  const run = vi.fn(async (c: EditCommand) => {
    commands.push(c);
    if (c.cmd === 'find')
      return result({
        matches: [{ p: 0, start: 0, end: 3, ep: 1 }],
        ...(c.highlight ? { highlights: [{ page: 0, x: 1, y: 2, w: 3, h: 4 }] } : {}),
      });
    if (c.cmd === 'replaceAll') return result({ count: 2 });
    return result({});
  });
  const clear = vi.fn();
  const view = wrap(<FindPanel run={run} replace onClose={() => {}} onClearHighlights={clear} />);
  return { commands, clear, view };
}

describe('FindPanel', () => {
  it('sends wildcard options and inserts special characters', async () => {
    const { commands } = setup();
    fireEvent.change(screen.getByTestId('find-query'), { target: { value: '(<*>)' } });
    fireEvent.click(screen.getByTestId('find-wildcards'));
    // Wildcards switch off case / whole-word choices.
    expect((screen.getByTestId('find-matchCase') as HTMLInputElement).disabled).toBe(true);
    fireEvent.change(screen.getByTestId('find-special'), { target: { value: '^p' } });
    expect((screen.getByTestId('find-query') as HTMLInputElement).value).toBe('(<*>)^p');
    // "^&" is only offered for the replacement.
    const special = screen.getByTestId('find-special');
    expect(special.querySelector('option[value="^&"]')).toBeNull();
    fireEvent.focus(screen.getByTestId('find-replace'));
    fireEvent.change(special, { target: { value: '^&' } });
    expect((screen.getByTestId('find-replace') as HTMLInputElement).value).toBe('^&');
    fireEvent.click(screen.getByText('Thay tất cả'));
    await waitFor(() =>
      expect(commands).toContainEqual({
        cmd: 'replaceAll',
        query: '(<*>)^p',
        options: { wildcards: true, regex: false },
        with: '^&',
      }),
    );
    // Regular expressions and wildcards exclude each other.
    fireEvent.click(screen.getByTestId('find-regex'));
    expect((screen.getByTestId('find-wildcards') as HTMLInputElement).checked).toBe(false);
  });

  it('highlights all matches and clears them when toggled off or closed', async () => {
    const { commands, clear, view } = setup();
    fireEvent.change(screen.getByTestId('find-query'), { target: { value: 'hà' } });
    const btn = screen.getByTestId('find-highlight-all');
    fireEvent.click(btn);
    await waitFor(() =>
      expect(commands).toContainEqual({ cmd: 'find', query: 'hà', options: {}, highlight: true }),
    );
    await waitFor(() => expect(btn.getAttribute('aria-pressed')).toBe('true'));
    expect(screen.getByTestId('find-message').textContent).toBe('Đã tô sáng 1 kết quả');
    // Find next keeps the highlights fresh; a replace carries the query for group expansion.
    fireEvent.click(screen.getByText('Tìm tiếp'));
    await waitFor(() =>
      expect(commands).toContainEqual({
        cmd: 'selectMatch',
        match: { p: 0, start: 0, end: 3, ep: 1 },
      }),
    );
    expect(commands.filter((c) => c.cmd === 'find' && c.highlight).length).toBe(2);
    fireEvent.click(screen.getByText('Thay'));
    await waitFor(() =>
      expect(commands).toContainEqual({
        cmd: 'replace',
        match: { p: 0, start: 0, end: 3, ep: 1 },
        with: '',
        query: 'hà',
        options: {},
      }),
    );
    fireEvent.click(btn);
    expect(clear).toHaveBeenCalledTimes(1);
    view.unmount();
    expect(clear).toHaveBeenCalledTimes(2);
  });
});
