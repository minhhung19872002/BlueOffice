// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import {
  FakeTranslationProvider,
  type DocTextSegment,
  type EditCommand,
  type EditResult,
} from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useEffect } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CommandRegistry } from '../src/commands';
import {
  docInsertAfter,
  docSelectionText,
  translateDocumentSegments,
  translatedName,
} from '../src/docTranslate';
import { useTranslate, type Translate, type TranslateOpen } from '../src/TranslatePane';
import { registerTranslateCommands } from '../src/translateCommands';

afterEach(() => {
  cleanup();
  localStorage.clear();
});

/** A fake BlueDoc engine: paragraphs of plain text, a selection, the translation commands. */
function fakeDoc(paras: { text: string; style?: string }[], segs?: DocTextSegment[]) {
  let selection = { anchor: { p: 0, o: 0 }, focus: { p: 0, o: 0 } };
  const log: EditCommand[] = [];
  const doc = paras.map((p) => ({ ...p }));
  const enc = new TextEncoder();
  const run = async (c: EditCommand): Promise<EditResult> => {
    log.push(c);
    const r = { selection } as unknown as EditResult;
    switch (c.cmd) {
      case 'setSelection':
        selection = c.selection;
        break;
      case 'speechText':
        r.speech = {
          paras: doc
            .map((p, i) => ({ p: i, text: p.text }))
            .filter((p) => p.p >= (c.from ?? 0) && p.p <= (c.to ?? doc.length)),
        };
        break;
      case 'translationSegments':
        r.translation = segs ?? doc.map((p, i) => ({ p: i, s: 0, text: p.text }));
        break;
      case 'translationApply':
        for (const s of c.segments) doc[s.p]!.text = s.text;
        r.count = c.segments.length;
        break;
      default:
        break;
    }
    return r;
  };
  const select = (a: [number, string], f: [number, string]) => {
    const off = (p: number, needle: string) =>
      enc.encode(doc[p]!.text.slice(0, doc[p]!.text.indexOf(needle))).length;
    selection = { anchor: { p: a[0], o: off(a[0], a[1]) }, focus: { p: f[0], o: off(f[0], f[1]) } };
  };
  return { run, log, doc, select };
}

describe('BlueDoc translation helpers', () => {
  it('reads the selection across paragraphs (byte offsets, objects left out)', async () => {
    const d = fakeDoc([{ text: 'Xin chào thế giới' }, { text: 'Hôm nay￼ trời đẹp' }]);
    expect(await docSelectionText(d.run)).toBe('');
    d.select([0, 'thế'], [1, ' trời']);
    expect(await docSelectionText(d.run)).toBe('thế giới\nHôm nay');
  });

  it('inserts after the selection with a space', async () => {
    const d = fakeDoc([{ text: 'Xin chào' }]);
    d.select([0, 'Xin'], [0, 'chào']);
    await docInsertAfter(d.run, 'Hello');
    expect(d.log.at(-2)).toEqual({
      cmd: 'setSelection',
      selection: { anchor: { p: 0, o: 4 }, focus: { p: 0, o: 4 } },
    });
    expect(d.log.at(-1)).toEqual({ cmd: 'insertText', text: ' Hello' });
  });

  it('translates the whole document by segments, keeping the paragraph count', async () => {
    const d = fakeDoc([
      { text: 'Báo cáo năm', style: 'Heading1' },
      { text: '' },
      { text: 'The quick brown fox' },
      { text: 'Hôm nay trời đẹp' },
    ]);
    const fake = new FakeTranslationProvider();
    const segs = await translateDocumentSegments(d.run, fake, 'auto', 'en');
    // Blank and English paragraphs are not sent and not rewritten.
    expect(segs).toEqual([
      { p: 0, s: 0, text: '[en] Báo cáo năm' },
      { p: 3, s: 0, text: '[en] Hôm nay trời đẹp' },
    ]);
    await d.run({ cmd: 'translationApply', segments: segs });
    expect(d.doc.map((p) => p.text)).toEqual([
      '[en] Báo cáo năm',
      '',
      'The quick brown fox',
      '[en] Hôm nay trời đẹp',
    ]);
    expect(d.doc).toHaveLength(4);
    expect(d.doc[0]?.style).toBe('Heading1');
    expect(translatedName('Báo cáo.docx', 'en')).toBe('Báo cáo (EN).docx');
    expect(translatedName('deck.pptx', 'vi', 'pptx')).toBe('deck (VI).pptx');
  });
});

describe('translation pane and commands', () => {
  const i18n = createI18n('vi-VN');

  function Harness({
    provider,
    onReady,
  }: {
    provider: FakeTranslationProvider | undefined;
    onReady: (t: Translate) => void;
  }) {
    const tr = useTranslate(provider, { writeText: async () => {} });
    useEffect(() => {
      onReady(tr);
    });
    return <>{tr.pane}</>;
  }

  function mount(provider: FakeTranslationProvider | undefined) {
    let tr: Translate | null = null;
    render(
      <I18nextProvider i18n={i18n}>
        <Harness provider={provider} onReady={(x) => (tr = x)} />
      </I18nextProvider>,
    );
    return { open: (o: TranslateOpen) => tr!.open(o) };
  }

  it('translates the selection and replaces / inserts it', async () => {
    const fake = new FakeTranslationProvider();
    const replace = vi.fn();
    const insert = vi.fn();
    const { open } = mount(fake);
    open({ source: { text: 'Xin chào các bạn', replace, insert } });
    const result = (await screen.findByTestId('translate-result')) as HTMLTextAreaElement;
    await waitFor(() => expect(result.value).toBe('[en] Xin chào các bạn'));
    expect(fake.calls[0]).toEqual({ texts: ['Xin chào các bạn'], from: 'vi', to: 'en' });
    // The result can be edited before use.
    fireEvent.change(result, { target: { value: 'Hello everyone' } });
    fireEvent.click(screen.getByTestId('translate-replace'));
    expect(replace).toHaveBeenCalledWith('Hello everyone');
    fireEvent.click(screen.getByTestId('translate-insert'));
    expect(insert).toHaveBeenCalledWith('Hello everyone');
  });

  it('says so when no provider is configured and sends nothing', async () => {
    const { open } = mount(undefined);
    open({ source: { text: 'Xin chào' } });
    expect(await screen.findByTestId('translate-unavailable')).toBeTruthy();
    const empty = new FakeTranslationProvider([]);
    cleanup();
    mount(empty).open({ source: { text: 'Xin chào' } });
    expect(await screen.findByTestId('translate-unavailable')).toBeTruthy();
    expect(empty.calls).toHaveLength(0);
  });

  it('runs a bulk action with progress, cancels it and reports errors', async () => {
    const fake = new FakeTranslationProvider();
    let release: () => void = () => {};
    const seen: { from: string; to: string }[] = [];
    let aborted = false;
    const { open } = mount(fake);
    open({
      source: { text: '' },
      actions: [
        {
          id: 'document',
          label: 'Whole',
          run: async ({ from, to, signal, onProgress }) => {
            seen.push({ from, to });
            onProgress(1, 4);
            await new Promise<void>((resolve, reject) => {
              release = resolve;
              signal.addEventListener('abort', () => {
                aborted = true;
                reject(new DOMException('aborted', 'AbortError'));
              });
            });
            return 'done!';
          },
        },
      ],
      autoRun: 'document',
    });
    const progress = await screen.findByTestId('translate-progress');
    expect(progress.textContent).toContain('1/4');
    expect(seen).toEqual([{ from: 'auto', to: 'en' }]);
    fireEvent.click(screen.getByTestId('translate-cancel'));
    await waitFor(() =>
      expect(screen.getByTestId('translate-notice').textContent).toBe('Đã hủy dịch.'),
    );
    expect(aborted).toBe(true);
    expect(screen.queryByTestId('translate-progress')).toBeNull();
    // Run again to the end.
    fireEvent.click(screen.getByTestId('translate-action-document'));
    await screen.findByTestId('translate-progress');
    release();
    await waitFor(() => expect(screen.getByTestId('translate-notice').textContent).toBe('done!'));
    // A provider error shows its message.
    fake.failWith = 'rateLimited';
    open({ source: { text: 'Xin chào các bạn' } });
    await waitFor(() =>
      expect(screen.getByTestId('translate-notice').textContent).toBe(
        'Dịch vụ dịch đang bận, hãy thử lại sau.',
      ),
    );
  });

  it('registers the commands with the action ids', async () => {
    const r = new CommandRegistry<{ doc: boolean }>();
    const begin = vi.fn();
    registerTranslateCommands(r, {
      enabled: (s) => s.doc,
      begin,
      actions: { document: 'review.translateDocument' },
    });
    expect(r.isEnabled('review.translate', { doc: false })).toBe(false);
    await r.execute('review.translate', { doc: true });
    await r.execute('review.translateDocument', { doc: true });
    expect(begin.mock.calls.map((c) => c[1])).toEqual([undefined, 'document']);
  });
});
