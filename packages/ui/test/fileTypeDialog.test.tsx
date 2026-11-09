// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FileTypeDialog } from '../src';

afterEach(cleanup);

const labels = { title: 'Lưu thành', type: 'Kiểu tệp', ok: 'Lưu', cancel: 'Hủy' };
const options = [
  { value: 'docx', label: 'Tài liệu (.docx)' },
  { value: 'docm', label: 'Tài liệu có macro (.docm)' },
];

describe('FileTypeDialog', () => {
  it('preselects, warns for the chosen type and confirms it', () => {
    const onConfirm = vi.fn();
    render(
      <FileTypeDialog
        open
        options={options}
        initial="docm"
        labels={labels}
        warning={(v) => (v === 'docx' ? 'Macro sẽ bị bỏ' : null)}
        onConfirm={onConfirm}
        onCancel={() => {}}
      />,
    );
    expect((screen.getByTestId('file-type-docm') as HTMLInputElement).checked).toBe(true);
    expect(screen.queryByTestId('file-type-warning')).toBeNull();
    fireEvent.click(screen.getByTestId('file-type-docx'));
    expect(screen.getByTestId('file-type-warning').textContent).toBe('Macro sẽ bị bỏ');
    fireEvent.click(screen.getByTestId('file-type-ok'));
    expect(onConfirm).toHaveBeenCalledWith('docx');
  });

  it('cancels', () => {
    const onCancel = vi.fn();
    render(
      <FileTypeDialog
        open
        options={options}
        initial="docx"
        labels={labels}
        onConfirm={() => {}}
        onCancel={onCancel}
      />,
    );
    fireEvent.click(screen.getByTestId('file-type-cancel'));
    expect(onCancel).toHaveBeenCalled();
  });
});
