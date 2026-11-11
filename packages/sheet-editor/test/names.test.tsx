// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { DefinedNameInfo, SheetCommand, SheetHost } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CreateNamesDialog, NameManagerDialog, absoluteRef } from '../src/names';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (ui: ReactElement) => render(<I18nextProvider i18n={i18n}>{ui}</I18nextProvider>);

function fakeHost(names: DefinedNameInfo[]) {
  const calls: SheetCommand[] = [];
  const host = {
    exec: vi.fn(async (cmd: SheetCommand) => {
      calls.push(cmd);
      if (cmd.cmd === 'nameList') return names;
      if (cmd.cmd === 'defineName' && cmd.name === 'A1') throw new Error('invalid name');
      if (cmd.cmd === 'createNames') return { created: ['Price'] };
      return { ok: true };
    }),
  } as unknown as SheetHost;
  return { host, calls };
}

describe('absoluteRef', () => {
  it('writes the selection as an absolute, sheet-qualified reference', () => {
    expect(absoluteRef('Sheet1', { r0: 1, c0: 1, r1: 3, c1: 1 })).toBe('=Sheet1!$B$2:$B$4');
    expect(absoluteRef('Bảng 1', { r0: 0, c0: 0, r1: 0, c1: 0 })).toBe("='Bảng 1'!$A$1");
    expect(absoluteRef("O'Neil", { r0: 0, c0: 26, r1: 0, c1: 26 })).toBe("='O''Neil'!$AA$1");
  });
});

describe('NameManagerDialog', () => {
  const tax: DefinedNameInfo = { name: 'Tax', refersTo: '=0.1', scope: null, comment: 'VAT' };

  it('lists the names, adds one, refuses a bad one and deletes', async () => {
    const { host, calls } = fakeHost([tax]);
    const onDone = vi.fn();
    wrap(
      <NameManagerDialog
        host={host}
        onClose={() => {}}
        onDone={onDone}
        fail={(e) => {
          throw e;
        }}
        sheets={['Sheet1']}
        refersTo="=Sheet1!$A$1:$A$3"
      />,
    );
    await waitFor(() => expect(screen.getByText('VAT')).toBeTruthy());
    expect(screen.getByText('Workbook')).toBeTruthy();
    // New: the selection is proposed as the reference.
    fireEvent.click(screen.getByTestId('name-manager-new'));
    expect((screen.getByTestId('name-edit-ref') as HTMLInputElement).value).toBe(
      '=Sheet1!$A$1:$A$3',
    );
    fireEvent.change(screen.getByTestId('name-edit-name'), { target: { value: 'A1' } });
    fireEvent.click(screen.getByTestId('name-edit-ok'));
    await waitFor(() =>
      expect(screen.getByTestId('name-edit-error').textContent).toMatch(/Invalid name/),
    );
    fireEvent.change(screen.getByTestId('name-edit-name'), { target: { value: 'Rows' } });
    fireEvent.change(screen.getByTestId('name-edit-scope'), { target: { value: '0' } });
    fireEvent.click(screen.getByTestId('name-edit-ok'));
    await waitFor(() =>
      expect(calls).toContainEqual({
        cmd: 'defineName',
        name: 'Rows',
        refersTo: '=Sheet1!$A$1:$A$3',
        scope: 0,
        comment: '',
      }),
    );
    // Delete the picked name.
    await waitFor(() => expect(screen.getByTestId('name-manager')).toBeTruthy());
    fireEvent.click(screen.getByText('Tax'));
    fireEvent.click(screen.getByTestId('name-manager-delete'));
    await waitFor(() =>
      expect(calls).toContainEqual({ cmd: 'deleteName', name: 'Tax', scope: null }),
    );
    expect(onDone).toHaveBeenCalled();
  });

  it('edits a name in place (the old name is replaced)', async () => {
    const { host, calls } = fakeHost([tax]);
    wrap(
      <NameManagerDialog
        host={host}
        onClose={() => {}}
        onDone={() => {}}
        fail={() => {}}
        sheets={['Sheet1']}
        refersTo="=Sheet1!$A$1"
      />,
    );
    await waitFor(() => expect(screen.getByText('Tax')).toBeTruthy());
    fireEvent.click(screen.getByText('Tax'));
    fireEvent.click(screen.getByTestId('name-manager-edit'));
    fireEvent.change(screen.getByTestId('name-edit-ref'), { target: { value: '=0.2' } });
    fireEvent.click(screen.getByTestId('name-edit-ok'));
    await waitFor(() =>
      expect(calls).toContainEqual({
        cmd: 'defineName',
        name: 'Tax',
        refersTo: '=0.2',
        scope: null,
        comment: 'VAT',
        old: { name: 'Tax', scope: null },
      }),
    );
  });
});

describe('CreateNamesDialog', () => {
  it('starts from the guessed sides and sends the ticked ones', async () => {
    const { host, calls } = fakeHost([]);
    const onCreated = vi.fn();
    wrap(
      <CreateNamesDialog
        host={host}
        sheet={0}
        range={{ r0: 0, c0: 0, r1: 3, c1: 1 }}
        guess={{ top: true, left: true }}
        onClose={() => {}}
        onDone={() => {}}
        onCreated={onCreated}
        fail={() => {}}
      />,
    );
    expect((screen.getByTestId('create-names-top') as HTMLInputElement).checked).toBe(true);
    fireEvent.click(screen.getByTestId('create-names-left'));
    fireEvent.click(screen.getByTestId('create-names-ok'));
    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(['Price']));
    expect(calls).toContainEqual({
      cmd: 'createNames',
      sheet: 0,
      range: { r0: 0, c0: 0, r1: 3, c1: 1 },
      top: true,
      left: false,
      bottom: false,
      right: false,
    });
  });
});
