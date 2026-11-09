// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type {
  EditCommand,
  FormFieldData,
  FormFieldInfo,
  ProtectionInfo,
} from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  FormFieldBar,
  FormFieldDialog,
  ProtectFormsDialog,
  formErrorKey,
} from '../src/FormFieldDialogs';

afterEach(cleanup);

const i18n = createI18n('vi-VN');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const common = {
  name: 'Text1',
  enabled: true,
  calcOnExit: false,
  entryMacro: null,
  exitMacro: null,
  helpText: null,
  statusText: null,
};

const info = (field: FormFieldData, value = ''): FormFieldInfo => ({
  field,
  value,
  fillable: true,
});

const input = (id: string) => screen.getByTestId(id) as HTMLInputElement;

describe('form field options', () => {
  it('edits a text field: type, length, format, name and hints', () => {
    const onApply = vi.fn();
    wrap(
      <FormFieldDialog
        info={info({
          ...common,
          kind: 'text',
          type: 'regular',
          default: '',
          maxLength: 0,
          format: '',
        })}
        onApply={onApply}
        onClose={() => {}}
      />,
    );
    fireEvent.change(screen.getByTestId('form-text-type'), { target: { value: 'number' } });
    fireEvent.change(input('form-text-max'), { target: { value: '3' } });
    fireEvent.change(input('form-text-format'), { target: { value: '#,##0' } });
    fireEvent.change(input('form-field-name'), { target: { value: '  SoLuong ' } });
    fireEvent.change(input('form-field-status'), { target: { value: 'Nhập số lượng' } });
    fireEvent.click(input('form-field-calc'));
    fireEvent.click(screen.getByTestId('form-field-ok'));
    expect(onApply).toHaveBeenCalledWith({
      cmd: 'setFormField',
      field: {
        ...common,
        name: 'SoLuong',
        calcOnExit: true,
        statusText: { autoText: false, text: 'Nhập số lượng' },
        kind: 'text',
        type: 'number',
        default: '',
        maxLength: 3,
        format: '#,##0',
      },
    });
  });

  it('edits the choices of a list; a selection past the end is dropped', () => {
    const onApply = vi.fn();
    wrap(
      <FormFieldDialog
        info={info({
          ...common,
          name: 'Tinh',
          kind: 'dropDown',
          result: 3,
          default: null,
          entries: ['Hà Nội', 'Huế', 'Đà Nẵng', 'Cần Thơ'],
        })}
        onApply={onApply}
        onClose={() => {}}
      />,
    );
    expect((screen.getByTestId('form-list-entries') as HTMLTextAreaElement).value).toBe(
      'Hà Nội\nHuế\nĐà Nẵng\nCần Thơ',
    );
    fireEvent.change(screen.getByTestId('form-list-entries'), {
      target: { value: 'Hà Nội\n\n  Huế \nĐà Nẵng' },
    });
    fireEvent.click(screen.getByTestId('form-field-ok'));
    const cmd = onApply.mock.calls[0]?.[0] as EditCommand;
    expect(cmd).toMatchObject({
      cmd: 'setFormField',
      field: { kind: 'dropDown', entries: ['Hà Nội', 'Huế', 'Đà Nẵng'], result: null },
    });
  });

  it('sizes a check box in points (stored in half-points)', () => {
    const onApply = vi.fn();
    wrap(
      <FormFieldDialog
        info={info({ ...common, kind: 'checkBox', size: null, default: false, checked: null })}
        onApply={onApply}
        onClose={() => {}}
      />,
    );
    fireEvent.change(screen.getByTestId('form-check-size-mode'), { target: { value: 'exact' } });
    expect(input('form-check-size').value).toBe('10');
    fireEvent.change(input('form-check-size'), { target: { value: '14' } });
    fireEvent.click(input('form-check-default'));
    fireEvent.click(screen.getByTestId('form-field-ok'));
    expect(onApply.mock.calls[0]?.[0]).toMatchObject({
      field: { kind: 'checkBox', size: 28, default: true, checked: null },
    });
  });

  it('shows that macros are kept but not run', () => {
    wrap(
      <FormFieldDialog
        info={info({
          ...common,
          entryMacro: 'VaoHoTen',
          kind: 'text',
          type: 'regular',
          default: '',
          maxLength: 0,
          format: '',
        })}
        onApply={() => {}}
        onClose={() => {}}
      />,
    );
    expect(screen.getByTestId('form-field-macros').textContent).toContain('VaoHoTen');
  });
});

describe('restrict to form filling', () => {
  it('starts with a confirmed password and the sections left free', () => {
    const onApply = vi.fn();
    wrap(
      <ProtectFormsDialog protection={null} sections={2} onApply={onApply} onClose={() => {}} />,
    );
    const ok = screen.getByTestId('protect-forms-ok') as HTMLButtonElement;
    fireEvent.change(input('protect-forms-password'), { target: { value: 'bí mật' } });
    expect(ok.disabled).toBe(true);
    fireEvent.change(input('protect-forms-confirm'), { target: { value: 'bí mật' } });
    expect(ok.disabled).toBe(false);
    fireEvent.click(input('protect-forms-section-1'));
    fireEvent.click(ok);
    expect(onApply).toHaveBeenCalledWith({
      cmd: 'protectDocument',
      edit: 'forms',
      password: 'bí mật',
      unprotectedSections: [1],
    });
  });

  it('stops with the password when one is set', () => {
    const onApply = vi.fn();
    const protection: ProtectionInfo = {
      edit: 'forms',
      enforced: true,
      hasPassword: true,
      unprotectedSections: [],
      sections: 1,
    };
    wrap(
      <ProtectFormsDialog
        protection={protection}
        sections={1}
        onApply={onApply}
        onClose={() => {}}
      />,
    );
    expect(screen.queryByTestId('protect-forms-confirm')).toBeNull();
    fireEvent.change(input('protect-forms-password'), { target: { value: 'x' } });
    fireEvent.click(screen.getByTestId('protect-forms-ok'));
    expect(onApply).toHaveBeenCalledWith({ cmd: 'unprotectDocument', password: 'x' });
  });
});

describe('form field bar', () => {
  const locked: ProtectionInfo = {
    edit: 'forms',
    enforced: true,
    hasPassword: false,
    unprotectedSections: [],
    sections: 1,
  };

  it('picks a list entry, ticks a box and moves between fields', () => {
    const commands: EditCommand[] = [];
    const run = vi.fn(async (c: EditCommand) => {
      commands.push(c);
      return null;
    });
    wrap(
      <FormFieldBar
        info={info(
          {
            ...common,
            name: 'Tinh',
            statusText: { autoText: false, text: 'Chọn tỉnh' },
            kind: 'dropDown',
            result: 1,
            default: null,
            entries: ['Hà Nội', 'Huế', 'Đà Nẵng'],
          },
          'Huế',
        )}
        protection={locked}
        run={run}
      />,
    );
    expect(screen.getByTestId('form-field-status-text').textContent).toBe('Chọn tỉnh');
    fireEvent.change(screen.getByTestId('form-field-choose'), { target: { value: '2' } });
    fireEvent.click(screen.getByTestId('form-field-next'));
    fireEvent.click(screen.getByTestId('form-field-previous'));
    cleanup();
    wrap(
      <FormFieldBar
        info={info({ ...common, kind: 'checkBox', size: null, default: false, checked: true }, '☒')}
        protection={null}
        run={run}
      />,
    );
    // Not restricted: no Tab buttons.
    expect(screen.queryByTestId('form-field-next')).toBeNull();
    fireEvent.click(screen.getByTestId('form-field-toggle'));
    expect(commands).toEqual([
      { cmd: 'setFormFieldValue', value: '2' },
      { cmd: 'nextFormField' },
      { cmd: 'nextFormField', back: true },
      { cmd: 'setFormFieldValue', value: '' },
    ]);
  });

  it('maps engine errors to messages', () => {
    expect(formErrorKey('forms:protected')).toBe('formFields.errors.protected');
    expect(formErrorKey('forms:number')).toBe('formFields.errors.number');
    expect(formErrorKey('invalid position')).toBeNull();
    expect(i18n.t('formFields.errors.password')).not.toBe('formFields.errors.password');
  });
});
