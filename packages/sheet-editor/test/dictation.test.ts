import { DictationSession } from '@blueoffice/editor';
import { FakeDictationProvider } from '@blueoffice/platform';
import { describe, expect, it } from 'vitest';
import { sheetDictationTarget, type CellEditText } from '../src/dictation';

function cellEditor(cell: string) {
  let edit: CellEditText | null = null;
  let begun = 0;
  const tg = sheetDictationTarget({
    editing: () => edit,
    begin: async () => {
      begun++;
      edit = { text: cell, start: cell.length, end: cell.length };
      return edit;
    },
    set: (text, caret) => {
      edit = { text, start: caret, end: caret };
    },
  });
  return { tg, edit: () => edit, begun: () => begun };
}

describe('BlueSheet dictation', () => {
  it('opens an in-place edit of the active cell and appends at its caret', async () => {
    const c = cellEditor('Ghi chú');
    const p = new FakeDictationProvider();
    const s = new DictationSession(p, c.tg, 'vi-VN');
    await s.start();
    p.final('hai chấm đã duyệt');
    p.final('xuống dòng hello');
    await s.written();
    expect(c.edit()).toEqual({
      text: 'Ghi chú: đã duyệt\nHello',
      start: 23,
      end: 23,
    });
    expect(c.begun()).toBe(1);
  });

  it('replaces the selection of an open edit', async () => {
    let edit: CellEditText | null = { text: 'Số XX tiền', start: 3, end: 5 };
    const tg = sheetDictationTarget({
      editing: () => edit,
      begin: async () => null,
      set: (text, caret) => {
        edit = { text, start: caret, end: caret };
      },
    });
    const p = new FakeDictationProvider();
    const s = new DictationSession(p, tg, 'vi-VN');
    await s.start();
    p.final('một trăm');
    await s.written();
    expect(edit).toEqual({ text: 'Số một trăm tiền', start: 11, end: 11 });
  });
});
