// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import type { ScriptHost } from '../src/host';
import { inlineRunner } from '../src/runner';
import { ScriptRecorder } from '../src/recorder';
import { ScriptPane } from '../src/ScriptPane';
import { scriptStore } from '../src/store';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const memory = () => {
  const m = new Map<string, string>();
  return {
    getItem: (k: string) => m.get(k) ?? null,
    setItem: (k: string, v: string) => void m.set(k, v),
  };
};

describe('ScriptPane', () => {
  it('runs a script, shows its output and saves it', async () => {
    const calls: string[] = [];
    const host: ScriptHost = {
      app: 'doc',
      call: async (m, args) => {
        calls.push(`${m}(${JSON.stringify(args)})`);
        return 3;
      },
    };
    const store = scriptStore('doc', memory());
    wrap(<ScriptPane app="doc" host={() => host} runner={inlineRunner()} store={store} />);
    const code = screen.getByTestId('script-code') as HTMLTextAreaElement;
    expect(code.value).toContain('doc.paragraphs');
    fireEvent.change(code, {
      target: { value: "const n = await doc.replace('a', 'b');\nconsole.log('n =', n);" },
    });
    fireEvent.click(screen.getByTestId('script-run'));
    await waitFor(() => expect(screen.getByTestId('script-console').textContent).toContain('Done'));
    expect(screen.getByTestId('script-console').textContent).toContain('n = 3');
    expect(calls).toEqual(['doc.replace(["a","b",{}])']);

    fireEvent.change(screen.getByTestId('script-name'), { target: { value: 'Thay a' } });
    fireEvent.click(screen.getByTestId('script-save'));
    expect(store.list().map((s) => s.name)).toEqual(['Thay a']);
    expect(screen.getByRole('option', { name: 'Thay a' })).toBeTruthy();
  });

  it('reports errors and the missing document', async () => {
    let open = false;
    const host: ScriptHost = { app: 'doc', call: async () => null };
    wrap(
      <ScriptPane
        app="doc"
        host={() => (open ? host : null)}
        runner={inlineRunner()}
        store={scriptStore('doc', memory())}
      />,
    );
    fireEvent.click(screen.getByTestId('script-run'));
    expect(screen.getByTestId('script-console').textContent).toContain('Open a file first.');
    open = true;
    fireEvent.change(screen.getByTestId('script-code'), {
      target: { value: 'throw new Error("boom")' },
    });
    fireEvent.click(screen.getByTestId('script-run'));
    await waitFor(() =>
      expect(screen.getByTestId('script-console').textContent).toContain('Script error: boom'),
    );
  });

  it('suggests API members after a dot', () => {
    wrap(
      <ScriptPane
        app="sheet"
        host={() => null}
        runner={inlineRunner()}
        store={scriptStore('sheet', memory())}
      />,
    );
    const code = screen.getByTestId('script-code') as HTMLTextAreaElement;
    fireEvent.change(code, { target: { value: 'workbook.' } });
    code.setSelectionRange(9, 9);
    fireEvent.keyDown(code, { key: ' ', ctrlKey: true });
    const list = screen.getByRole('listbox', { name: 'Suggestions' });
    const names = [...list.querySelectorAll('[role=option]')].map((o) => o.textContent ?? '');
    expect(names.some((n) => n.startsWith('addSheet'))).toBe(true);
    expect(names.some((n) => n.startsWith('sheets'))).toBe(true);
  });

  it('converts pasted macro VBA into an unsaved draft with its warnings, without running it (CORE-SCRIPT-003)', async () => {
    const calls: string[] = [];
    const host: ScriptHost = {
      app: 'sheet',
      call: async (m, args) => {
        calls.push(`${m}(${JSON.stringify(args)})`);
        return null;
      },
    };
    const store = scriptStore('sheet', memory());
    store.save({ name: 'Existing', code: 'console.log(1)' });
    let handled = 0;
    const pane = (request: number) => (
      <I18nextProvider i18n={i18n}>
        <ScriptPane
          app="sheet"
          host={() => host}
          runner={inlineRunner()}
          store={store}
          vbaRequest={request}
          onVbaRequestHandled={() => handled++}
        />
      </I18nextProvider>
    );
    const view = render(pane(0));
    fireEvent.change(screen.getByTestId('script-list'), {
      target: { value: store.list()[0]!.id },
    });
    expect(screen.queryByTestId('script-vba-source')).toBeNull();

    // The "Convert macro VBA to BlueScript" command of the editor opens the converter.
    view.rerender(pane(1));
    const source = (await screen.findByTestId('script-vba-source')) as HTMLTextAreaElement;
    expect(handled).toBe(1);
    expect((screen.getByTestId('script-vba-convert') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(source, {
      target: {
        value: [
          'Sub Fill()',
          '  Application.ScreenUpdating = False',
          '  Range("A1").Value = "Total"',
          '  Range("A1").Select',
          'End Sub',
        ].join('\n'),
      },
    });
    fireEvent.click(screen.getByTestId('script-vba-convert'));

    const code = screen.getByTestId('script-code') as HTMLTextAreaElement;
    expect(code.value).toContain('BlueScript draft converted from macro VBA');
    expect(code.value).toContain('await sheet.range("A1").setValue("Total");');
    expect(code.value).toContain('// TODO(VBA): Range("A1").Select');
    // A new unsaved script: no saved script selected, nothing added to the store.
    expect((screen.getByTestId('script-list') as HTMLSelectElement).value).toBe('');
    expect((screen.getByTestId('script-name') as HTMLInputElement).value).toBe(
      'Draft from macro VBA',
    );
    expect((screen.getByTestId('script-delete') as HTMLButtonElement).disabled).toBe(true);
    expect(store.list().map((s) => s.name)).toEqual(['Existing']);
    // The warnings are listed with their line and reason.
    const warnings = [...screen.getByTestId('script-vba-warnings').querySelectorAll('li')].map(
      (li) => li.textContent,
    );
    expect(warnings).toEqual([
      'Line 2: Application.ScreenUpdating — not needed in BlueScript',
      'Line 4: range.Select — no matching BlueScript API',
    ]);
    expect(screen.queryByTestId('script-vba-source')).toBeNull();
    // Never run automatically.
    await new Promise((r) => setTimeout(r, 30));
    expect(calls).toEqual([]);
    expect(screen.getByTestId('script-console').textContent).toContain('It was not run.');
    expect(screen.getByTestId('script-console').textContent).not.toContain('Done in');

    // The user runs the reviewed draft.
    fireEvent.click(screen.getByTestId('script-run'));
    await waitFor(() =>
      expect(screen.getByTestId('script-console').textContent).toContain('Done in'),
    );
    expect(calls).toEqual(['sheet.fill([-1,"A1","Total"])']);

    // The button of the pane opens the converter too; a clean macro reports no warning.
    fireEvent.click(screen.getByTestId('script-vba-open'));
    fireEvent.change(screen.getByTestId('script-vba-source'), {
      target: { value: 'Range("B2").Value = 1' },
    });
    fireEvent.click(screen.getByTestId('script-vba-convert'));
    expect(screen.queryByTestId('script-vba-warnings')).toBeNull();
    expect(screen.getByTestId('script-vba-report').textContent).toContain(
      'Everything was converted',
    );
  });

  it('drafts a module sent by the macro viewer without running it (CORE-VBA-002)', async () => {
    const calls: string[] = [];
    const host: ScriptHost = {
      app: 'sheet',
      call: async (m, args) => {
        calls.push(`${m}(${JSON.stringify(args)})`);
        return null;
      },
    };
    const store = scriptStore('sheet', memory());
    let handled = 0;
    const pane = (draft: { id: number; source: string; module?: string } | null) => (
      <I18nextProvider i18n={i18n}>
        <ScriptPane
          app="sheet"
          host={() => host}
          runner={inlineRunner()}
          store={store}
          vbaDraft={draft}
          onVbaDraftHandled={() => handled++}
        />
      </I18nextProvider>
    );
    // The pane opens with the request already set (the viewer opened it).
    const source = 'Sub ChaoMung()\n    Range("A1").Value = "Xin chào"\n    Range("A1").Select\nEnd Sub\n';
    const view = render(pane({ id: 1, source, module: 'Module1' }));
    const code = screen.getByTestId('script-code') as HTMLTextAreaElement;
    expect(code.value).toContain('BlueScript draft converted from macro VBA');
    expect(code.value).toContain('await sheet.range("A1").setValue("Xin chào");');
    expect(code.value).toContain('// TODO(VBA): Range("A1").Select');
    expect((screen.getByTestId('script-name') as HTMLInputElement).value).toBe(
      'Draft from macro VBA: Module1',
    );
    expect((screen.getByTestId('script-list') as HTMLSelectElement).value).toBe('');
    expect(screen.getByTestId('script-vba-warnings').querySelectorAll('li')).toHaveLength(1);
    // Nothing ran, the converter box stays closed, and the host clears the request.
    expect(calls).toEqual([]);
    expect(screen.getByTestId('script-console').textContent).toContain('It was not run.');
    expect(screen.queryByTestId('script-vba-source')).toBeNull();
    await waitFor(() => expect(handled).toBe(1));

    // Cleared, then the same text edited by the user is not overwritten on re-render.
    view.rerender(pane(null));
    fireEvent.change(code, { target: { value: '// reviewed' } });
    view.rerender(pane(null));
    expect(code.value).toBe('// reviewed');

    // A second request replaces the editor content with the new draft.
    view.rerender(pane({ id: 2, source: 'Sub B()\n    Range("B2").Value = 2\nEnd Sub' }));
    expect(code.value).toContain('await sheet.range("B2").setValue(2);');
    expect((screen.getByTestId('script-name') as HTMLInputElement).value).toBe(
      'Draft from macro VBA',
    );
    expect(screen.getByTestId('script-vba-report').textContent).toContain(
      'Everything was converted',
    );
    await waitFor(() => expect(handled).toBe(2));
    expect(calls).toEqual([]);
  });

  it('records actions into the editor and binds a saved script (CORE-SCRIPT-002)', async () => {
    const rec = new ScriptRecorder('sheet');
    const host: ScriptHost = { app: 'sheet', call: async () => null };
    const store = scriptStore('sheet', memory());
    wrap(
      <ScriptPane
        app="sheet"
        host={() => host}
        runner={inlineRunner()}
        store={store}
        recorder={rec}
        checkShortcut={(s) => (s === 'Ctrl+B' ? 'Bold' : null)}
      />,
    );
    fireEvent.click(screen.getByTestId('script-record'));
    await waitFor(() => expect(rec.recording).toBe(true));
    expect(screen.getByTestId('script-recording')).toBeTruthy();
    rec.sheetInputs(0, [[1, 2, 'x']]);
    fireEvent.click(screen.getByTestId('script-record'));
    const code = screen.getByTestId('script-code') as HTMLTextAreaElement;
    await waitFor(() => expect(code.value).toContain('range("C2").setValues([["x"]]);'));
    expect(screen.queryByTestId('script-recording')).toBeNull();

    fireEvent.click(screen.getByTestId('script-save'));
    const input = screen.getByTestId('script-shortcut');
    fireEvent.keyDown(input, { key: 'b', ctrlKey: true });
    expect(screen.getByTestId('script-bind-error').textContent).toContain('Bold');
    fireEvent.keyDown(input, { key: '1', ctrlKey: true, altKey: true });
    fireEvent.click(screen.getByTestId('script-qat'));
    expect(store.list()[0]).toMatchObject({ shortcut: 'Ctrl+Alt+1', quickAccess: true });
    expect((screen.getByTestId('script-shortcut') as HTMLInputElement).value).toBe('Ctrl+Alt+1');
  });
});
