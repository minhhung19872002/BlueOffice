import type { EditCommand, EditResult } from '@blueoffice/platform';
import { describe, expect, it } from 'vitest';
import { overtypeInsert } from '../src/overtype';

type Sel = EditResult['selection'];

/** A one-paragraph-per-line document that understands the commands overtype sends. */
function fakeDoc(lines: string[], at: { p: number; o: number }) {
  let sel: Sel = { anchor: at, focus: at } as Sel;
  const log: string[] = [];
  const result = () => ({ selection: sel }) as EditResult;
  const run = async (c: EditCommand): Promise<EditResult | null> => {
    const cmd = c as { cmd: string; [k: string]: unknown };
    log.push(cmd.cmd);
    if (cmd.cmd === 'move') {
      const f = sel.focus;
      const line = lines[f.p] ?? '';
      const focus =
        f.o < line.length
          ? { p: f.p, o: f.o + 1 }
          : f.p + 1 < lines.length
            ? { p: f.p + 1, o: 0 }
            : f;
      sel = { anchor: sel.anchor, focus } as Sel;
    } else if (cmd.cmd === 'setSelection') {
      sel = cmd.selection as Sel;
    } else if (cmd.cmd === 'insertText') {
      const [a, b] = [sel.anchor, sel.focus];
      const line = lines[a.p] ?? '';
      lines[a.p] = line.slice(0, a.o) + String(cmd.text) + line.slice(b.o);
      const o = a.o + String(cmd.text).length;
      sel = { anchor: { p: a.p, o }, focus: { p: a.p, o } } as Sel;
    }
    return result();
  };
  return { run, lines, log, selection: () => sel };
}

describe('overtype mode', () => {
  it('typed characters replace the ones after the caret', async () => {
    const d = fakeDoc(['Hello world'], { p: 0, o: 6 });
    await overtypeInsert(d.run, 'W', d.selection());
    expect(d.lines[0]).toBe('Hello World');
    await overtypeInsert(d.run, 'ORL', d.selection());
    expect(d.lines[0]).toBe('Hello WORLd');
    expect(d.selection().focus.o).toBe(10);
  });

  it('never types over the paragraph mark', async () => {
    const d = fakeDoc(['abc', 'next'], { p: 0, o: 2 });
    // Two characters typed with one left in the paragraph: one replaced, one inserted.
    await overtypeInsert(d.run, 'XY', d.selection());
    expect(d.lines).toEqual(['abXY', 'next']);
    await overtypeInsert(d.run, 'Z', d.selection());
    expect(d.lines).toEqual(['abXYZ', 'next']);
  });

  it('a range selection is replaced as in insert mode', async () => {
    const d = fakeDoc(['abcdef'], { p: 0, o: 1 });
    const sel = { anchor: { p: 0, o: 1 }, focus: { p: 0, o: 3 } } as Sel;
    await d.run({ cmd: 'setSelection', selection: sel });
    d.log.length = 0;
    await overtypeInsert(d.run, 'X', d.selection());
    expect(d.lines[0]).toBe('aXdef');
    expect(d.log).toEqual(['insertText']);
  });

  it('counts characters, not UTF-16 units', async () => {
    const d = fakeDoc(['abcd'], { p: 0, o: 0 });
    await overtypeInsert(d.run, '😀', d.selection());
    expect(d.log.filter((c) => c === 'move')).toHaveLength(1);
    expect(d.lines[0]).toBe('😀bcd');
  });
});
