// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { EditCommand, EditResult, InspectReport } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CheckDocumentDialog, type CheckDocumentBridge } from '../src/CheckDocument';
import { docCheckBridge } from '../src/DocCheckDocument';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const report = (pending = false): InspectReport => ({
  inspector: [
    { category: 'comments', count: 2, details: ['Ann'], action: 'remove', pending: false },
    { category: 'revisions', count: 3, details: [], action: 'acceptAll', pending: false },
    { category: 'properties', count: 1, details: ['creator: Ann'], action: 'onSave', pending },
    { category: 'hiddenText', count: 0, details: [], action: 'none', pending: false },
    { category: 'embedded', count: 1, details: ['oleObject1.bin'], action: 'none', pending: false },
  ],
  accessibility: [
    {
      rule: 'altText',
      severity: 'error',
      target: { kind: 'drawing', para: 4, at: 0 },
      detail: 'Picture 1',
      fix: 'altText',
    },
    {
      rule: 'tableHeader',
      severity: 'warning',
      target: { kind: 'table', para: 7 },
      detail: 'Name Value',
      fix: 'headerRow',
    },
    {
      rule: 'contrast',
      severity: 'warning',
      target: { kind: 'paragraph', para: 3 },
      detail: 'grey',
      ratio: 4.48,
    },
    { rule: 'documentTitle', severity: 'tip', target: { kind: 'document' }, detail: '' },
  ],
  issueCount: 4,
  compatibility: [{ feature: 'contentControls', count: 2, formats: ['legacy', 'odf'] }],
});

function setup(initial = report()) {
  const bridge: CheckDocumentBridge = {
    inspect: vi.fn(async () => initial),
    remove: vi.fn(async () => report(true)),
    keep: vi.fn(async () => report(false)),
    fix: vi.fn(async () => initial),
    acceptAll: vi.fn(async () => initial),
    goTo: vi.fn(),
  };
  const onClose = vi.fn();
  wrap(<CheckDocumentDialog bridge={bridge} onClose={onClose} />);
  return { bridge, onClose };
}

describe('CheckDocumentDialog', () => {
  it('lists inspector findings and removes a category', async () => {
    const { bridge } = setup();
    const row = await screen.findByTestId('inspect-cat-comments');
    expect(row.textContent).toContain('2 found');
    expect(row.textContent).toContain('Ann');
    expect(screen.getByTestId('inspect-cat-hiddenText').textContent).toContain('Nothing found');
    expect(screen.getByTestId('inspect-cat-embedded').textContent).toContain(
      'not removed automatically',
    );
    fireEvent.click(screen.getByTestId('inspect-remove-comments'));
    await waitFor(() => expect(bridge.remove).toHaveBeenCalledWith(['comments']));
    // The properties are now queued for the next save: "Keep" cancels.
    const keep = await screen.findByTestId('inspect-keep-properties');
    expect(screen.getByTestId('inspect-cat-properties').textContent).toContain(
      'next time you save',
    );
    fireEvent.click(keep);
    await waitFor(() => expect(bridge.keep).toHaveBeenCalledWith(['properties']));
    fireEvent.click(await screen.findByTestId('inspect-accept-revisions'));
    await waitFor(() => expect(bridge.acceptAll).toHaveBeenCalled());
  });

  it('removes everything removable in one call', async () => {
    const { bridge } = setup();
    fireEvent.click(await screen.findByTestId('inspect-remove-everything'));
    await waitFor(() => expect(bridge.remove).toHaveBeenCalledWith(['comments', 'properties']));
  });

  it('shows accessibility issues by severity with quick fixes', async () => {
    const { bridge, onClose } = setup();
    fireEvent.click(await screen.findByTestId('inspect-tab-accessibility'));
    expect(screen.getByTestId('inspect-tab-accessibility').textContent).toContain('4');
    const errors = screen.getByTestId('inspect-severity-error');
    expect(within(errors).getByText('Missing alternative text')).toBeTruthy();
    expect(screen.getByTestId('inspect-severity-warning').textContent).toContain('Contrast 4.48:1');
    expect(screen.getByTestId('inspect-severity-tip').textContent).toContain(
      'Document has no title',
    );
    // Alternative text.
    fireEvent.click(screen.getByTestId('inspect-fix-altText'));
    fireEvent.change(screen.getByTestId('inspect-alt-input'), {
      target: { value: ' Red square ' },
    });
    fireEvent.click(screen.getByTestId('inspect-alt-apply'));
    await waitFor(() =>
      expect(bridge.fix).toHaveBeenCalledWith({
        kind: 'altText',
        target: { kind: 'drawing', para: 4, at: 0 },
        text: 'Red square',
      }),
    );
    // Header row.
    fireEvent.click(screen.getByTestId('inspect-fix-headerRow'));
    await waitFor(() =>
      expect(bridge.fix).toHaveBeenCalledWith({
        kind: 'headerRow',
        target: { kind: 'table', para: 7 },
      }),
    );
    // Go to: no button for document-wide issues; going to an object keeps the checker
    // open as a side pane (no overlay: the document stays usable).
    const goTo = screen.getAllByTestId('inspect-goto');
    expect(goTo).toHaveLength(3);
    fireEvent.click(goTo[0] as HTMLElement);
    expect(bridge.goTo).toHaveBeenCalledWith({ kind: 'drawing', para: 4, at: 0 });
    expect(onClose).not.toHaveBeenCalled();
    const pane = await screen.findByTestId('inspect-dialog');
    expect(pane.getAttribute('data-f6-region')).toBe('');
    // The next issue can be reached from the pane.
    fireEvent.click(screen.getAllByTestId('inspect-goto')[1] as HTMLElement);
    expect(bridge.goTo).toHaveBeenCalledTimes(2);
    fireEvent.click(screen.getByTestId('inspect-close'));
    expect(onClose).toHaveBeenCalled();
  });

  it('lists compatibility features with their formats', async () => {
    setup();
    fireEvent.click(await screen.findByTestId('inspect-tab-compatibility'));
    const row = screen.getByTestId('inspect-compat-contentControls');
    expect(row.textContent).toContain('Content controls');
    expect(row.textContent).toContain('× 2');
    expect(row.textContent).toContain('97–2003');
    expect(row.textContent).toContain('OpenDocument');
  });

  it('says when there is nothing to report', async () => {
    setup({ inspector: [], accessibility: [], issueCount: 0, compatibility: [] });
    fireEvent.click(await screen.findByTestId('inspect-tab-accessibility'));
    expect(screen.getByTestId('inspect-panel-accessibility').textContent).toContain(
      'No accessibility problems',
    );
    fireEvent.click(screen.getByTestId('inspect-tab-compatibility'));
    expect(screen.getByTestId('inspect-panel-compatibility').textContent).toContain(
      'Nothing in this file is affected',
    );
  });

  it('uses the Vietnamese wording', async () => {
    await i18n.changeLanguage('vi-VN');
    try {
      setup();
      expect((await screen.findByTestId('inspect-cat-comments')).textContent).toContain(
        'Chú thích và người viết',
      );
    } finally {
      await i18n.changeLanguage('en-US');
    }
  });
});

describe('docCheckBridge', () => {
  it('maps the dialog onto edit commands', async () => {
    const commands: EditCommand[] = [];
    const run = vi.fn(async (c: EditCommand) => {
      commands.push(c);
      return { inspection: report() } as EditResult;
    });
    const b = docCheckBridge(run);
    expect((await b.inspect())?.issueCount).toBe(4);
    await b.remove(['hiddenText']);
    await b.keep(['macros']);
    await b.fix?.({ kind: 'headerRow', target: { kind: 'table', para: 2 } });
    await b.acceptAll?.();
    b.goTo?.({ kind: 'drawing', para: 5, at: 6 });
    b.goTo?.({ kind: 'paragraph', para: 1 });
    b.goTo?.({ kind: 'document' });
    expect(commands).toEqual([
      { cmd: 'inspect' },
      { cmd: 'inspectRemove', categories: ['hiddenText'] },
      { cmd: 'inspectKeep', categories: ['macros'] },
      { cmd: 'inspectFix', fix: { kind: 'headerRow', target: { kind: 'table', para: 2 } } },
      { cmd: 'acceptAllRevisions' },
      { cmd: 'inspect' },
      { cmd: 'setSelection', selection: { anchor: { p: 5, o: 6 }, focus: { p: 5, o: 9 } } },
      { cmd: 'setSelection', selection: { anchor: { p: 1, o: 0 }, focus: { p: 1, o: 0 } } },
    ]);
  });
});
