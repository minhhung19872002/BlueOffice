// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  Backstage,
  BackstageFiles,
  BackstageFormats,
  BackstageProperties,
  backstagePage,
  loadPinned,
  navTarget,
  Ribbon,
  savePinned,
  togglePinned,
  type BackstageSection,
} from '../src';

afterEach(cleanup);

const labels = { title: 'Tệp', back: 'Quay lại' };

function sections(save = vi.fn()): BackstageSection[] {
  return [
    { id: 'new', label: 'Mới', content: <p>new page</p> },
    { id: 'info', label: 'Thông tin', content: <p>info page</p> },
    { id: 'save', label: 'Lưu', command: 'file.save', onSelect: save },
    { id: 'print', label: 'In', enabled: false, content: <p>print page</p> },
    { id: 'options', label: 'Tùy chọn', bottom: true, content: <p>options page</p> },
  ];
}

describe('Backstage', () => {
  it('picks the page: the requested one, else the first enabled page', () => {
    const s = sections();
    expect(backstagePage(s, 'info')?.id).toBe('info');
    expect(backstagePage(s, 'save')?.id).toBe('new');
    expect(backstagePage(s, 'print')?.id).toBe('new');
    expect(backstagePage(s, undefined)?.id).toBe('new');
  });

  it('moves through the navigation with the arrows, Home and End', () => {
    expect(navTarget('ArrowDown', 0, 3)).toBe(1);
    expect(navTarget('ArrowDown', 2, 3)).toBe(0);
    expect(navTarget('ArrowUp', 0, 3)).toBe(2);
    expect(navTarget('Home', 2, 3)).toBe(0);
    expect(navTarget('End', 0, 3)).toBe(2);
    expect(navTarget('a', 0, 3)).toBeNull();
    expect(navTarget('ArrowDown', -1, 0)).toBeNull();
  });

  it('shows pages, runs actions, closes on Back and Escape', () => {
    const onClose = vi.fn();
    const save = vi.fn();
    const onPage = vi.fn();
    render(
      <Backstage
        open
        onClose={onClose}
        sections={sections(save)}
        labels={labels}
        heading="a.docx"
        onPageChange={onPage}
      />,
    );
    const dialog = screen.getByRole('dialog', { name: 'Tệp' });
    expect(dialog.getAttribute('aria-modal')).toBe('true');
    expect(screen.getByText('new page')).toBeTruthy();
    expect(screen.getByText('a.docx')).toBeTruthy();
    // The current page's entry has focus and aria-current.
    const newBtn = screen.getByRole('button', { name: 'Mới' });
    expect(newBtn.getAttribute('aria-current')).toBe('page');
    expect(document.activeElement).toBe(newBtn);
    fireEvent.click(screen.getByRole('button', { name: 'Thông tin' }));
    expect(onPage).toHaveBeenCalledWith('info');
    expect(screen.getByText('info page')).toBeTruthy();
    expect(screen.getByRole('heading', { name: 'Thông tin' })).toBeTruthy();
    // Disabled page.
    expect((screen.getByRole('button', { name: 'In' }) as HTMLButtonElement).disabled).toBe(true);
    // Keyboard navigation inside the column.
    const info = screen.getByRole('button', { name: 'Thông tin' });
    info.focus();
    fireEvent.keyDown(info, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Lưu' }));
    fireEvent.keyDown(document.activeElement!, { key: 'End' });
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Tùy chọn' }));
    // An action closes the screen, then runs.
    fireEvent.click(screen.getByRole('button', { name: 'Lưu' }));
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByTestId('backstage-back'));
    expect(onClose).toHaveBeenCalledTimes(2);
    fireEvent.keyDown(screen.getByRole('button', { name: 'Mới' }), { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(3);
  });

  it('renders nothing while closed', () => {
    render(<Backstage open={false} onClose={() => {}} sections={sections()} labels={labels} />);
    expect(screen.queryByTestId('backstage')).toBeNull();
  });

  it('commits a property on blur and on Enter', () => {
    const onChange = vi.fn();
    render(
      <BackstageProperties
        title="Thuộc tính"
        fields={[
          { id: 'title', label: 'Tiêu đề', value: 'A' },
          { id: 'created', label: 'Ngày tạo', value: '', readOnly: true },
        ]}
        onChange={onChange}
      />,
    );
    const input = screen.getByLabelText('Tiêu đề') as HTMLInputElement;
    expect(input.value).toBe('A');
    fireEvent.change(input, { target: { value: 'B' } });
    fireEvent.blur(input);
    expect(onChange).toHaveBeenCalledWith('title', 'B');
    fireEvent.change(input, { target: { value: 'C' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onChange).toHaveBeenLastCalledWith('title', 'C');
    expect(screen.getByTestId('prop-created').textContent).toBe('—');
  });

  it('lists files with a pin toggle and formats with their extension', () => {
    const onOpen = vi.fn();
    const onPin = vi.fn();
    const onFormat = vi.fn();
    render(
      <>
        <BackstageFiles
          title="Gần đây"
          files={[{ id: 'C:/a.docx', name: 'a.docx', pinned: true }]}
          empty="—"
          onOpen={onOpen}
          onTogglePin={onPin}
          pinLabel="Ghim"
          unpinLabel="Bỏ ghim"
        />
        <BackstageFormats
          groups={[
            {
              title: 'Kiểu',
              formats: [{ id: 'docm', label: 'Có macro', ext: '.docm', onSelect: onFormat }],
            },
            { title: 'Trống', formats: [] },
          ]}
        />
      </>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Bỏ ghim: a.docx' }));
    expect(onPin).toHaveBeenCalled();
    fireEvent.click(screen.getByText('a.docx'));
    expect(onOpen).toHaveBeenCalled();
    fireEvent.click(screen.getByText('Có macro'));
    expect(onFormat).toHaveBeenCalled();
    expect(screen.queryByText('Trống')).toBeNull();
  });
});

describe('pinned files', () => {
  it('toggles and stores pins in the setting store', async () => {
    const data = new Map<string, string | null>();
    const store = {
      getSetting: async (k: string) => data.get(k) ?? null,
      setSetting: async (k: string, v: string | null) => void data.set(k, v),
    };
    const a = { name: 'a.docx', path: 'C:/a.docx' };
    const b = { name: 'b.docx', path: 'C:/b.docx' };
    let pins = togglePinned([], a);
    pins = togglePinned(pins, b);
    expect(pins).toEqual([b, a]);
    await savePinned(store, 'doc', pins);
    expect(await loadPinned(store, 'doc')).toEqual([b, a]);
    expect(await loadPinned(store, 'sheet')).toEqual([]);
    await savePinned(store, 'doc', togglePinned(togglePinned(pins, a), b));
    expect(await loadPinned(store, 'doc')).toEqual([]);
    data.set('blueoffice.pinned.doc', 'not json');
    expect(await loadPinned(store, 'doc')).toEqual([]);
  });

  it('falls back to localStorage without a setting store', async () => {
    await savePinned({}, 'slide', [{ name: 'x.pptx', path: '/x.pptx' }]);
    expect(await loadPinned({}, 'slide')).toEqual([{ name: 'x.pptx', path: '/x.pptx' }]);
  });
});

describe('Ribbon File button', () => {
  it('opens the File screen', async () => {
    const onOpen = vi.fn();
    render(
      <Ribbon
        tabs={[{ id: 'home', label: 'Trang đầu', groups: [] }]}
        fileMenu={{ label: 'Tệp', onOpen }}
      />,
    );
    const b = screen.getByTestId('file-tab');
    expect(b.getAttribute('aria-haspopup')).toBe('dialog');
    fireEvent.click(screen.getByRole('button', { name: 'Tệp' }));
    await waitFor(() => expect(onOpen).toHaveBeenCalled());
  });
});
