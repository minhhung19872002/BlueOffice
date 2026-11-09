// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { EditCommand, EditResult, EngineHost } from '@blueoffice/platform';
import { I18nextProvider, createI18n } from '@blueoffice/i18n';
import type { ReactNode } from 'react';
import { DocController, type EditView } from '../src/controller';
import { ClipboardPanel } from '../src/clipboard/ClipboardPanel';
import { CLIP_HISTORY_MAX, ClipboardHistory, type HistoryStorage } from '../src/clipboard/history';
import {
  DEFAULT_PASTE,
  defaultChoice,
  isLink,
  loadPasteDefaults,
  pasteChoices,
  savePasteDefaults,
} from '../src/clipboard/docPaste';
import { PasteOptionsButton } from '../src/clipboard/PasteOptions';

afterEach(cleanup);

const i18n = createI18n();
void i18n.changeLanguage('en-US');
const wrap = (ui: ReactNode) => render(<I18nextProvider i18n={i18n}>{ui}</I18nextProvider>);

function memoryStorage(): HistoryStorage & { data: Map<string, string> } {
  const data = new Map<string, string>();
  return {
    data,
    getItem: (k) => data.get(k) ?? null,
    setItem: (k, v) => void data.set(k, v),
    removeItem: (k) => void data.delete(k),
  };
}

describe('clipboard history', () => {
  it('keeps the newest 24 items, folds repeated copies and survives a reload', () => {
    const store = memoryStorage();
    const h = new ClipboardHistory(store);
    for (let i = 0; i < 30; i++) h.push({ app: 'doc', kind: 'text', text: `mục ${i}` });
    expect(h.list()).toHaveLength(CLIP_HISTORY_MAX);
    expect(h.list()[0]?.text).toBe('mục 29');
    expect(h.list().at(-1)?.text).toBe('mục 6');
    // Copying the same thing again moves it to the top instead of adding it twice.
    h.push({ app: 'doc', kind: 'text', text: 'mục 10' });
    expect(h.list()).toHaveLength(CLIP_HISTORY_MAX);
    expect(h.list()[0]?.text).toBe('mục 10');
    expect(h.list().filter((i) => i.text === 'mục 10')).toHaveLength(1);
    // Empty copies are ignored.
    expect(h.push({ app: 'sheet', kind: 'cells', text: '' })).toBeNull();
    const again = new ClipboardHistory(store);
    expect(again.list().map((i) => i.text)).toEqual(h.list().map((i) => i.text));
    const first = again.list()[0];
    if (first) again.remove(first.id);
    expect(again.list()).toHaveLength(CLIP_HISTORY_MAX - 1);
    again.clear();
    expect(again.list()).toHaveLength(0);
    expect(store.data.size).toBe(0);
  });
});

describe('paste mode rules', () => {
  it('recognises links and offers the modes a paste allows', () => {
    expect(isLink(' https://bluestar.vn/a ')).toBe(true);
    expect(isLink('www.bluestar.vn')).toBe(true);
    expect(isLink('ai@bluestar.vn')).toBe(true);
    expect(isLink('hai từ')).toBe(false);
    expect(isLink('bluestar')).toBe(false);
    expect(pasteChoices({ text: 'a', html: '<b>a</b>', internal: '' })).toEqual([
      'keep',
      'merge',
      'text',
    ]);
    expect(pasteChoices({ text: 'a', html: '', internal: '[]', png: 'iVBO' })).toEqual([
      'keep',
      'merge',
      'text',
      'picture',
    ]);
    expect(pasteChoices({ text: 'https://x.vn', html: '', internal: '' })).toEqual([
      'text',
      'link',
    ]);
    expect(pasteChoices({ text: '', html: '', internal: '', png: 'x', pictureOnly: true })).toEqual(
      ['picture'],
    );
  });

  it('picks the default mode by source and keeps the defaults', () => {
    const d = { same: 'keep', other: 'merge', plain: 'link' } as const;
    const base = { html: '', internal: '', text: 'x' };
    expect(defaultChoice({ ...base, internal: '[]', source: 'same' }, d)).toBe('keep');
    expect(defaultChoice({ ...base, html: '<p>x</p>', source: 'other' }, d)).toBe('merge');
    expect(defaultChoice({ ...base, text: 'https://a.vn', source: 'plain' }, d)).toBe('link');
    expect(defaultChoice({ ...base, source: 'plain' }, d)).toBe('text');
    savePasteDefaults(d);
    expect(loadPasteDefaults()).toEqual(d);
    localStorage.setItem('bo.pasteDefaults', '{"same":"bogus"}');
    expect(loadPasteDefaults()).toEqual(DEFAULT_PASTE);
  });
});

function result(over: Partial<EditResult> = {}): EditResult {
  return {
    info: { name: 'a.docx', pageCount: 1, pages: [{ width: 595, height: 842 }], wordCount: 0 },
    selection: { anchor: { p: 0, o: 0 }, focus: { p: 0, o: 0 } },
    caret: { page: 0, x: 85, y: 56, h: 15 },
    rects: [],
    dirty: [],
    format: {} as EditResult['format'],
    modified: true,
    ...over,
  };
}

function fakeController() {
  const commands: EditCommand[] = [];
  const images: number[] = [];
  const engine = {
    command: async (c: EditCommand) => {
      commands.push(c);
      if (c.cmd === 'copy')
        return result({
          clip: { text: 'đậm chữ', html: '<b>đậm</b> chữ', internal: '[{}]', png: 'iVBORw0KGgo=' },
        });
      return result();
    },
    insertImage: async (b: Uint8Array) => {
      images.push(b.length);
      return result();
    },
  } as unknown as EngineHost;
  const view: EditView = { refresh: () => {}, setSelection: () => {} };
  const clipboard = {
    readText: async () => '',
    writeText: async () => {},
    writeHtml: async () => {},
  };
  const history = new ClipboardHistory(memoryStorage());
  const c = new DocController(
    engine,
    view,
    clipboard,
    () => {},
    (e) => {
      throw e;
    },
  );
  c.history = history;
  return { c, commands, images, history };
}

describe('DocController paste options', () => {
  it('redoes the last paste in another mode as undo + paste (one undo step)', async () => {
    const { c, commands } = fakeController();
    const seen: (string | null)[] = [];
    c.onPasteChange((r) => seen.push(r?.mode ?? null));
    await c.paste('đậm chữ', '<b>đậm</b> chữ');
    expect(commands.at(-1)).toMatchObject({ cmd: 'paste', mode: 'keep', html: '<b>đậm</b> chữ' });
    expect(c.lastPaste?.source).toBe('other');
    await c.switchPaste('text');
    expect(commands.slice(-2).map((x) => [x.cmd, 'mode' in x ? x.mode : undefined])).toEqual([
      ['undo', undefined],
      ['paste', 'text'],
    ]);
    await c.switchPaste('merge');
    expect(commands.filter((x) => x.cmd === 'undo')).toHaveLength(2);
    expect(c.lastPaste?.mode).toBe('merge');
    // The same mode again does nothing.
    const n = commands.length;
    await c.switchPaste('merge');
    expect(commands).toHaveLength(n);
    // Any other edit ends the paste options.
    await c.run({ cmd: 'insertText', text: 'x' });
    expect(c.lastPaste).toBeNull();
    expect(seen).toEqual(['keep', 'text', 'merge', null]);
  });

  it('copies into the history and pastes its own copy as a picture', async () => {
    const { c, commands, images, history } = fakeController();
    await c.copy();
    expect(history.list()[0]).toMatchObject({ app: 'doc', kind: 'formatted', text: 'đậm chữ' });
    // Own copy: the internal form, the "same document" default.
    await c.paste('đậm chữ', '<b>đậm</b> chữ');
    expect(commands.at(-1)).toMatchObject({ cmd: 'paste', internal: '[{}]', mode: 'keep' });
    expect(c.lastPaste?.source).toBe('same');
    expect(pasteChoices(c.lastPaste!)).toContain('picture');
    await c.switchPaste('picture');
    expect(commands.at(-1)?.cmd).toBe('undo');
    expect(images).toHaveLength(1);
    expect(c.lastPaste?.mode).toBe('picture');
  });

  it('uses the default modes and pastes history items', async () => {
    const { c, commands } = fakeController();
    c.pasteDefaults = { same: 'keep', other: 'text', plain: 'link' };
    await c.paste('https://bluestar.vn', '');
    expect(commands.at(-1)).toMatchObject({ cmd: 'paste', mode: 'link' });
    await c.pasteItem({
      id: 1,
      app: 'slide',
      kind: 'shapes',
      text: 'hình',
      html: '<p>hình</p>',
      time: 0,
    });
    expect(commands.at(-1)).toMatchObject({ cmd: 'paste', mode: 'text', text: 'hình' });
  });
});

describe('paste options button', () => {
  it('opens with a click or Alt+Shift+V and chooses with a digit', () => {
    const onChoose = vi.fn();
    wrap(
      <PasteOptionsButton
        anchor={() => ({ x: 100, y: 50 })}
        options={[
          { id: 'keep', label: 'Giữ' },
          { id: 'text', label: 'Chữ' },
        ]}
        current="keep"
        onChoose={onChoose}
        settings={{ label: 'Mặc định', onSelect: () => {} }}
      />,
    );
    const button = screen.getByTestId('paste-options');
    expect(button.parentElement?.style.left).toBe('100px');
    fireEvent.click(button);
    const menu = screen.getByTestId('paste-options-menu');
    expect(screen.getByRole('menuitemradio', { name: /Giữ/ }).getAttribute('aria-checked')).toBe(
      'true',
    );
    fireEvent.keyDown(menu, { key: '2' });
    expect(onChoose).toHaveBeenCalledWith('text');
    expect(screen.queryByTestId('paste-options-menu')).toBeNull();
    act(() => {
      window.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'V', code: 'KeyV', altKey: true, shiftKey: true }),
      );
    });
    expect(screen.getByTestId('paste-options-menu')).toBeTruthy();
    fireEvent.keyDown(screen.getByTestId('paste-options-menu'), { key: 'Escape' });
    expect(screen.queryByTestId('paste-options-menu')).toBeNull();
  });
});

describe('clipboard history panel', () => {
  it('lists items, pastes one and clears all', () => {
    const h = new ClipboardHistory(memoryStorage());
    h.push({ app: 'sheet', kind: 'cells', text: '1\t2' });
    h.push({ app: 'doc', kind: 'formatted', text: 'Xin chào' });
    const onPaste = vi.fn();
    wrap(<ClipboardPanel history={h} onPaste={onPaste} onClose={() => {}} />);
    const items = screen.getAllByTestId('clipboard-item');
    expect(items).toHaveLength(2);
    expect(items[0]?.textContent).toContain('Xin chào');
    expect(items[1]?.textContent).toContain('BlueSheet');
    fireEvent.click(screen.getByRole('button', { name: 'Paste 1 2' }));
    expect(onPaste).toHaveBeenCalledWith(expect.objectContaining({ text: '1\t2' }));
    fireEvent.click(screen.getByRole('button', { name: 'Remove Xin chào' }));
    expect(screen.getAllByTestId('clipboard-item')).toHaveLength(1);
    fireEvent.click(screen.getByTestId('clipboard-clear'));
    expect(screen.queryAllByTestId('clipboard-item')).toHaveLength(0);
  });
});
