// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Platform } from '@blueoffice/platform';
import { BlueEditor } from '../src/BlueEditor';

afterEach(cleanup);

function fakePlatform(): Platform & { opened: string[] } {
  const opened: string[] = [];
  return {
    opened,
    kind: 'web',
    files: { openFile: async () => null, saveFile: async (_b, n) => n },
    storage: {
      id: 'local',
      open: async () => ({
        ref: { provider: 'local', id: 'a.docx', name: 'a.docx' },
        bytes: new Uint8Array([1]),
        permissions: { edit: true, download: true, share: false },
      }),
      save: async (d) => d,
      saveAs: async () => null,
    },
    clipboard: { readText: async () => '', writeText: async () => {}, writeHtml: async () => {} },
    fonts: { list: async () => [], load: async () => new Uint8Array() },
    print: { printPdf: async () => {} },
    engine: {
      backend: 'wasm',
      init: async () => {},
      open: async (_b, name) => {
        opened.push(name);
        return {
          name,
          pageCount: 3,
          pages: [
            { width: 595, height: 842 },
            { width: 595, height: 842 },
            { width: 595, height: 842 },
          ],
          wordCount: 42,
        };
      },
      getPage: async () => ({ width: 595, height: 842, commands: [] }),
      getGlyphOutlines: async () => [],
      exportPdf: async () => new Uint8Array([37]),
      dispose: () => {},
    },
    auth: {
      user: null,
      getAccessToken: async () => null,
      signIn: async () => {},
      signOut: async () => {},
    },
    window: { setTitle: vi.fn(), openNew: async () => {} },
    offline: { get: async () => null, put: async () => {}, delete: async () => {} },
    shell: {
      recentFiles: async () => [],
      addRecentFile: async () => {},
      onOpenRequest: () => () => {},
    },
  };
}

describe('BlueEditor', () => {
  it('shows the Vietnamese shell and opens a document via the storage provider', async () => {
    const p = fakePlatform();
    render(<BlueEditor platform={p} />);
    expect(screen.getByRole('tab', { name: 'Trang đầu' })).toBeTruthy();
    expect(screen.getByText('Chào mừng đến với BlueWrite')).toBeTruthy();
    expect(screen.getByTestId('status-engine').textContent).toBe('Lõi: WASM');
    fireEvent.click(screen.getByText('Mở tệp từ máy'));
    await waitFor(() => expect(screen.getByTestId('status-page').textContent).toBe('Trang 1/3'));
    expect(p.opened).toEqual(['a.docx']);
    expect(screen.getByTestId('doc-name').textContent).toBe('a.docx');
    expect(p.window.setTitle).toHaveBeenCalledWith('a.docx — BlueWrite');
  });

  it('switches language and theme through commands', async () => {
    render(<BlueEditor platform={fakePlatform()} />);
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Xem' }));
    fireEvent.click(screen.getByText('English'));
    await waitFor(() => expect(screen.getByRole('tab', { name: 'View' })).toBeTruthy());
    const root = document.querySelector('.bo-root')!;
    expect(root.getAttribute('data-theme')).toBe('light');
    fireEvent.click(screen.getByText('Light / Dark'));
    await waitFor(() => expect(root.getAttribute('data-theme')).toBe('dark'));
  });

  it('honours host configuration', () => {
    const onClose = vi.fn();
    render(
      <BlueEditor
        platform={fakePlatform()}
        host={{ brandName: 'BlueDrive', showShare: true, showClose: true, onClose }}
      />,
    );
    expect(screen.getByText(/BlueDrive · BlueWrite/)).toBeTruthy();
    expect(screen.getByText('Chia sẻ')).toBeTruthy();
    fireEvent.click(screen.getByLabelText('Đóng'));
    expect(onClose).toHaveBeenCalled();
  });

  it('shows placeholders for sheet and slide apps', () => {
    render(<BlueEditor platform={fakePlatform()} app="sheet" />);
    expect(screen.getByText(/Phase 3/)).toBeTruthy();
  });
});
