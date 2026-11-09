// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { StatusBar, TitleBar } from '../src';

afterEach(cleanup);

describe('TitleBar', () => {
  it('shows the app mark, document, save state and quick-access commands', () => {
    const save = vi.fn();
    render(
      <TitleBar
        app="sheet"
        appName="BlueSheet"
        docName="Báo cáo.xlsx"
        saveState="unsaved"
        saveLabel="Chưa lưu"
        quickAccessLabel="Thanh truy cập nhanh"
        quickAccess={[
          { id: 'file.save', label: 'Lưu', shortcut: 'Ctrl+S', onSelect: save },
          { id: 'edit.undo', label: 'Hoàn tác', enabled: false, onSelect: vi.fn() },
        ]}
      />,
    );
    const bar = screen.getByTestId('title-bar');
    expect(bar.tagName).toBe('HEADER');
    expect(bar.querySelector('[data-icon="app.sheet"]')).not.toBeNull();
    expect(screen.getByTestId('doc-name').textContent).toBe('Báo cáo.xlsx');
    expect(screen.getByTestId('save-state').getAttribute('data-state')).toBe('unsaved');
    const qat = screen.getByRole('toolbar', { name: 'Thanh truy cập nhanh' });
    const [saveBtn, undoBtn] = Array.from(qat.querySelectorAll('button'));
    expect(saveBtn!.getAttribute('title')).toBe('Lưu (Ctrl+S)');
    fireEvent.click(saveBtn!);
    expect(save).toHaveBeenCalledOnce();
    expect((undoBtn as HTMLButtonElement).disabled).toBe(true);
  });

  it('hides the save state without a document and renders extra actions', () => {
    render(
      <TitleBar
        app="doc"
        appName="BlueDoc"
        docName=""
        saveState="saved"
        saveLabel="Đã lưu"
        quickAccessLabel="QAT"
        actions={<button type="button">Chia sẻ</button>}
        search={{ label: 'Tìm kiếm', shortcut: 'Alt+Q', onOpen: vi.fn() }}
      />,
    );
    expect(screen.queryByTestId('save-state')).toBeNull();
    expect(screen.queryByRole('toolbar')).toBeNull();
    expect(screen.getByRole('button', { name: 'Chia sẻ' })).toBeTruthy();
    expect(screen.getByTestId('title-search').getAttribute('aria-keyshortcuts')).toBe('Alt+Q');
  });
});

describe('StatusBar', () => {
  it('is a 24px status landmark', () => {
    render(<StatusBar left={<span>1 trang</span>} />);
    const bar = screen.getByTestId('status-bar');
    expect(bar.getAttribute('role')).toBe('status');
    expect(bar.style.height).toBe('24px');
  });
});
