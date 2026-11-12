// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import type { DocTreePara, EngineHost } from '@blueoffice/platform';
import { DEFAULT_MIRROR_LABELS, DocMirror, buildMirror, textChange, wordAt } from '../src/a11y';

const enc = new TextEncoder();
const bytes = (s: string) => enc.encode(s).length;

const TREE: DocTreePara[] = [
  { p: 0, text: 'Chương 1', heading: 1, page: 0 },
  { p: 1, text: 'Mục a', list: 1, page: 0 },
  { p: 2, text: 'Mục b', list: 2, page: 0 },
  {
    p: 3,
    text: 'Tên',
    cell: { table: '0.3', row: 0, col: 0, rows: 2, cols: 2, header: true },
    page: 0,
  },
  {
    p: 4,
    text: 'Tuổi',
    cell: { table: '0.3', row: 0, col: 1, rows: 2, cols: 2, header: true },
    page: 0,
  },
  { p: 5, text: 'An', cell: { table: '0.3', row: 1, col: 0, rows: 2, cols: 2 }, page: 0 },
  { p: 6, text: '30', cell: { table: '0.3', row: 1, col: 1, rows: 2, cols: 2 }, page: 0 },
  {
    p: 7,
    text: 'Xem trang chủ\uFFFC và mã',
    links: [
      { start: bytes('Xem '), end: bytes('Xem trang chủ'), href: 'https://example.org/' },
      {
        start: bytes('Xem trang chủ\uFFFC và '),
        end: bytes('Xem trang chủ\uFFFC và mã'),
        href: 'javascript:alert(1)',
      },
    ],
    objects: [{ at: bytes('Xem trang chủ'), kind: 'picture', alt: 'Biểu đồ' }],
    page: 1,
  },
];

describe('buildMirror', () => {
  it('mirrors headings, lists, tables, links and pictures', () => {
    const root = document.createElement('div');
    root.appendChild(buildMirror(TREE, DEFAULT_MIRROR_LABELS, 'm', 5));
    expect(root.querySelector('h1')?.textContent).toBe('Chương 1');
    const items = root.querySelectorAll('ul > li');
    expect([...items].map((li) => [li.textContent, li.getAttribute('aria-level')])).toEqual([
      ['Mục a', '1'],
      ['Mục b', '2'],
    ]);
    const table = root.querySelector('table')!;
    expect(table.getAttribute('aria-rowcount')).toBe('2');
    const rows = table.querySelectorAll('tr');
    expect(rows).toHaveLength(2);
    expect([...rows[0]!.querySelectorAll('th')].map((c) => c.textContent)).toEqual(['Tên', 'Tuổi']);
    expect(rows[0]!.querySelector('th')?.getAttribute('scope')).toBe('col');
    expect([...rows[1]!.querySelectorAll('td')].map((c) => c.textContent)).toEqual(['An', '30']);
    expect(rows[1]!.querySelectorAll('td')[1]!.getAttribute('aria-colindex')).toBe('2');
    // The caret's paragraph.
    expect(root.querySelector('[aria-current="true"]')?.textContent).toBe('An');
    expect(root.querySelector('[aria-current="true"]')?.id).toBe('m-p5');
    // Links: safe targets only; the picture by its alternative text.
    const last = root.querySelector('#m-p7')!;
    const a = last.querySelector('a')!;
    expect(a.getAttribute('href')).toBe('https://example.org/');
    expect(a.textContent).toBe('trang chủ');
    expect(last.querySelectorAll('a')).toHaveLength(1);
    expect(last.querySelector('[role="img"]')?.getAttribute('aria-label')).toBe('Picture: Biểu đồ');
    expect(last.textContent).toContain('và mã');
  });

  it('names pictures without alternative text, equations and objects', () => {
    const root = document.createElement('div');
    root.appendChild(
      buildMirror(
        [
          {
            p: 0,
            text: '\uFFFC\uFFFC\uFFFC',
            objects: [
              { at: 0, kind: 'picture', alt: '' },
              { at: 3, kind: 'equation', alt: '' },
              { at: 6, kind: 'object', alt: '' },
            ],
            page: 0,
          },
        ],
        DEFAULT_MIRROR_LABELS,
        'm',
        null,
      ),
    );
    expect(
      [...root.querySelectorAll('[role="img"]')].map((e) => e.getAttribute('aria-label')),
    ).toEqual(['Picture without alternative text', 'Equation', 'Object']);
  });
});

describe('text helpers', () => {
  it('finds the word at the caret and what an edit changed', () => {
    expect(wordAt('một hai ba', 4)).toBe('hai');
    expect(wordAt('một hai ba', 3)).toBe('hai');
    expect(wordAt('một', 3)).toBe('');
    expect(textChange('abc', 'abxc')).toEqual({ inserted: 'x', deleted: '' });
    expect(textChange('abxc', 'abc')).toEqual({ inserted: '', deleted: 'x' });
    expect(textChange('abc', 'abc')).toBeNull();
  });
});

/** An engine answering `speechTree` / `speechLine` from `paras` (one page). */
function fakeEngine(paras: { current: DocTreePara[] }, calls: unknown[]): EngineHost {
  return {
    command: async (cmd: { cmd: string; p?: number; o?: number }) => {
      calls.push(cmd);
      if (cmd.cmd === 'speechLine') {
        const t = paras.current.find((x) => x.p === cmd.p)?.text ?? '';
        // Lines of 6 bytes.
        const s = Math.floor((cmd.o ?? 0) / 6) * 6;
        return { speech: { line: [s, Math.min(bytes(t), s + 6)] } };
      }
      return { speech: { tree: paras.current } };
    },
  } as unknown as EngineHost;
}

describe('DocMirror', () => {
  it('follows the pages in view, the caret, moves and edits', async () => {
    const host = document.createElement('div');
    const input = document.createElement('textarea');
    host.appendChild(input);
    const paras = { current: [{ p: 0, text: 'một hai ba bốn năm', page: 0 }] as DocTreePara[] };
    const calls: unknown[] = [];
    const m = new DocMirror(host, fakeEngine(paras, calls), DEFAULT_MIRROR_LABELS, input);
    m.setRange(3, 4);
    m.select({ p: 0, o: 0 }, false);
    await m.idle();
    // Pages in view ± 1, the caret's paragraph included.
    expect(calls.at(-1)).toEqual({ cmd: 'speechTree', fromPage: 2, toPage: 5, include: 0 });
    expect(m.root.getAttribute('aria-label')).toBe('Text of page 1');
    expect(m.root.style.display).not.toBe('none');
    expect(m.root.querySelector('p')?.textContent).toBe('một hai ba bốn năm');
    expect(input.getAttribute('aria-describedby')).toBe(m.root.querySelector('p')?.id);
    expect(m.live.getAttribute('aria-live')).toBe('polite');

    const key = (k: string, ctrl = false) =>
      input.dispatchEvent(new KeyboardEvent('keydown', { key: k, ctrlKey: ctrl, bubbles: true }));
    // Ctrl+→: the word.
    key('ArrowRight', true);
    m.select({ p: 0, o: bytes('một ') }, false);
    await m.idle();
    expect(m.live.textContent).toBe('hai');
    // →: the character.
    key('ArrowRight');
    m.select({ p: 0, o: bytes('một h') }, false);
    await m.idle();
    expect(m.live.textContent).toBe('a');
    // ↓: the line (engine line range).
    key('ArrowDown');
    m.select({ p: 0, o: 8 }, false);
    await m.idle();
    expect(m.live.textContent).toBe('hai ba');
    // Typing: the text typed; deleting: the text deleted.
    paras.current = [{ p: 0, text: 'một hai xba bốn năm', page: 0 }];
    m.invalidate();
    m.select({ p: 0, o: bytes('một hai x') }, true);
    await m.idle();
    expect(m.live.textContent).toBe('x');
    paras.current = [{ p: 0, text: 'một hai ba bốn năm', page: 0 }];
    m.select({ p: 0, o: bytes('một hai ') }, true);
    await m.idle();
    expect(m.live.textContent).toBe('Deleted x');
    m.dispose();
    expect(host.querySelector('[data-testid="doc-a11y-mirror"]')).toBeNull();
  });

  it('announces an edit when an earlier caret query sees the new text first', async () => {
    const host = document.createElement('div');
    const input = document.createElement('textarea');
    host.appendChild(input);
    const paras = { current: [{ p: 0, text: 'abc', page: 0 }] as DocTreePara[] };
    const mirror = new DocMirror(host, fakeEngine(paras, []), DEFAULT_MIRROR_LABELS, input);
    mirror.select({ p: 0, o: 0 }, false);
    await mirror.idle();
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'X', bubbles: true }));
    paras.current = [{ p: 0, text: 'Xabc', page: 0 }];
    // A movement response flushes after the edit is in the engine but before its result.
    mirror.select({ p: 0, o: 0 }, false);
    await mirror.idle();
    mirror.select({ p: 0, o: 1 }, true);
    await mirror.idle();
    expect(mirror.live.textContent).toBe('X');
    mirror.dispose();
  });
});
