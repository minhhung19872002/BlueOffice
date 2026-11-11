import { describe, expect, it } from 'vitest';
import type { EditCommand, EditResult, EngineHost, MathState } from '@blueoffice/platform';
import {
  DocController,
  equationSelected,
  MATH_UNSUPPORTED,
  type EditView,
} from '../src/controller';

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

describe('an equation that cannot be edited in place', () => {
  it('is selected with the engine error; a double click or Enter opens the equation dialog', async () => {
    const commands: EditCommand[] = [];
    const eq = { linear: 'a/b', display: false, isLinear: false };
    const selected = { anchor: { p: 0, o: 6 }, focus: { p: 0, o: 9 } };
    const engine = {
      command: async (c: EditCommand & { now?: number }) => {
        const cmd = Object.fromEntries(
          Object.entries(c).filter(([k]) => k !== 'now'),
        ) as unknown as EditCommand;
        commands.push(cmd);
        // The engine refuses in-place editing and selects the equation (ADR-0015 addendum).
        if (cmd.cmd === 'click' && cmd.count === 1)
          return result({ selection: selected, equation: eq, error: MATH_UNSUPPORTED });
        if (cmd.cmd === 'click') return result({ selection: selected, equation: eq });
        // Elsewhere: the caret just after the equation.
        return result({ equation: eq });
      },
    } as unknown as EngineHost;
    const errors: (string | undefined)[] = [];
    const c = new DocController(
      engine,
      { refresh: () => {}, setSelection: () => {} },
      { readText: async () => '', writeText: async () => {}, writeHtml: async () => {} },
      (r) => void errors.push(r.error),
      (e) => {
        throw e;
      },
    );
    let dialogs = 0;
    c.onEquationDialog = () => {
      dialogs += 1;
    };
    const down = (count: number) => ({
      type: 'down' as const,
      page: 0,
      x: 90,
      y: 60,
      shift: false,
      count,
    });
    c.pointer(down(1));
    await c.idle();
    expect(errors).toEqual([MATH_UNSUPPORTED]);
    expect(dialogs).toBe(0);
    // Enter on the selected equation: the dialog, no paragraph split.
    c.input(key('Enter'));
    await c.idle();
    expect(dialogs).toBe(1);
    expect(commands.some((x) => x.cmd === 'splitParagraph')).toBe(false);
    // A double click: the dialog again.
    c.pointer(down(2));
    await c.idle();
    await Promise.resolve();
    expect(dialogs).toBe(2);
    // With the caret after the equation (not selected), Enter splits the paragraph.
    expect(equationSelected(await c.run({ cmd: 'state' }))).toBe(false);
    c.input(key('Enter'));
    await c.idle();
    expect(commands.at(-1)).toEqual({ cmd: 'splitParagraph' });
    expect(dialogs).toBe(2);
    // Select it again, then End and Enter typed at once: Enter is decided after End is
    // applied (the caret after the equation), so the paragraph splits.
    c.pointer(down(1));
    await c.idle();
    c.input(key('End'));
    c.input(key('Enter'));
    await c.idle();
    expect(commands.slice(-2)).toEqual([
      { cmd: 'move', motion: 'lineEnd', extend: false },
      { cmd: 'splitParagraph' },
    ]);
    expect(dialogs).toBe(2);
  });
});
