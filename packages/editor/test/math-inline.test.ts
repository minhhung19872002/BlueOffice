import { describe, expect, it } from 'vitest';
import type { EditCommand, EditResult, EngineHost, MathState } from '@blueoffice/platform';
import { DocController, type EditView } from '../src/controller';

function result(over: Partial<EditResult> = {}): EditResult {
  return {
    info: { name: 'a.docx', pageCount: 1, pages: [{ width: 595, height: 842 }], wordCount: 0 },
    selection: { anchor: { p: 0, o: 9 }, focus: { p: 0, o: 9 } },
    caret: { page: 0, x: 85, y: 56, h: 15 },
    rects: [],
    dirty: [],
    format: {} as EditResult['format'],
    modified: true,
    ...over,
  };
}

const zone = { page: 0, x: 80, y: 50, w: 30, h: 20 };

/** An engine that keeps the caret in an equation from `mathEnter` until it is left (Esc / an arrow
 * past its end), as the engine's math caret does (CORE-MATH-010). */
function fakeEngine() {
  const commands: EditCommand[] = [];
  let math: MathState | null = null;
  const engine = {
    command: async (c: EditCommand & { now?: number }) => {
      const cmd = Object.fromEntries(
        Object.entries(c).filter(([k]) => k !== 'now'),
      ) as unknown as EditCommand;
      commands.push(cmd);
      if (cmd.cmd === 'mathEnter')
        math = { path: [], offset: 0, selected: false, zone, placeholders: [] };
      if (cmd.cmd === 'tab' && math) math = { ...math, path: [[0, 0]], offset: 0 };
      if (cmd.cmd === 'deleteBackward' && math) math = { ...math, selected: true };
      if (cmd.cmd === 'mathMove' && cmd.motion === 'exit') math = null;
      return result(math ? { math, caret: { page: 0, x: 90, y: 52, h: 9 } } : {});
    },
  } as unknown as EngineHost;
  return { engine, commands };
}

const key = (k: string, shift = false) => ({
  type: 'key' as const,
  key: k,
  ctrl: false,
  shift,
  alt: false,
});

describe('editing keys inside an equation', () => {
  it('sends the ordinary editing commands and draws the math caret', async () => {
    const { engine, commands } = fakeEngine();
    const marks: (MathState | null | undefined)[] = [];
    const carets: EditResult['caret'][] = [];
    const view: EditView = {
      refresh: () => {},
      setSelection: (c) => void carets.push(c),
      setMathMarks: (m) => void marks.push(m),
    };
    const c = new DocController(
      engine,
      view,
      { readText: async () => '', writeText: async () => {}, writeHtml: async () => {} },
      () => {},
      (e) => {
        throw e;
      },
    );
    await c.run({ cmd: 'mathEnter', end: true });
    // Tab, Shift+Tab, arrows, Home / End, Backspace / Delete and typing reach the engine unchanged:
    // it routes them into the equation while its caret is in one.
    c.input(key('Tab'));
    c.input(key('Tab', true));
    c.input(key('ArrowRight'));
    c.input(key('Home'));
    c.input({ type: 'insert', text: 'x' });
    c.input({ type: 'insert', text: '^' });
    c.input({ type: 'insert', text: ' ' });
    c.input(key('Backspace'));
    c.input(key('Delete'));
    await c.idle();
    expect(commands).toEqual([
      { cmd: 'mathEnter', end: true },
      { cmd: 'tab', back: false },
      { cmd: 'tab', back: true },
      { cmd: 'move', motion: 'right', extend: false },
      { cmd: 'move', motion: 'lineStart', extend: false },
      { cmd: 'insertText', text: 'x' },
      { cmd: 'insertText', text: '^' },
      { cmd: 'insertText', text: ' ' },
      { cmd: 'deleteBackward' },
      { cmd: 'deleteForward' },
    ]);
    // Every result shows the equation's box and the caret inside it.
    expect(marks.every((m) => m?.zone.w === 30)).toBe(true);
    expect(marks[1]?.path).toEqual([[0, 0]]);
    expect(marks.at(-1)?.selected).toBe(true);
    expect(carets.at(-1)).toEqual({ page: 0, x: 90, y: 52, h: 9 });
    // Leaving the equation clears its marks.
    await c.run({ cmd: 'mathMove', motion: 'exit' });
    expect(marks.at(-1)).toBeNull();
  });
});
