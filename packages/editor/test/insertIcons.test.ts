import { describe, expect, it } from 'vitest';
import type {
  EditCommand,
  EditResult,
  SheetCommand,
  SlideCommand,
  SlideEditResult,
} from '@blueoffice/platform';
import type { IconPick } from '@blueoffice/ui';
import { fromBase64, insertDocIcons, insertSheetIcons, insertSlideIcons } from '../src';

const SVG_A =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M0 0h24v24z"/></svg>';
// Non-ASCII content: the base64 payload must be UTF-8.
const SVG_B =
  '<svg xmlns="http://www.w3.org/2000/svg"><title>Lịch</title><rect width="4" height="4"/></svg>';
const icons: IconPick[] = [
  { id: 'arrow-right', name: 'Mũi tên phải', svg: SVG_A },
  { id: 'calendar', name: 'Lịch', svg: SVG_B },
];

/** Fake rasteriser: the "PNG" records which SVG and size it was made from. */
const raster = async (svg: string, size: number) =>
  new TextEncoder().encode(`${size}:${svg === SVG_A ? 'A' : 'B'}`);
const text = (b64: string) => new TextDecoder().decode(fromBase64(b64));

describe('insertDocIcons', () => {
  function doc(fail?: 'insert' | 'change') {
    const log: unknown[] = [];
    return {
      log,
      target: {
        run: async (cmd: EditCommand) => {
          log.push(cmd);
          if (cmd.cmd === 'historyLen') return { count: 7 } as EditResult;
          if (cmd.cmd === 'pictureChange' && fail === 'change')
            return { error: 'badImage' } as EditResult;
          return {} as EditResult;
        },
        insertImage: async (bytes: Uint8Array, name: string) => {
          log.push({ insertImage: new TextDecoder().decode(bytes), name });
          return (fail === 'insert' ? { error: 'readOnly' } : {}) as EditResult;
        },
      },
    };
  }

  it('reserves a one-inch box, swaps in the sharp picture with the name as alt text', async () => {
    const { log, target } = doc();
    await insertDocIcons(target, icons, raster);
    expect(log).toEqual([
      { cmd: 'historyLen' },
      // 96 px at 96 dpi = 72 pt: the engine sizes the picture by these pixels.
      { insertImage: '96:A', name: 'arrow-right.png' },
      { cmd: 'pictureChange', data: expect.any(String), name: 'Mũi tên phải' },
      { insertImage: '96:B', name: 'calendar.png' },
      { cmd: 'pictureChange', data: expect.any(String), name: 'Lịch' },
      // All of it is one undo step.
      { cmd: 'mergeHistory', since: 7 },
    ]);
    const changes = log.filter(
      (c): c is { cmd: 'pictureChange'; data: string } =>
        (c as EditCommand).cmd === 'pictureChange',
    );
    expect(changes.map((c) => text(c.data))).toEqual(['384:A', '384:B']);
  });

  it('stops at a refusal and still closes the undo step', async () => {
    const refused = doc('insert');
    await expect(insertDocIcons(refused.target, icons, raster)).rejects.toThrow('readOnly');
    expect(refused.log).toEqual([
      { cmd: 'historyLen' },
      { insertImage: '96:A', name: 'arrow-right.png' },
      { cmd: 'mergeHistory', since: 7 },
    ]);
    const bad = doc('change');
    await expect(insertDocIcons(bad.target, icons, raster)).rejects.toThrow('badImage');
    expect(bad.log.at(-1)).toEqual({ cmd: 'mergeHistory', since: 7 });
  });
});

describe('insertSlideIcons', () => {
  it('inserts SVG pictures named after the icon, the later ones beside the first', async () => {
    const log: SlideCommand[] = [];
    const target = {
      run: async (cmd: SlideCommand) => {
        log.push(cmd);
        return (cmd.cmd === 'historyLen' ? { history: 3 } : {}) as SlideEditResult;
      },
    };
    await insertSlideIcons(target, icons);
    expect(log).toEqual([
      { cmd: 'historyLen' },
      { cmd: 'pictureInsertSvg', data: expect.any(String), name: 'Mũi tên phải' },
      { cmd: 'pictureInsertSvg', data: expect.any(String), name: 'Lịch' },
      // 72 pt icon + 12 pt gap to the right of the first one.
      { cmd: 'transform', transform: { kind: 'move', dx: 84, dy: 0 } },
      { cmd: 'mergeHistory', since: 3 },
    ]);
    // The SVG documents go to the engine unchanged (UTF-8).
    const sent = log.filter((c) => c.cmd === 'pictureInsertSvg').map((c) => text(c.data));
    expect(sent).toEqual([SVG_A, SVG_B]);
  });

  it('starts a new row after five icons', async () => {
    const moves: unknown[] = [];
    const target = {
      run: async (cmd: SlideCommand) => {
        if (cmd.cmd === 'transform') moves.push(cmd.transform);
        return {} as SlideEditResult;
      },
    };
    const seven = Array.from({ length: 7 }, (_, i) => ({ ...icons[0]!, id: `i${i}` }));
    await insertSlideIcons(target, seven);
    expect(moves).toEqual([
      { kind: 'move', dx: 84, dy: 0 },
      { kind: 'move', dx: 168, dy: 0 },
      { kind: 'move', dx: 252, dy: 0 },
      { kind: 'move', dx: 336, dy: 0 },
      { kind: 'move', dx: 0, dy: 84 },
      { kind: 'move', dx: 84, dy: 84 },
    ]);
  });

  it('reports a refused insertion', async () => {
    const target = {
      run: async (cmd: SlideCommand) =>
        (cmd.cmd === 'pictureInsertSvg' ? { error: 'badImage' } : {}) as SlideEditResult,
    };
    await expect(insertSlideIcons(target, icons)).rejects.toThrow('badImage');
  });
});

describe('insertSheetIcons', () => {
  it('places PNG pictures in the active cell and the cells to its right', async () => {
    const log: SheetCommand[] = [];
    await insertSheetIcons(
      async (cmd) => void log.push(cmd),
      { sheet: 2, row: 4, col: 1 },
      icons,
      raster,
    );
    expect(log).toEqual([
      {
        cmd: 'placeImageInCell',
        sheet: 2,
        row: 4,
        col: 1,
        data: expect.any(String),
        ext: 'png',
        alt: 'Mũi tên phải',
      },
      {
        cmd: 'placeImageInCell',
        sheet: 2,
        row: 4,
        col: 2,
        data: expect.any(String),
        ext: 'png',
        alt: 'Lịch',
      },
    ]);
    expect(log.map((c) => (c.cmd === 'placeImageInCell' ? text(c.data) : ''))).toEqual([
      '384:A',
      '384:B',
    ]);
  });
});
