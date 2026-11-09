// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type {
  EditCommand,
  EditResult,
  SignatureLineInfo,
  SignatureReport,
} from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  bytesToSign,
  imagesCommand,
  InsertSignatureLineDialog,
  isoDay,
  lineSignRequest,
  newLineId,
  SignatureLineStatusList,
  SignLineDialog,
  statusCommand,
  unsignedLines,
  validateChoice,
} from '../src/SignatureLineDialogs';

afterEach(cleanup);

const i18n = createI18n('vi-VN');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);
const input = (id: string) => screen.getByTestId(id) as HTMLInputElement;

const line = (
  id: string,
  signer: string,
  extra: Partial<SignatureLineInfo['line']> = {},
): SignatureLineInfo => ({
  para: 0,
  at: 0,
  line: {
    id,
    providerId: '{00000000-0000-0000-0000-000000000000}',
    isSignatureLine: true,
    suggestedSigner: signer,
    suggestedSigner2: '',
    suggestedSignerEmail: '',
    signingInstructions: '',
    allowComments: false,
    showSignDate: true,
    ...extra,
  },
});

describe('signature line helpers', () => {
  it('makes version 4 GUIDs with braces', () => {
    // All-0xFF bytes: the version nibble becomes 4 and the variant bits 10 (RFC 9562 §4.1–4.2).
    const id = newLineId((n) => new Uint8Array(n).fill(0xff));
    expect(id).toBe('{FFFFFFFF-FFFF-4FFF-BFFF-FFFFFFFFFFFF}');
    expect(newLineId()).toMatch(
      /^\{[0-9A-F]{8}-[0-9A-F]{4}-4[0-9A-F]{3}-[89AB][0-9A-F]{3}-[0-9A-F]{12}\}$/,
    );
  });

  it('lists unsigned lines and validates the choice', () => {
    const signed = line('{2}', 'B', { signed: { text: 'B', date: '', valid: null } });
    expect(unsignedLines([line('{1}', 'A'), signed]).map((l) => l.line.id)).toEqual(['{1}']);
    expect(validateChoice({ id: '', text: 'A', picture: null, comments: '' })).toBe(
      'sigline.needLine',
    );
    expect(validateChoice({ id: '{1}', text: '  ', picture: null, comments: '' })).toBe(
      'sigline.needName',
    );
    expect(
      validateChoice({
        id: '{1}',
        text: '',
        picture: { name: 'k.png', base64: 'AA==' },
        comments: '',
      }),
    ).toBeNull();
  });

  it('builds the engine and signing requests', () => {
    const c = { id: '{1}', text: ' Nguyễn Văn A ', picture: null, comments: ' Đồng ý ' };
    expect(imagesCommand(c, '2026-11-09')).toEqual({
      cmd: 'signatureLineImages',
      id: '{1}',
      text: 'Nguyễn Văn A',
      imageBase64: null,
      date: '2026-11-09',
    });
    expect(lineSignRequest(c, { valid: 'V', invalid: 'I' })).toEqual({
      id: '{1}',
      text: 'Nguyễn Văn A',
      imageBase64: undefined,
      comments: 'Đồng ý',
      validImageBase64: 'V',
      invalidImageBase64: 'I',
    });
    // [MS-OFFCRYPTO] SignatureText is at most 100 characters.
    expect(
      lineSignRequest({ ...c, text: 'x'.repeat(150) }, { valid: '', invalid: '' }).text,
    ).toHaveLength(100);
    expect(isoDay(new Date(2026, 0, 5))).toBe('2026-01-05');
  });

  it('signs the opened package while unchanged, else the saved one', async () => {
    const opened = new Uint8Array([0x50, 0x4b, 3, 4]);
    const saved = new Uint8Array([0x50, 0x4b, 9, 9]);
    const save = vi.fn(() => Promise.resolve(saved));
    expect(await bytesToSign(false, opened, save)).toBe(opened);
    expect(save).not.toHaveBeenCalled();
    expect(await bytesToSign(true, opened, save)).toBe(saved);
    expect(await bytesToSign(false, null, save)).toBe(saved);
    // A legacy .doc (not a ZIP package) is converted by saving.
    expect(await bytesToSign(false, new Uint8Array([0xd0, 0xcf]), save)).toBe(saved);
  });

  it('turns a report into the status command', () => {
    const report: SignatureReport = {
      kind: 'ooxml',
      signatures: [],
      error: 'changed',
      signatureLines: [
        { id: '{1}', valid: false, signer: null, text: null, error: 'changed' },
        { id: '{2}', valid: true, signer: 'CN=B', text: 'B', error: null },
      ],
    };
    expect(statusCommand(report)).toEqual({
      cmd: 'setSignatureLineStatus',
      lines: [
        { id: '{1}', valid: false },
        { id: '{2}', valid: true },
      ],
    });
    expect(statusCommand({ ...report, signatureLines: undefined })).toBeNull();
    expect(statusCommand(null)).toBeNull();
  });
});

describe('insert signature line dialog', () => {
  it('collects the line settings', () => {
    const onInsert = vi.fn();
    wrap(<InsertSignatureLineDialog onInsert={onInsert} onClose={() => {}} />);
    fireEvent.change(input('sigline-signer'), { target: { value: 'Nguyễn Văn A' } });
    fireEvent.change(input('sigline-title'), { target: { value: 'Giám đốc' } });
    fireEvent.change(input('sigline-email'), { target: { value: 'a@example.vn' } });
    fireEvent.change(screen.getByTestId('sigline-instructions'), {
      target: { value: 'Ký rõ họ tên' },
    });
    fireEvent.click(input('sigline-allow-comments'));
    fireEvent.click(input('sigline-show-date'));
    fireEvent.click(screen.getByTestId('sigline-insert-ok'));
    expect(onInsert).toHaveBeenCalledWith({
      suggestedSigner: 'Nguyễn Văn A',
      suggestedSigner2: 'Giám đốc',
      suggestedSignerEmail: 'a@example.vn',
      signingInstructions: 'Ký rõ họ tên',
      allowComments: true,
      showSignDate: false,
    });
  });
});

describe('sign a signature line dialog', () => {
  const lines = [
    line('{1}', 'Nguyễn Văn A', { signed: { text: 'A', date: '2026-11-09', valid: null } }),
    line('{2}', 'Trần Thị B', { allowComments: true, signingInstructions: 'Ký và đóng dấu' }),
    line('{3}', ''),
  ];
  const run = vi.fn((c: EditCommand) =>
    Promise.resolve(
      c.cmd === 'signatureLines' ? ({ signatureLines: lines } as unknown as EditResult) : null,
    ),
  );

  it('offers the unsigned lines, a typed name or a picture, and a comment', async () => {
    const onContinue = vi.fn();
    const pickPicture = vi.fn(() =>
      Promise.resolve({ name: 'chu-ky.png', bytes: new Uint8Array([1, 2, 3]) }),
    );
    wrap(
      <SignLineDialog
        run={run}
        pickPicture={pickPicture}
        onContinue={onContinue}
        onClose={() => {}}
      />,
    );
    const select = (await screen.findByTestId('sigline-line')) as HTMLSelectElement;
    // The signed line is not offered; the unnamed one has a placeholder label.
    expect(Array.from(select.options).map((o) => o.value)).toEqual(['{2}', '{3}']);
    expect(select.options[1]!.textContent).toContain('chưa có người ký đề xuất');
    expect(screen.getByTestId('sigline-instructions-text').textContent).toBe('Ký và đóng dấu');
    // The suggested signer's name is proposed.
    expect(input('sigline-name').value).toBe('Trần Thị B');
    fireEvent.change(input('sigline-comments'), { target: { value: 'Đã xem' } });
    fireEvent.click(screen.getByTestId('sigline-pick-picture'));
    await waitFor(() =>
      expect(screen.getByTestId('sigline-picture-name').textContent).toBe('chu-ky.png'),
    );
    fireEvent.click(screen.getByTestId('sigline-sign-next'));
    expect(onContinue).toHaveBeenCalledWith({
      id: '{2}',
      text: 'Trần Thị B',
      // base64 of the bytes 1, 2, 3 (RFC 4648 §4).
      picture: { name: 'chu-ky.png', base64: 'AQID' },
      comments: 'Đã xem',
    });
  });

  it('needs a name or a picture; comments only where allowed', async () => {
    wrap(
      <SignLineDialog
        run={run}
        pickPicture={() => Promise.resolve(null)}
        onContinue={() => {}}
        onClose={() => {}}
      />,
    );
    const select = (await screen.findByTestId('sigline-line')) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: '{3}' } });
    expect(input('sigline-name').value).toBe('');
    expect(screen.queryByTestId('sigline-comments')).toBeNull();
    expect((screen.getByTestId('sigline-sign-next') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId('sigline-problem').textContent).toBe(
      'Hãy nhập tên hoặc chọn ảnh chữ ký.',
    );
  });

  it('says when nothing is left to sign', async () => {
    const none = vi.fn(() =>
      Promise.resolve({ signatureLines: [lines[0]] } as unknown as EditResult),
    );
    wrap(
      <SignLineDialog
        run={none}
        pickPicture={() => Promise.resolve(null)}
        onContinue={() => {}}
        onClose={() => {}}
      />,
    );
    expect(await screen.findByTestId('sigline-none')).toBeTruthy();
  });
});

describe('signature line status', () => {
  it('shows each line valid or invalid', () => {
    wrap(
      <SignatureLineStatusList
        lines={[
          { id: '{1}', valid: true, signer: 'CN=Nguyễn Văn A,O=X', text: '', error: null },
          {
            id: '{2}',
            valid: false,
            signer: null,
            text: null,
            error: '/word/document.xml was modified',
          },
        ]}
      />,
    );
    const items = screen.getAllByTestId('sigline-status-item');
    expect(items.map((i) => i.getAttribute('data-valid'))).toEqual(['true', 'false']);
    expect(items[0]!.textContent).toContain('Nguyễn Văn A');
    expect(items[1]!.textContent).toContain('was modified');
  });
});
