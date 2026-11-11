// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { Platform, SheetCommand, SheetHost } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { EditLinksDialog, type ExternalLinkInfo } from '../src/links';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (ui: ReactElement) => render(<I18nextProvider i18n={i18n}>{ui}</I18nextProvider>);

const LINKS: ExternalLinkInfo[] = [
  {
    index: 0,
    source: 'Prices.xlsx',
    target: 'C:\\Data\\Prices.xlsx',
    sheets: ['P'],
    formulas: 3,
    workbook: true,
  },
  {
    index: 1,
    source: 'Stock.xlsx',
    target: 'Stock.xlsx',
    sheets: ['S'],
    formulas: 1,
    workbook: true,
  },
];

function setup(missing: string[] = []) {
  const calls: SheetCommand[] = [];
  let links = [...LINKS];
  const host = {
    exec: vi.fn(async (cmd: SheetCommand) => {
      calls.push(cmd);
      if (cmd.cmd === 'externalLinks') return { links };
      if (cmd.cmd === 'externalLinkUpdate') return { missing };
      if (cmd.cmd === 'externalLinkBreak') {
        links = links.slice(1).map((l, i) => ({ ...l, index: i }));
        return { converted: 3 };
      }
      return {};
    }),
  } as unknown as SheetHost;
  const openFile = vi.fn(async () => ({ name: 'New.xlsx', bytes: new Uint8Array([1, 2, 3]) }));
  const platform = { files: { openFile } } as unknown as Platform;
  const onDone = vi.fn();
  wrap(
    <EditLinksDialog
      host={host}
      platform={platform}
      onClose={() => {}}
      onDone={onDone}
      fail={(e) => {
        throw e;
      }}
    />,
  );
  return { calls, openFile, onDone };
}

describe('EditLinksDialog', () => {
  it('lists the links with their location', async () => {
    setup();
    await waitFor(() => expect(screen.getByText('Prices.xlsx')).toBeTruthy());
    expect(screen.getByText('Stock.xlsx')).toBeTruthy();
    expect(screen.getByTestId('edit-links-location').textContent).toContain(
      'C:\\Data\\Prices.xlsx',
    );
    expect(screen.getByTestId('edit-links-status-0').textContent).toBe('Unknown');
    // Picking another row shows its location.
    fireEvent.click(screen.getByText('Stock.xlsx'));
    expect(screen.getByTestId('edit-links-location').textContent).toContain('Stock.xlsx');
  });

  it('updates values from a picked file and changes the source', async () => {
    const { calls, openFile, onDone } = setup();
    await waitFor(() => expect(screen.getByText('Prices.xlsx')).toBeTruthy());
    fireEvent.click(screen.getByTestId('edit-links-update'));
    await waitFor(() => expect(screen.getByTestId('edit-links-status-0').textContent).toBe('OK'));
    expect(openFile).toHaveBeenCalled();
    const upd = calls.find((c) => c.cmd === 'externalLinkUpdate');
    expect(upd).toEqual({
      cmd: 'externalLinkUpdate',
      index: 0,
      base64: 'AQID',
      name: 'New.xlsx',
      changeSource: false,
    });
    expect(onDone).toHaveBeenCalled();
    fireEvent.click(screen.getByTestId('edit-links-change'));
    await waitFor(() =>
      expect(
        calls.some((c) => c.cmd === 'externalLinkUpdate' && 'changeSource' in c && c.changeSource),
      ).toBe(true),
    );
  });

  it('reports a sheet missing from the source', async () => {
    setup(['P']);
    await waitFor(() => expect(screen.getByText('Prices.xlsx')).toBeTruthy());
    fireEvent.click(screen.getByTestId('edit-links-update'));
    await waitFor(() =>
      expect(screen.getByTestId('edit-links-status-0').textContent).toBe(
        'Sheet not found in source',
      ),
    );
  });

  it('breaks a link after confirmation', async () => {
    const { calls } = setup();
    await waitFor(() => expect(screen.getByText('Prices.xlsx')).toBeTruthy());
    fireEvent.click(screen.getByTestId('edit-links-break'));
    expect(calls.some((c) => c.cmd === 'externalLinkBreak')).toBe(false);
    fireEvent.click(screen.getByTestId('break-link-ok'));
    await waitFor(() => expect(screen.queryByText('Prices.xlsx')).toBeNull());
    expect(calls).toContainEqual({ cmd: 'externalLinkBreak', index: 0 });
  });
});
