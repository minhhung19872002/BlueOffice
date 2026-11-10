// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { VbaProjectInfo } from '@blueoffice/platform';
import { act, cleanup, fireEvent, render, renderHook, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { initialModule, lineCount, MacroViewer, useMacroViewer } from '../src/MacroViewer';
import { MacroBanner } from '../src/SecurityPanels';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const MODULE1 = 'Option Explicit\n\nSub ChaoMung()\n    Range("A1").Value = "Xin chào"\nEnd Sub\n';

const project = (patch: Partial<VbaProjectInfo> = {}): VbaProjectInfo => ({
  name: 'VBAProject',
  codePage: 1258,
  codePageSupported: true,
  modules: [
    { name: 'ThisWorkbook', kind: 'document', source: 'Option Explicit\n' },
    { name: 'Sheet1', kind: 'document', source: '' },
    { name: 'Module1', kind: 'standard', source: MODULE1 },
    { name: 'KhachHang', kind: 'class', source: 'Public Ten As String\n' },
    { name: 'Hong', kind: 'standard', source: '', unreadable: true },
  ],
  ...patch,
});

const code = () => screen.getByTestId('macro-viewer-code') as HTMLTextAreaElement;
const convert = () => screen.getByTestId('macro-viewer-convert') as HTMLButtonElement;
const option = (name: string) =>
  screen.getAllByRole('option').find((o) => o.textContent?.startsWith(name)) as HTMLElement;

describe('MacroViewer (CORE-VBA-002)', () => {
  it('lists the modules and shows the code read-only, with the never-run notice', async () => {
    const onConvert = vi.fn();
    wrap(<MacroViewer load={async () => project()} onConvert={onConvert} onClose={() => {}} />);
    expect(screen.getByTestId('macro-viewer-notice').textContent).toContain(
      'BlueOffice never runs VBA',
    );
    expect(screen.getByRole('status').textContent).toBe('Reading the macros…');

    // The first standard module with code is shown, not the (nearly empty) document module.
    const list = await screen.findByTestId('macro-viewer-modules');
    expect(screen.getAllByRole('option').map((o) => o.textContent)).toEqual([
      'ThisWorkbookDocument object · 1 line(s)',
      'Sheet1Document object · 0 line(s)',
      'Module1Module · 5 line(s)',
      'KhachHangClass · 1 line(s)',
      'HongModule',
    ]);
    expect(list.getAttribute('aria-label')).toBe('Modules');
    expect(option('Module1').getAttribute('aria-selected')).toBe('true');
    expect(code().value).toBe(MODULE1);
    expect(code().readOnly).toBe(true);
    expect(code().getAttribute('aria-label')).toBe('Code of Module1 (read-only)');

    // Typing does not change the code: the box is read-only and its value is controlled.
    fireEvent.change(code(), { target: { value: 'hacked' } });
    expect(code().value).toBe(MODULE1);

    // Another module, by click and by keyboard.
    fireEvent.click(option('KhachHang'));
    expect(code().value).toBe('Public Ten As String\n');
    fireEvent.keyDown(option('KhachHang'), { key: 'ArrowUp' });
    expect(option('Module1').getAttribute('aria-selected')).toBe('true');
    expect(document.activeElement).toBe(option('Module1'));

    // Convert sends exactly the selected module; nothing was converted before the click.
    expect(onConvert).not.toHaveBeenCalled();
    fireEvent.click(convert());
    expect(onConvert).toHaveBeenCalledTimes(1);
    expect(onConvert).toHaveBeenCalledWith({ name: 'Module1', source: MODULE1 });
  });

  it('does not offer to convert an empty or unreadable module', async () => {
    wrap(<MacroViewer load={async () => project()} onConvert={() => {}} onClose={() => {}} />);
    await screen.findByTestId('macro-viewer-modules');
    expect(convert().disabled).toBe(false);
    fireEvent.click(option('Sheet1'));
    expect(convert().disabled).toBe(true);
    expect(code().placeholder).toBe('This module has no code.');
    fireEvent.click(option('Hong'));
    expect(screen.getByTestId('macro-viewer-unreadable').textContent).toBe(
      'This module could not be read.',
    );
    expect(screen.queryByTestId('macro-viewer-code')).toBeNull();
    expect(convert().disabled).toBe(true);
  });

  it('explains a file without a VBA project, a failed read and an unknown code page', async () => {
    const empty = wrap(
      <MacroViewer
        load={async () => project({ name: '', modules: [] })}
        onConvert={() => {}}
        onClose={() => {}}
      />,
    );
    expect((await screen.findByTestId('macro-viewer-empty')).textContent).toContain(
      'no VBA project to show',
    );
    expect(convert().disabled).toBe(true);
    empty.unmount();

    const broken = wrap(
      <MacroViewer
        load={async () => project({ modules: [], error: 'malformed file' })}
        onConvert={() => {}}
        onClose={() => {}}
      />,
    );
    expect((await screen.findByTestId('macro-viewer-error')).textContent).toBe(
      'The VBA project could not be read. It is still kept in the file.',
    );
    expect(screen.queryByTestId('macro-viewer-empty')).toBeNull();
    broken.unmount();

    const rejected = wrap(
      <MacroViewer
        load={() => Promise.reject(new Error('engine gone'))}
        onConvert={() => {}}
        onClose={() => {}}
      />,
    );
    expect(await screen.findByTestId('macro-viewer-error')).toBeTruthy();
    rejected.unmount();

    wrap(
      <MacroViewer
        load={async () => project({ codePage: 932, codePageSupported: false, truncated: true })}
        onConvert={() => {}}
        onClose={() => {}}
      />,
    );
    expect((await screen.findByTestId('macro-viewer-codepage')).textContent).toContain(
      'code page 932',
    );
    expect(screen.getByText('The project is very large: part of it is not shown.')).toBeTruthy();
  });

  it('closes from the Close button', async () => {
    const onClose = vi.fn();
    wrap(<MacroViewer load={async () => project()} onConvert={() => {}} onClose={onClose} />);
    await screen.findByTestId('macro-viewer-modules');
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('has the same keys in Vietnamese', async () => {
    const vi18n = createI18n('vi-VN');
    render(
      <I18nextProvider i18n={vi18n}>
        <MacroViewer load={async () => project()} onConvert={() => {}} onClose={() => {}} />
      </I18nextProvider>,
    );
    await screen.findByTestId('macro-viewer-modules');
    expect(screen.getByTestId('macro-viewer-notice').textContent).toContain(
      'BlueOffice không bao giờ chạy VBA',
    );
    expect(option('Module1').textContent).toBe('Module1Mô-đun · 5 dòng');
    expect(convert().textContent).toBe('Chuyển thành bản nháp BlueScript');
  });
});

describe('macro viewer helpers', () => {
  it('picks the first module worth showing', () => {
    expect(initialModule(project().modules)).toBe(2);
    expect(initialModule(project().modules.slice(0, 2))).toBe(0);
    expect(initialModule([{ name: 'A', kind: 'class', source: '' }])).toBe(0);
    expect(initialModule([])).toBe(0);
    expect(
      initialModule([
        { name: 'A', kind: 'standard', source: 'x', unreadable: true },
        { name: 'B', kind: 'class', source: 'y' },
      ]),
    ).toBe(1);
  });

  it('counts lines', () => {
    expect(lineCount('')).toBe(0);
    expect(lineCount('a')).toBe(1);
    expect(lineCount('a\n')).toBe(1);
    expect(lineCount('a\nb')).toBe(2);
    expect(lineCount('a\n\nb\n')).toBe(3);
  });

  it('useMacroViewer hands the module to the script pane as a new draft request', () => {
    const openPane = vi.fn();
    const { result } = renderHook(() => useMacroViewer(openPane));
    expect(result.current.open).toBe(false);
    act(() => result.current.show());
    expect(result.current.open).toBe(true);
    act(() => result.current.convert({ name: 'Module1', source: MODULE1 }));
    expect(result.current.open).toBe(false);
    expect(result.current.draft).toEqual({ id: 1, source: MODULE1, module: 'Module1' });
    expect(openPane).toHaveBeenCalledTimes(1);
    act(() => result.current.draftHandled());
    expect(result.current.draft).toBeNull();
    // The same module again is a new request.
    act(() => result.current.convert({ name: 'Module1', source: MODULE1 }));
    expect(result.current.draft?.id).toBe(2);
  });
});

describe('MacroBanner', () => {
  it('offers to view the macros when the editor has a viewer', () => {
    const onView = vi.fn();
    const onDismiss = vi.fn();
    const withView = wrap(<MacroBanner onView={onView} onDismiss={onDismiss} />);
    fireEvent.click(screen.getByTestId('macro-warning-view'));
    expect(onView).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId('macro-warning-view').textContent).toBe('View macros');
    expect(onDismiss).not.toHaveBeenCalled();
    withView.unmount();
    wrap(<MacroBanner onDismiss={onDismiss} />);
    expect(screen.queryByTestId('macro-warning-view')).toBeNull();
  });
});
