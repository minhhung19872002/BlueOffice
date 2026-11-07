import { describe, expect, it } from 'vitest';
import type { Platform } from '@blueoffice/platform';
import { READ_ONLY_ERROR, SessionMode, guardPlatform } from '../src';

class Engine {
  calls: string[] = [];
  async command(c: { cmd: string }) {
    this.calls.push(c.cmd);
    return { cmd: c.cmd };
  }
  async insertImage() {
    this.calls.push('insertImage');
    return { cmd: 'insertImage' };
  }
  async slideCommand(c: { cmd: string }) {
    this.calls.push(`slide:${c.cmd}`);
    return { cmd: c.cmd };
  }
  async slideInsertPicture() {
    this.calls.push('slideInsertPicture');
    return {};
  }
  async saveDocx() {
    return new Uint8Array([1]);
  }
}

class Sheet {
  calls: string[] = [];
  async setInputs() {
    this.calls.push('setInputs');
  }
  async undo() {
    this.calls.push('undo');
    return true;
  }
  async exec(c: { cmd: string }) {
    this.calls.push(`exec:${c.cmd}`);
    return c.cmd;
  }
  async save() {
    return new Uint8Array([2]);
  }
}

function platform() {
  const engine = new Engine();
  const sheet = new Sheet();
  return { p: { engine, sheet } as unknown as Platform, engine, sheet };
}

describe('view-mode guard', () => {
  it('passes everything through while editable', async () => {
    const { p, engine, sheet } = platform();
    const g = guardPlatform(p, new SessionMode('edit'));
    await g.engine.command({ cmd: 'insertText', text: 'a' });
    await g.engine.slideCommand({ cmd: 'delete' });
    await g.sheet!.setInputs(0, [[0, 0, '1']]);
    await expect(g.sheet!.exec({ cmd: 'sort' } as never)).resolves.toBe('sort');
    expect(engine.calls).toEqual(['insertText', 'slide:delete']);
    expect(sheet.calls).toEqual(['setInputs', 'exec:sort']);
  });

  it('turns edits into state reads once the mode becomes view', async () => {
    const { p, engine, sheet } = platform();
    const mode = new SessionMode('edit');
    const g = guardPlatform(p, mode);
    mode.set('view');
    await expect(g.engine.command({ cmd: 'insertText', text: 'a' })).resolves.toEqual({
      cmd: 'state',
    });
    await g.engine.command({ cmd: 'move', motion: 'left' });
    await g.engine.command({ cmd: 'copy' });
    await g.engine.insertImage(new Uint8Array(), 'a.png');
    await g.engine.slideCommand({ cmd: 'transform', ids: [], dx: 1 } as never);
    await g.engine.slideCommand({ cmd: 'goTo', slide: 1 });
    await g.engine.slideInsertPicture(new Uint8Array(), 'b.png');
    expect(engine.calls).toEqual([
      'state',
      'move',
      'copy',
      'state',
      'slide:state',
      'slide:goTo',
      'slide:state',
    ]);
    // Saving / exporting still works.
    await expect(g.engine.saveDocx()).resolves.toEqual(new Uint8Array([1]));

    await g.sheet!.setInputs(0, [[0, 0, '1']]);
    await expect(g.sheet!.undo()).resolves.toBe(false);
    await expect(g.sheet!.exec({ cmd: 'find' } as never)).resolves.toBe('find');
    await expect(g.sheet!.exec({ cmd: 'format' } as never)).rejects.toThrow(READ_ONLY_ERROR);
    expect(sheet.calls).toEqual(['exec:find']);
    await expect(g.sheet!.save('xlsx', 0)).resolves.toEqual(new Uint8Array([2]));
  });
});
