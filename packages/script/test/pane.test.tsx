// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import type { ScriptHost } from '../src/host';
import { inlineRunner } from '../src/runner';
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
});
