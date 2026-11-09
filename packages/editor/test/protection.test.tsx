// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { configuredLabels, type SecurityCommand, type SecurityInfo } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  identityNames,
  OpenProtectionDialog,
  ProtectDialog,
  ProtectionBar,
  ProtectionStatus,
  refusalKey,
} from '../src/ProtectionPanels';

afterEach(cleanup);
beforeEach(() => localStorage.clear());

const i18n = createI18n('vi-VN');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const info = (patch: Partial<SecurityInfo> = {}): SecurityInfo => ({
  markedFinal: false,
  readOnlyRecommended: false,
  writePassword: false,
  readOnly: false,
  label: null,
  foreignLabels: [],
  showRegions: false,
  identity: [],
  ...patch,
});

describe('protection UI', () => {
  it('final notice bar offers "edit anyway"', () => {
    const onEdit = vi.fn();
    wrap(<ProtectionBar info={info({ markedFinal: true, readOnly: true })} onEdit={onEdit} />);
    expect(screen.getByTestId('protection-bar').textContent).toContain('bản cuối');
    fireEvent.click(screen.getByTestId('protection-edit'));
    expect(onEdit).toHaveBeenCalled();
    cleanup();
    // Not read-only: no bar.
    wrap(<ProtectionBar info={info()} onEdit={onEdit} />);
    expect(screen.queryByTestId('protection-bar')).toBeNull();
  });

  it('asks for the password to modify; a wrong one is reported', async () => {
    const sent: SecurityCommand[] = [];
    const run = async (c: SecurityCommand) => {
      sent.push(c);
      return c.cmd === 'unlockWrite' && c.password !== 'đúng'
        ? { error: 'wrong protection password' }
        : { security: info({ writePassword: true }) };
    };
    const onDone = vi.fn();
    wrap(
      <OpenProtectionDialog
        info={info({ writePassword: true, readOnly: true })}
        name="a.docx"
        run={run}
        onDone={onDone}
      />,
    );
    expect(screen.getByTestId('open-protection').textContent).toContain('a.docx');
    fireEvent.change(screen.getByTestId('open-write-password'), { target: { value: 'sai' } });
    fireEvent.click(screen.getByTestId('open-edit'));
    expect(await screen.findByRole('alert')).toBeTruthy();
    expect(screen.getByRole('alert').textContent).toBe('Sai mật khẩu.');
    fireEvent.change(screen.getByTestId('open-write-password'), { target: { value: 'đúng' } });
    fireEvent.click(screen.getByTestId('open-edit'));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(sent).toEqual([
      { cmd: 'unlockWrite', password: 'sai' },
      { cmd: 'unlockWrite', password: 'đúng' },
    ]);
  });

  it('read-only recommended: view only or edit', async () => {
    const run = vi.fn(async () => ({ security: info() }));
    const onDone = vi.fn();
    wrap(
      <OpenProtectionDialog
        info={info({ readOnlyRecommended: true, readOnly: true })}
        name="b.xlsx"
        run={run}
        onDone={onDone}
      />,
    );
    fireEvent.click(screen.getByTestId('open-edit'));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(run).toHaveBeenCalledWith({ cmd: 'setReadOnly', on: false });
  });

  it('protect dialog sends final mark, label and restricted editing commands', async () => {
    const sent: SecurityCommand[] = [];
    let state = info();
    const run = async (c: SecurityCommand) => {
      sent.push(c);
      if (c.cmd === 'markFinal') state = { ...state, markedFinal: c.on, readOnly: c.on };
      if (c.cmd === 'restrictEditing')
        state = { ...state, protection: { edit: 'readOnly', enforced: true, hasPassword: true } };
      if (c.cmd === 'setLabel') state = { ...state, label: c.label ?? null };
      return { security: state };
    };
    wrap(<ProtectDialog app="doc" info={state} run={run} onClose={() => {}} />);
    fireEvent.change(screen.getByTestId('protect-label'), { target: { value: 'Nội bộ' } });
    await waitFor(() => expect(sent.length).toBe(1));
    expect(sent[0]).toEqual({
      cmd: 'setLabel',
      label: { id: 'internal', name: 'Nội bộ', marking: 'TÀI LIỆU NỘI BỘ' },
    });
    fireEvent.click(screen.getByTestId('protect-region-everyone'));
    fireEvent.change(screen.getByTestId('protect-doc-password'), { target: { value: 'k' } });
    fireEvent.click(screen.getByTestId('protect-start'));
    await waitFor(() => expect(screen.getByTestId('protect-stop')).toBeTruthy());
    fireEvent.click(screen.getByTestId('protect-final'));
    await waitFor(() =>
      expect(sent.map((c) => c.cmd)).toEqual([
        'setLabel',
        'addEditableRegion',
        'restrictEditing',
        'markFinal',
      ]),
    );
    expect(sent[2]).toEqual({ cmd: 'restrictEditing', password: 'k' });
  });

  it('sheet dialog manages protected ranges', async () => {
    const sent: SecurityCommand[] = [];
    const run = async (c: SecurityCommand) => {
      sent.push(c);
      return {
        security: info(),
        ranges: [
          {
            name: 'Nhập',
            sqref: 'B2:C3',
            hasPassword: true,
            hasSecurityDescriptor: false,
            unlocked: false,
          },
        ],
      };
    };
    wrap(
      <ProtectDialog
        app="sheet"
        info={info()}
        run={run}
        sheet={0}
        selectionRef="B2:C3"
        onClose={() => {}}
      />,
    );
    // Workbooks offer the read-only recommendation.
    expect(screen.getByTestId('protect-recommended')).toBeTruthy();
    fireEvent.change(screen.getByPlaceholderText('Tên vùng'), { target: { value: 'Nhập' } });
    fireEvent.click(screen.getByTestId('protect-range-add'));
    await waitFor(() => expect(screen.getByTestId('protect-ranges')).toBeTruthy());
    expect(sent[0]).toEqual({
      cmd: 'addProtectedRange',
      sheet: 0,
      name: 'Nhập',
      sqref: 'B2:C3',
      password: '',
    });
  });

  it('presentations have no read-only recommendation', () => {
    wrap(<ProtectDialog app="slide" info={info()} run={async () => null} onClose={() => {}} />);
    expect(screen.queryByTestId('protect-recommended')).toBeNull();
  });

  it('status bar shows the label: our own, else an applied foreign one', () => {
    wrap(
      <ProtectionStatus
        info={info({
          foreignLabels: [{ id: 'x', name: 'Restricted-Test', enabled: true }],
          readOnly: true,
        })}
      />,
    );
    expect(screen.getByTestId('status-label').textContent).toBe('Nhãn: Restricted-Test');
    expect(screen.getByTestId('status-readonly')).toBeTruthy();
    cleanup();
    wrap(<ProtectionStatus info={info({ label: { name: 'Mật' } })} />);
    expect(screen.getByTestId('status-label').textContent).toBe('Nhãn: Mật');
  });

  it('identity, refusal messages and the configured label list', () => {
    expect(identityNames(null)).toEqual([]);
    expect(identityNames({ id: 'an@bluestar.vn', name: 'Nguyễn An' })).toEqual([
      'Nguyễn An',
      'an@bluestar.vn',
    ]);
    expect(identityNames({ id: '42', name: 'An' })).toEqual(['An']);
    expect(refusalKey('read only')).toBe('protect.err.readOnly');
    expect(refusalKey('document protected')).toBe('protect.err.documentProtected');
    const defaults = [{ name: 'A' }];
    expect(configuredLabels(defaults)).toBe(defaults);
    localStorage.setItem('blueoffice.labels', JSON.stringify([{ id: 'k', name: 'Khách hàng' }]));
    expect(configuredLabels(defaults)).toEqual([{ id: 'k', name: 'Khách hàng' }]);
  });
});
