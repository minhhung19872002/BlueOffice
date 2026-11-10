// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import {
  revealDiff,
  type DocStyleEntry,
  type DocStyleSpec,
  type EditCommand,
  type EditResult,
  type RevealItem,
} from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { emptyStyleSpec, previewCss, StyleDialog } from '../src/StyleDialog';
import { RevealFormattingPane, StyleInspectorPane } from '../src/StyleInspector';
import { StylesPane, visibleStyles, type StylesPaneManage } from '../src/StylesPane';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const entry = (id: string, extra: Partial<DocStyleEntry> = {}): DocStyleEntry => ({
  id,
  name: id,
  kind: 'paragraph',
  quick: true,
  preview: {
    font: 'Arial',
    size: 11,
    bold: false,
    italic: false,
    underline: false,
    color: '000000',
  },
  ...extra,
});

const spec = (patch: Partial<DocStyleSpec> = {}): DocStyleSpec => ({
  ...emptyStyleSpec('Normal'),
  name: 'heading 1',
  size: 32,
  bold: true,
  ...patch,
});

/** A fake `run`/`query` answering style commands. */
function fakeEngine(answer: (c: EditCommand) => Partial<EditResult>) {
  return vi.fn(async (c: EditCommand) => answer(c) as EditResult);
}

function manage(run: StylesPaneManage['run'], query: StylesPaneManage['query']): StylesPaneManage {
  return {
    run,
    query,
    openFile: vi.fn(async () => null),
    saveFile: vi.fn(async () => null),
    openPane: vi.fn(),
  };
}

describe('visibleStyles', () => {
  const list = [
    entry('Normal', { inUse: true, uiPriority: 0 }),
    entry('Quote', { uiPriority: 29 }),
    entry('Heading1', { inUse: true, uiPriority: 9 }),
    entry('Hidden', { recommended: false }),
  ];
  it('recommended sorts by uiPriority and hides semi-hidden styles', () => {
    expect(visibleStyles(list, 'recommended', '').map((s) => s.id)).toEqual([
      'Normal',
      'Heading1',
      'Quote',
    ]);
  });
  it('in use and all', () => {
    expect(visibleStyles(list, 'inUse', '').map((s) => s.id)).toEqual(['Normal', 'Heading1']);
    expect(visibleStyles(list, 'all', 'h').map((s) => s.id)).toEqual(['Heading1', 'Hidden']);
  });
});

describe('StylesPane management', () => {
  it('modifies a style through its menu and the dialog', async () => {
    const query = fakeEngine((c) =>
      c.cmd === 'styleDetails' ? { styleMgr: { spec: spec() } } : {},
    );
    const run = fakeEngine(() => ({ styles: [entry('Normal'), entry('Heading1')] }));
    wrap(
      <StylesPane
        load={async () => [entry('Normal', { isDefault: true }), entry('Heading1')]}
        current="Heading1"
        refreshKey="a"
        apply={vi.fn()}
        manage={manage(run, query)}
      />,
    );
    const menus = await screen.findAllByTestId('style-menu');
    fireEvent.click(menus[1]!);
    fireEvent.click(screen.getByTestId('style-modify'));
    await screen.findByTestId('style-dialog');
    expect(query).toHaveBeenCalledWith({ cmd: 'styleDetails', id: 'Heading1' });
    expect((screen.getByTestId('style-dlg-name') as HTMLInputElement).value).toBe('heading 1');
    expect((screen.getByTestId('style-dlg-size') as HTMLInputElement).value).toBe('16');
    fireEvent.change(screen.getByTestId('style-dlg-color'), { target: { value: '#c00000' } });
    fireEvent.change(screen.getByTestId('style-dlg-space-before'), { target: { value: '24' } });
    expect(screen.getByTestId('style-dlg-preview').style.color).toMatch(/c00000|192, 0, 0/i);
    fireEvent.click(screen.getByTestId('style-dlg-ok'));
    await waitFor(() => expect(screen.queryByTestId('style-dialog')).toBeNull());
    const sent = run.mock.calls[0]![0] as Extract<EditCommand, { cmd: 'styleModify' }>;
    expect(sent.cmd).toBe('styleModify');
    expect(sent.id).toBe('Heading1');
    expect(sent.spec.color).toBe('C00000');
    expect(sent.spec.spaceBefore).toBe(480);
  });

  it('keeps the dialog open with the engine error', async () => {
    const run = fakeEngine(() => ({ error: 'nameTaken' }));
    wrap(
      <StylesPane
        load={async () => [entry('Normal')]}
        current={null}
        refreshKey="a"
        apply={vi.fn()}
        manage={manage(
          run,
          fakeEngine(() => ({})),
        )}
      />,
    );
    fireEvent.click(await screen.findByTestId('styles-new'));
    fireEvent.change(screen.getByTestId('style-dlg-name'), { target: { value: 'Normal' } });
    fireEvent.click(screen.getByTestId('style-dlg-ok'));
    expect((await screen.findByTestId('style-dlg-error')).textContent).toMatch(/already exists/);
    const sent = run.mock.calls[0]![0] as Extract<EditCommand, { cmd: 'styleNew' }>;
    expect(sent).toMatchObject({ cmd: 'styleNew', fromSelection: true });
    expect(sent.spec.basedOn).toBe('Normal');
  });

  it('select all, update to match and delete; the default style cannot be deleted', async () => {
    const run = fakeEngine((c) =>
      c.cmd === 'styleSelectAll'
        ? {
            matches: [
              { p: 0, start: 0, end: 3 },
              { p: 2, start: 0, end: 3 },
            ],
          }
        : {},
    );
    const openPane = vi.fn();
    wrap(
      <StylesPane
        load={async () => [entry('Normal', { isDefault: true }), entry('Heading1')]}
        current={null}
        refreshKey="a"
        apply={vi.fn()}
        manage={{
          ...manage(
            run,
            fakeEngine(() => ({})),
          ),
          openPane,
        }}
      />,
    );
    const menus = await screen.findAllByTestId('style-menu');
    fireEvent.click(menus[0]!);
    expect((screen.getByTestId('style-delete') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(menus[1]!);
    fireEvent.click(screen.getByTestId('style-select-all'));
    expect((await screen.findByTestId('styles-note')).textContent).toMatch(/2 instances/);
    fireEvent.click(screen.getAllByTestId('style-menu')[1]!);
    fireEvent.click(screen.getByTestId('style-update'));
    fireEvent.click(screen.getAllByTestId('style-menu')[1]!);
    fireEvent.click(screen.getByTestId('style-delete'));
    await waitFor(() => expect(run).toHaveBeenCalledTimes(3));
    expect(run.mock.calls.map((c) => c[0])).toEqual([
      { cmd: 'styleSelectAll', id: 'Heading1' },
      { cmd: 'styleUpdateToMatch', id: 'Heading1' },
      { cmd: 'styleDelete', id: 'Heading1' },
    ]);
    fireEvent.click(screen.getByTestId('styles-reveal'));
    expect(openPane).toHaveBeenCalledWith('revealFormatting');
  });

  it('opens the organizer on request and lists both documents', async () => {
    const query = fakeEngine(() => ({
      styleMgr: {
        organizer: {
          current: [{ id: 'Normal', name: 'Normal', type: 'paragraph', isDefault: true }],
          file: [{ id: 'Fancy', name: 'Fancy', type: 'paragraph', isDefault: false }],
        },
      },
    }));
    const run = fakeEngine(() => ({
      styleMgr: {
        organizer: {
          current: [
            { id: 'Fancy', name: 'Fancy', type: 'paragraph', isDefault: false },
            { id: 'Normal', name: 'Normal', type: 'paragraph', isDefault: true },
          ],
          file: [{ id: 'Fancy', name: 'Fancy', type: 'paragraph', isDefault: false }],
          copied: ['Fancy'],
        },
      },
    }));
    const m = manage(run, query);
    m.openFile = vi.fn(async () => ({ name: 'mau.dotx', bytes: new Uint8Array([1, 2, 3]) }));
    wrap(
      <StylesPane
        load={async () => [entry('Normal')]}
        current={null}
        refreshKey="a"
        apply={vi.fn()}
        manage={m}
        request={{ kind: 'organizer', n: 1 }}
      />,
    );
    fireEvent.click(await screen.findByTestId('organizer-open'));
    const right = (await screen.findByTestId('organizer-file')) as HTMLSelectElement;
    expect(query).toHaveBeenCalledWith({ cmd: 'styleOrganizer', file: 'AQID' });
    right.options[0]!.selected = true;
    fireEvent.change(right);
    fireEvent.click(screen.getByTestId('organizer-copy-to-current'));
    await waitFor(() =>
      expect(run).toHaveBeenCalledWith({
        cmd: 'styleOrganizerApply',
        file: 'AQID',
        action: 'copy',
        ids: ['Fancy'],
        toFile: false,
      }),
    );
    await waitFor(() =>
      expect((screen.getByTestId('organizer-current') as HTMLSelectElement).options).toHaveLength(
        2,
      ),
    );
  });
});

describe('StyleDialog', () => {
  it('character styles hide paragraph fields; preview follows the values', () => {
    wrap(
      <StyleDialog
        mode="modify"
        styleId="Strong"
        initial={spec({ type: 'character', name: 'Strong' })}
        styles={[entry('Emphasis', { kind: 'character' })]}
        onCancel={vi.fn()}
        onOk={vi.fn()}
      />,
    );
    expect(screen.queryByTestId('style-dlg-align')).toBeNull();
    expect(screen.queryByTestId('style-dlg-from-selection')).toBeNull();
    expect((screen.getByTestId('style-dlg-type') as HTMLSelectElement).disabled).toBe(true);
    const css = previewCss(spec({ align: 'Center', border: { style: 'double', size: 8 } }));
    expect(css.textAlign).toBe('center');
    expect(css.fontWeight).toBe(700);
    expect(String(css.border)).toContain('double');
  });
});

describe('Style Inspector and Reveal Formatting', () => {
  it('shows both levels and clears one', async () => {
    const info = {
      paragraphStyle: { id: 'Heading1', name: 'heading 1' },
      paragraphDirect: [{ key: 'align', value: 'center' }],
      characterStyle: null,
      characterDirect: [{ key: 'bold', value: 'true' }],
    };
    const query = fakeEngine(() => ({ styleMgr: { inspector: info } }));
    const run = fakeEngine(() => ({
      styleMgr: { inspector: { ...info, characterDirect: [] } },
    }));
    wrap(<StyleInspectorPane query={query} run={run} refreshKey={1} />);
    expect((await screen.findByTestId('inspector-paragraph-style')).textContent).toBe('heading 1');
    const direct = screen.getAllByTestId('inspector-direct');
    expect(direct.map((d) => d.textContent)).toEqual(['Alignment: center', 'Bold: Yes']);
    expect(
      (screen.getByTestId('inspector-clear-characterStyle') as HTMLButtonElement).disabled,
    ).toBe(true);
    fireEvent.click(screen.getByTestId('inspector-clear-characterDirect'));
    await waitFor(() => expect(screen.getAllByTestId('inspector-direct')).toHaveLength(1));
    expect(run).toHaveBeenCalledWith({ cmd: 'styleClear', level: 'characterDirect' });
  });

  it('lists sources and marks differences in compare mode', async () => {
    const a: RevealItem[] = [
      { group: 'font', key: 'size', value: '16pt', source: 'paragraphStyle', style: 'heading 1' },
      { group: 'font', key: 'bold', value: 'true', source: 'paragraphStyle', style: 'heading 1' },
      { group: 'paragraph', key: 'align', value: 'left', source: 'default' },
    ];
    const b: RevealItem[] = [
      { group: 'font', key: 'size', value: '12pt', source: 'default' },
      { group: 'font', key: 'bold', value: 'true', source: 'direct' },
      { group: 'paragraph', key: 'align', value: 'left', source: 'default' },
    ];
    expect([...revealDiff(a, b)]).toEqual(['font.size']);
    let answer = a;
    const query = fakeEngine(() => ({ styleMgr: { reveal: answer } }));
    const { rerender } = wrap(<RevealFormattingPane query={query} refreshKey={1} />);
    const items = await screen.findAllByTestId('reveal-item');
    expect(items[0]!.getAttribute('title')).toBe('Paragraph style: heading 1');
    fireEvent.click(screen.getByTestId('reveal-compare'));
    answer = b;
    rerender(
      <I18nextProvider i18n={i18n}>
        <RevealFormattingPane query={query} refreshKey={2} />
      </I18nextProvider>,
    );
    await waitFor(() =>
      expect(
        screen
          .getAllByTestId('reveal-item')
          .filter((e) => e.getAttribute('data-differs'))
          .map((e) => e.getAttribute('data-key')),
      ).toEqual(['size']),
    );
    expect(
      screen.getAllByTestId('reveal-item').find((e) => e.dataset.key === 'bold')!.dataset.source,
    ).toBe('direct');
  });
});
