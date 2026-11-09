// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { InspectReport, SheetCommand } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SheetCheckDocumentDialog, sheetCheckBridge } from '../src/inspect';

afterEach(cleanup);

const report: InspectReport = {
  inspector: [{ category: 'hiddenRows', count: 2, details: [], action: 'remove', pending: false }],
  accessibility: [
    {
      rule: 'mergedCells',
      severity: 'warning',
      target: { kind: 'range', sheet: 1, r0: 2, c0: 0, r1: 2, c1: 3 },
      detail: 'A3:D3',
    },
    { rule: 'sheetName', severity: 'tip', target: { kind: 'sheet', sheet: 0 }, detail: 'Sheet1' },
  ],
  issueCount: 2,
  compatibility: [],
};

describe('BlueSheet check document', () => {
  it('runs the sheet commands and goes to ranges and sheets', async () => {
    const sent: SheetCommand[] = [];
    const exec = async <T,>(c: SheetCommand): Promise<T | null> => {
      sent.push(c);
      return report as T;
    };
    const goTo = vi.fn();
    const b = sheetCheckBridge(exec, goTo);
    await b.inspect();
    await b.remove(['hiddenRows']);
    await b.keep(['macros']);
    expect(sent).toEqual([
      { cmd: 'inspect' },
      { cmd: 'inspectRemove', categories: ['hiddenRows'] },
      { cmd: 'inspectKeep', categories: ['macros'] },
    ]);
    expect(b.fix).toBeUndefined();
    b.goTo?.({ kind: 'sheet', sheet: 0 });
    expect(goTo).toHaveBeenCalledWith({ sheet: 0, r0: 0, c0: 0, r1: 0, c1: 0 });
  });

  it('shows the report and selects the merged range', async () => {
    const sent: SheetCommand[] = [];
    const exec = async <T,>(c: SheetCommand): Promise<T | null> => {
      sent.push(c);
      return report as T;
    };
    const goTo = vi.fn();
    const onClose = vi.fn();
    render(
      <I18nextProvider i18n={createI18n('vi-VN')}>
        <SheetCheckDocumentDialog exec={exec} goTo={goTo} onClose={onClose} />
      </I18nextProvider>,
    );
    expect((await screen.findByTestId('inspect-cat-hiddenRows')).textContent).toContain('Hàng ẩn');
    fireEvent.click(screen.getByTestId('inspect-remove-hiddenRows'));
    await waitFor(() =>
      expect(sent).toContainEqual({ cmd: 'inspectRemove', categories: ['hiddenRows'] }),
    );
    fireEvent.click(screen.getByTestId('inspect-tab-accessibility'));
    fireEvent.click(screen.getAllByTestId('inspect-goto')[0] as HTMLElement);
    expect(goTo).toHaveBeenCalledWith({ kind: 'range', sheet: 1, r0: 2, c0: 0, r1: 2, c1: 3 });
    expect(onClose).toHaveBeenCalled();
    // One report request when the dialog opens.
    expect(sent.filter((c) => c.cmd === 'inspect')).toHaveLength(1);
  });
});
