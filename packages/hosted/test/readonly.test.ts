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

  it('comment mode runs only comment commands; the host cap limits the server mode', async () => {
    const { p, engine, sheet } = platform();
    // The user may edit, but the host opened the file with `mode: 'comment'`.
    const mode = new SessionMode('edit', 'comment');
    expect(mode.value).toBe('comment');
    expect(mode.editable).toBe(false);
    expect(mode.canComment).toBe(true);
    const g = guardPlatform(p, mode);
    await g.engine.command({ cmd: 'insertText', text: 'a' });
    await g.engine.command({ cmd: 'addComment', text: 'Xem lại số liệu' });
    await g.engine.command({ cmd: 'replyComment', id: 1, text: 'Đã sửa' });
    await g.engine.command({ cmd: 'resolveComment', id: 1, done: true });
    await g.engine.command({ cmd: 'deleteComment', id: 1 });
    expect(engine.calls).toEqual([
      'state',
      'addComment',
      'replyComment',
      'resolveComment',
      'deleteComment',
    ]);
    await g.sheet!.setInputs(0, [[0, 0, '1']]);
    expect(sheet.calls).toEqual([]);
    // `mode-changed` back to edit never goes above the cap; a downgrade applies.
    mode.set('edit');
    expect(mode.value).toBe('comment');
    mode.set('view');
    expect(mode.value).toBe('view');
    await g.engine.command({ cmd: 'addComment', text: 'x' });
    expect(engine.calls.at(-1)).toBe('state');
  });
});

describe('background queries', () => {
  it('are read-only, so they neither mark a session dirty nor are refused in view mode', async () => {
    const { DOC_READ_COMMANDS, SLIDE_READ_COMMANDS } = await import('../src/readonly');
    for (const cmd of [
      'spelling',
      'spellAt',
      'outline',
      'wordCount',
      'styleList',
      'styleDetails',
    ]) {
      expect(DOC_READ_COMMANDS.has(cmd), cmd).toBe(true);
    }
    for (const cmd of ['vbaProject', 'designIdeas', 'actionInfo', 'inkInfo']) {
      expect(SLIDE_READ_COMMANDS.has(cmd), cmd).toBe(true);
    }
  });
});

describe('dirty tracking', () => {
  it('a spelling query after a save leaves the session clean; an edit makes it dirty', async () => {
    const { DirtyState, trackDirty } = await import('../src/dirty');
    const engine = new Engine();
    const state = new DirtyState();
    const p = trackDirty({ engine } as unknown as Platform, state);
    await p.engine.command({ cmd: 'spelling' } as never);
    await p.engine.command({ cmd: 'wordCount' } as never);
    expect(state.dirty).toBe(false);
    await p.engine.command({ cmd: 'insertText', text: 'a' } as never);
    expect(state.dirty).toBe(true);
  });
});
