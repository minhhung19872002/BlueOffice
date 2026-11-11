// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  TASK_PANE_MAX_WIDTH,
  TASK_PANE_MIN_WIDTH,
  TaskPaneHost,
  clampTaskPaneRect,
  dockTaskPane,
  emptyTaskPaneLayout,
  loadTaskPaneLayout,
  parseTaskPaneLayout,
  resizeTaskPaneDock,
  saveTaskPaneLayout,
  taskPaneDock,
  taskPaneInsets,
  taskPaneStorageKey,
  useTaskPaneLayout,
  type TaskPaneDef,
  type TaskPaneStorage,
} from '../src';

afterEach(cleanup);

const labels = {
  dockLeft: 'Dock left',
  dockRight: 'Dock right',
  float: 'Float',
  close: 'Close',
  resize: 'Resize',
  move: 'Move',
  tabs: 'Panes',
};

function memoryStorage(): TaskPaneStorage & { data: Map<string, string> } {
  const data = new Map<string, string>();
  return {
    data,
    getItem: (k) => data.get(k) ?? null,
    setItem: (k, v) => void data.set(k, v),
  };
}

const pane = (id: string, extra: Partial<TaskPaneDef> = {}): TaskPaneDef => ({
  id,
  title: `Pane ${id}`,
  content: <button type="button">inside {id}</button>,
  ...extra,
});

function Harness({
  ids,
  storage,
  focusApp,
  defs = {},
}: {
  ids: string[];
  storage: TaskPaneStorage;
  focusApp?: () => void;
  defs?: Record<string, Partial<TaskPaneDef>>;
}) {
  const [layout, setLayout] = useTaskPaneLayout('test', storage);
  const [open, setOpen] = useState(ids);
  return (
    <div>
      <button type="button" data-testid="app">
        app
      </button>
      <TaskPaneHost
        layout={layout}
        onLayout={setLayout}
        panes={open.map((id) => pane(id, defs[id]))}
        onClose={(id) => setOpen((o) => o.filter((x) => x !== id))}
        labels={labels}
        focusApp={focusApp}
      />
    </div>
  );
}

describe('task pane layout', () => {
  it('docks, floats and remembers the side to go back to', () => {
    let l = emptyTaskPaneLayout();
    expect(taskPaneDock(l, { id: 'a' })).toBe('right');
    expect(taskPaneDock(l, { id: 'nav', defaultDock: 'left' })).toBe('left');
    l = dockTaskPane(l, 'a', 'left');
    expect(l.active.left).toBe('a');
    l = dockTaskPane(l, 'a', 'float', { x: 10, y: 20, w: 300, h: 200 });
    expect(l.panes.a).toEqual({
      dock: 'float',
      side: 'left',
      rect: { x: 10, y: 20, w: 300, h: 200 },
    });
    expect(taskPaneInsets(l, [{ id: 'a' }])).toEqual({ left: 0, right: 0 });
    l = dockTaskPane(l, 'a', l.panes.a!.side);
    expect(taskPaneInsets(l, [{ id: 'a' }, { id: 'b' }])).toEqual({ left: 300, right: 300 });
  });

  it('clamps dock widths and floating boxes', () => {
    const l = emptyTaskPaneLayout();
    expect(resizeTaskPaneDock(l, 'left', 10).width.left).toBe(TASK_PANE_MIN_WIDTH);
    expect(resizeTaskPaneDock(l, 'right', 5000).width.right).toBe(TASK_PANE_MAX_WIDTH);
    expect(clampTaskPaneRect({ x: 900, y: -40, w: 300, h: 900 }, { w: 1000, h: 600 })).toEqual({
      x: 700,
      y: 0,
      w: 300,
      h: 600,
    });
  });

  it('persists per app and survives broken or blocked storage', () => {
    const s = memoryStorage();
    const l = dockTaskPane(emptyTaskPaneLayout(), 'styles', 'left');
    saveTaskPaneLayout('doc', l, s);
    expect(s.data.has(taskPaneStorageKey('doc'))).toBe(true);
    expect(loadTaskPaneLayout('doc', s).panes.styles?.dock).toBe('left');
    expect(loadTaskPaneLayout('sheet', s).panes.styles).toBeUndefined();
    s.data.set(taskPaneStorageKey('doc'), '{not json');
    expect(loadTaskPaneLayout('doc', s)).toEqual(emptyTaskPaneLayout());
    const throwing: TaskPaneStorage = {
      getItem: () => {
        throw new Error('blocked');
      },
      setItem: () => {
        throw new Error('blocked');
      },
    };
    expect(loadTaskPaneLayout('doc', throwing)).toEqual(emptyTaskPaneLayout());
    expect(() => saveTaskPaneLayout('doc', l, throwing)).not.toThrow();
    expect(loadTaskPaneLayout('doc', null)).toEqual(emptyTaskPaneLayout());
    const parsed = parseTaskPaneLayout({
      panes: { a: { dock: 'up' }, b: { dock: 'float', rect: { x: 1, y: 2, w: 3, h: 4 } } },
      width: { left: 'wide', right: 9999 },
    });
    expect(parsed.panes.a).toBeUndefined();
    expect(parsed.panes.b).toEqual({
      dock: 'float',
      side: 'right',
      rect: { x: 1, y: 2, w: 200, h: 140 },
    });
    expect(parsed.width).toEqual({ left: 300, right: TASK_PANE_MAX_WIDTH });
  });
});

describe('TaskPaneHost', () => {
  it('renders a docked pane as a labelled complementary region and closes it', () => {
    const s = memoryStorage();
    render(<Harness ids={['a']} storage={s} />);
    const dock = screen.getByRole('complementary', { name: 'Pane a' });
    expect(dock.getAttribute('data-dock')).toBe('right');
    expect(within(dock).getByText('inside a')).toBeTruthy();
    fireEvent.click(within(dock).getByTestId('taskpane-close'));
    expect(screen.queryByRole('complementary')).toBeNull();
  });

  it('shows panes of one dock as tabs; a new pane is selected', () => {
    const s = memoryStorage();
    render(<Harness ids={['a', 'b']} storage={s} />);
    const dock = screen.getByTestId('taskpane-dock-right');
    const tabs = within(dock).getAllByRole('tab');
    expect(tabs.map((t) => t.textContent)).toEqual(['Pane a', 'Pane b']);
    expect(tabs[1]!.getAttribute('aria-selected')).toBe('true');
    expect(
      within(dock).getByText('inside a').closest('[role=tabpanel]')!.hasAttribute('hidden'),
    ).toBe(true);
    fireEvent.click(tabs[0]!);
    expect(dock.getAttribute('data-active')).toBe('a');
    fireEvent.keyDown(within(dock).getAllByRole('tab')[0]!, { key: 'ArrowRight' });
    expect(dock.getAttribute('data-active')).toBe('b');
  });

  it('docks left / right, floats and docks back; layout is persisted', () => {
    const s = memoryStorage();
    const { unmount } = render(<Harness ids={['a']} storage={s} />);
    fireEvent.click(screen.getByTestId('taskpane-to-left'));
    expect(screen.getByTestId('taskpane-dock-left')).toBeTruthy();
    fireEvent.click(screen.getByTestId('taskpane-to-float'));
    const win = screen.getByTestId('taskpane-float-a');
    expect(win.getAttribute('role')).toBe('complementary');
    expect(win.style.width).toBe('300px');
    // Drag the title bar.
    const handle = within(win).getByTestId('taskpane-handle');
    fireEvent.pointerDown(handle, { button: 0, clientX: 100, clientY: 100, pointerId: 1 });
    fireEvent.pointerMove(handle, { clientX: 140, clientY: 130, pointerId: 1 });
    fireEvent.pointerUp(handle, { clientX: 140, clientY: 130, pointerId: 1 });
    const moved = JSON.parse(s.data.get(taskPaneStorageKey('test'))!);
    expect(moved.panes.a.dock).toBe('float');
    expect(moved.panes.a.rect.x).toBe(64);
    expect(moved.panes.a.rect.y).toBe(54);
    // Resize by the grip.
    const grip = within(win).getByTestId('taskpane-grip');
    fireEvent.pointerDown(grip, { button: 0, clientX: 0, clientY: 0, pointerId: 2 });
    fireEvent.pointerMove(grip, { clientX: 50, clientY: 20, pointerId: 2 });
    fireEvent.pointerUp(grip, { clientX: 50, clientY: 20, pointerId: 2 });
    expect(screen.getByTestId('taskpane-float-a').style.width).toBe('350px');
    unmount();
    // Remembered after a reload.
    render(<Harness ids={['a']} storage={s} />);
    const again = screen.getByTestId('taskpane-float-a');
    expect(again.style.left).toBe('64px');
    expect(again.style.width).toBe('350px');
    // Double-click the title bar docks it back where it was.
    fireEvent.doubleClick(within(again).getByTestId('taskpane-handle'));
    expect(screen.getByTestId('taskpane-dock-left')).toBeTruthy();
  });

  it('resizes a dock with the splitter (pointer and keyboard)', () => {
    const s = memoryStorage();
    render(<Harness ids={['a']} storage={s} />);
    const sep = screen.getByRole('separator', { name: 'Resize' });
    fireEvent.pointerDown(sep, { button: 0, clientX: 500, pointerId: 1 });
    fireEvent.pointerMove(sep, { clientX: 440, pointerId: 1 });
    fireEvent.pointerUp(sep, { clientX: 440, pointerId: 1 });
    expect(screen.getByTestId('taskpane-dock-right').style.width).toBe('360px');
    fireEvent.keyDown(sep, { key: 'ArrowRight' });
    expect(screen.getByTestId('taskpane-dock-right').style.width).toBe('344px');
    fireEvent.keyDown(sep, { key: 'Home' });
    expect(sep.getAttribute('aria-valuenow')).toBe(String(TASK_PANE_MIN_WIDTH));
    expect(JSON.parse(s.data.get(taskPaneStorageKey('test'))!).width.right).toBe(
      TASK_PANE_MIN_WIDTH,
    );
  });

  it('F6 cycles app → panes → app; Escape returns to the app', () => {
    const s = memoryStorage();
    const focusApp = vi.fn(() => screen.getByTestId('app').focus());
    render(
      <Harness
        ids={['a', 'n']}
        storage={s}
        focusApp={focusApp}
        defs={{ n: { defaultDock: 'left' } }}
      />,
    );
    screen.getByTestId('app').focus();
    act(() => void fireEvent.keyDown(document.activeElement!, { key: 'F6' }));
    expect(document.activeElement).toBe(screen.getByTestId('taskpane-dock-left'));
    act(() => void fireEvent.keyDown(document.activeElement!, { key: 'F6' }));
    expect(document.activeElement).toBe(screen.getByTestId('taskpane-dock-right'));
    act(() => void fireEvent.keyDown(document.activeElement!, { key: 'F6' }));
    expect(focusApp).toHaveBeenCalledTimes(1);
    expect(document.activeElement).toBe(screen.getByTestId('app'));
    act(() => void fireEvent.keyDown(document.activeElement!, { key: 'F6', shiftKey: true }));
    expect(document.activeElement).toBe(screen.getByTestId('taskpane-dock-right'));
    screen.getByText('inside a').focus();
    fireEvent.keyDown(screen.getByText('inside a'), { key: 'Escape' });
    expect(focusApp).toHaveBeenCalledTimes(2);
  });

  it('F6 also reaches the ribbon (its selected tab) and the status bar, then the app', () => {
    const s = memoryStorage();
    const focusApp = vi.fn(() => screen.getByTestId('app').focus());
    render(
      <>
        <div data-f6-region="ribbon">
          <button type="button" role="tab" aria-selected="false">
            Chèn
          </button>
          <button type="button" role="tab" aria-selected="true">
            Trang đầu
          </button>
        </div>
        <Harness ids={['a']} storage={s} focusApp={focusApp} />
        <footer data-f6-region="status" tabIndex={-1} data-testid="status" />
      </>,
    );
    screen.getByTestId('app').focus();
    const f6 = (shiftKey = false) =>
      act(() => void fireEvent.keyDown(document.activeElement!, { key: 'F6', shiftKey }));
    f6();
    expect(document.activeElement).toBe(screen.getByTestId('taskpane-dock-right'));
    f6();
    expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'Trang đầu' }));
    f6();
    expect(document.activeElement).toBe(screen.getByTestId('status'));
    f6();
    expect(document.activeElement).toBe(screen.getByTestId('app'));
    f6(true);
    expect(document.activeElement).toBe(screen.getByTestId('status'));
  });
});
