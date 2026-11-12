// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { isCaptionTarget, StatusBar, TitleBar, titleBarWindow } from '../src';

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

/** Fake window chrome recording every call (the desktop platform's `WindowChrome`). */
function fakeControls() {
  let state = { maximized: false, focused: true };
  const stateHandlers: ((s: typeof state) => void)[] = [];
  const maxHandlers: ((s: { hover: boolean; pressed: boolean }) => void)[] = [];
  const controls = {
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
    startDragging: vi.fn(),
    showSystemMenu: vi.fn(),
    getState: vi.fn(async () => state),
    onStateChange: vi.fn((h: (s: typeof state) => void) => {
      stateHandlers.push(h);
      return () => {};
    }),
    setMaximizeButton: vi.fn(),
    onMaximizeButton: vi.fn((h: (s: { hover: boolean; pressed: boolean }) => void) => {
      maxHandlers.push(h);
      return () => {};
    }),
    setAppearance: vi.fn(async () => 'mica' as const),
  };
  return {
    controls,
    setState(s: Partial<typeof state>) {
      state = { ...state, ...s };
      act(() => stateHandlers.forEach((h) => h(state)));
    },
    nativeMax(s: { hover: boolean; pressed: boolean }) {
      act(() => maxHandlers.forEach((h) => h(s)));
    },
  };
}

const LABELS = { minimize: 'Thu nhỏ', maximize: 'Phóng to', restore: 'Khôi phục', close: 'Đóng' };

function caption(f: ReturnType<typeof fakeControls>, theme = 'light') {
  return render(
    <div data-theme={theme} data-app="doc">
      <TitleBar
        app="doc"
        appName="BlueDoc"
        docName="Báo cáo.docx"
        quickAccessLabel="QAT"
        quickAccess={[{ id: 'file.save', label: 'Lưu', onSelect: vi.fn() }]}
        window={{ controls: f.controls, labels: LABELS }}
      />
    </div>,
  );
}

const frame = () => act(() => new Promise<void>((r) => requestAnimationFrame(() => r())));

describe('TitleBar as the window caption (desktop, ADR-0016)', () => {
  it('draws minimise, maximise / restore and close buttons that drive the window', async () => {
    const f = fakeControls();
    caption(f);
    const buttons = screen.getByTestId('window-buttons').querySelectorAll('button');
    expect(Array.from(buttons).map((b) => b.getAttribute('aria-label'))).toEqual([
      'Thu nhỏ',
      'Phóng to',
      'Đóng',
    ]);
    fireEvent.click(screen.getByTestId('window-minimize'));
    fireEvent.click(screen.getByTestId('window-maximize'));
    fireEvent.click(screen.getByTestId('window-close'));
    expect(f.controls.minimize).toHaveBeenCalledOnce();
    expect(f.controls.toggleMaximize).toHaveBeenCalledOnce();
    expect(f.controls.close).toHaveBeenCalledOnce();
    // Maximised: the middle button restores, with its own glyph.
    f.setState({ maximized: true });
    const max = screen.getByTestId('window-maximize');
    expect(max.getAttribute('aria-label')).toBe('Khôi phục');
    expect(max.querySelector('[data-icon="window.restore"]')).not.toBeNull();
    // Keyboard: real buttons in the tab order.
    expect(max.tagName).toBe('BUTTON');
    expect(max.tabIndex).toBe(0);
  });

  it('reports the maximise button for snap layouts and mirrors the native hover', async () => {
    const f = fakeControls();
    const { unmount } = caption(f);
    await frame();
    expect(f.controls.setMaximizeButton).toHaveBeenCalledWith(
      expect.objectContaining({ x: expect.any(Number), width: expect.any(Number) }),
    );
    f.nativeMax({ hover: true, pressed: false });
    expect(screen.getByTestId('window-maximize').hasAttribute('data-hover')).toBe(true);
    f.nativeMax({ hover: true, pressed: true });
    expect(screen.getByTestId('window-maximize').hasAttribute('data-pressed')).toBe(true);
    f.nativeMax({ hover: false, pressed: false });
    expect(screen.getByTestId('window-maximize').hasAttribute('data-hover')).toBe(false);
    unmount();
    expect(f.controls.setMaximizeButton).toHaveBeenLastCalledWith(null);
  });

  it('drags, maximises on double-click and opens the system menu from the caption only', () => {
    const f = fakeControls();
    caption(f);
    const name = screen.getByTestId('doc-name');
    fireEvent.mouseDown(name, { button: 0, detail: 1 });
    expect(f.controls.startDragging).toHaveBeenCalledOnce();
    fireEvent.mouseDown(name, { button: 0, detail: 2 });
    expect(f.controls.toggleMaximize).toHaveBeenCalledOnce();
    // Buttons keep their own handling; the right button does not drag.
    fireEvent.mouseDown(screen.getByRole('button', { name: 'Lưu' }), { button: 0, detail: 1 });
    fireEvent.mouseDown(name, { button: 2, detail: 1 });
    expect(f.controls.startDragging).toHaveBeenCalledOnce();
    fireEvent.contextMenu(name, { clientX: 300, clientY: 12 });
    expect(f.controls.showSystemMenu).toHaveBeenCalledWith(300, 12);
    fireEvent.contextMenu(screen.getByRole('button', { name: 'Lưu' }));
    expect(f.controls.showSystemMenu).toHaveBeenCalledOnce();
    // Alt+Space: the system menu from the keyboard.
    fireEvent.keyDown(window, { key: ' ', code: 'Space', altKey: true });
    expect(f.controls.showSystemMenu).toHaveBeenCalledTimes(2);
    expect(isCaptionTarget(name)).toBe(true);
    expect(isCaptionTarget(screen.getByTestId('window-close'))).toBe(false);
  });

  it('asks for the backdrop of the app theme: Mica light / dark, solid in high contrast', async () => {
    const f = fakeControls();
    const { container } = caption(f, 'dark');
    expect(f.controls.setAppearance).toHaveBeenLastCalledWith({ dark: true, solid: false });
    const themed = container.querySelector('[data-theme]')!;
    themed.setAttribute('data-theme', 'hc');
    await act(() => Promise.resolve());
    expect(f.controls.setAppearance).toHaveBeenLastCalledWith({ dark: false, solid: true });
    themed.setAttribute('data-theme', 'light');
    await act(() => Promise.resolve());
    expect(f.controls.setAppearance).toHaveBeenLastCalledWith({ dark: false, solid: false });
    // The chrome band height for the see-through backdrop.
    await frame();
    expect(themed.getAttribute('style') ?? '').toContain('--bo-chrome-h');
  });

  it('dims in an inactive window and stays a plain bar on the web', () => {
    const f = fakeControls();
    caption(f);
    f.setState({ focused: false });
    expect(screen.getByTestId('title-bar').getAttribute('data-focused')).toBe('false');
    cleanup();
    render(<TitleBar app="doc" appName="BlueDoc" quickAccessLabel="QAT" />);
    const bar = screen.getByTestId('title-bar');
    expect(screen.queryByTestId('window-buttons')).toBeNull();
    expect(bar.hasAttribute('data-caption')).toBe(false);
    expect(titleBarWindow(undefined, (k) => k)).toBeUndefined();
    expect(titleBarWindow(f.controls, (k) => k)?.labels.restore).toBe('titleBar.window.restore');
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
