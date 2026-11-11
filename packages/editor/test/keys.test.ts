import { describe, expect, it } from 'vitest';
import type { EditCommand, EditResult } from '@blueoffice/platform';
import { DocController } from '../src/controller';
import { keyToCommand } from '../src/keys';

const k = (key: string, mods: Partial<{ ctrl: boolean; shift: boolean; alt: boolean }> = {}) => ({
  key,
  ctrl: false,
  shift: false,
  alt: false,
  ...mods,
});

describe('keyToCommand', () => {
  it('maps caret and editing keys', () => {
    expect(keyToCommand(k('ArrowLeft'))).toEqual({ cmd: 'move', motion: 'left', extend: false });
    expect(keyToCommand(k('ArrowRight', { ctrl: true, shift: true }))).toEqual({
      cmd: 'move',
      motion: 'wordRight',
      extend: true,
    });
    expect(keyToCommand(k('End', { ctrl: true }))).toEqual({
      cmd: 'move',
      motion: 'docEnd',
      extend: false,
    });
    expect(keyToCommand(k('Backspace'))).toEqual({ cmd: 'deleteBackward' });
    expect(keyToCommand(k('Delete'))).toEqual({ cmd: 'deleteForward' });
    expect(keyToCommand(k('Enter'))).toEqual({ cmd: 'splitParagraph' });
    expect(keyToCommand(k('Enter', { shift: true }))).toEqual({
      cmd: 'insertText',
      text: '\u2028',
    });
    expect(keyToCommand(k('Enter', { ctrl: true }))).toBeNull();
    expect(keyToCommand(k('Tab'))).toEqual({ cmd: 'tab', back: false });
    expect(keyToCommand(k('Tab', { shift: true }))).toEqual({ cmd: 'tab', back: true });
    expect(keyToCommand(k('b', { ctrl: true }))).toBeNull();
  });

  it('Ctrl+Backspace / Ctrl+Delete delete a word, Ctrl+Up / Ctrl+Down go by paragraph', () => {
    expect(keyToCommand(k('Backspace', { ctrl: true }))).toEqual({ cmd: 'deleteWordBackward' });
    expect(keyToCommand(k('Delete', { ctrl: true }))).toEqual({ cmd: 'deleteWordForward' });
    expect(keyToCommand(k('ArrowUp', { ctrl: true, shift: true }))).toEqual({
      cmd: 'move',
      motion: 'paraUp',
      extend: true,
    });
    expect(keyToCommand(k('ArrowDown', { ctrl: true }))).toEqual({
      cmd: 'move',
      motion: 'paraDown',
      extend: false,
    });
  });
});

describe('DocController Ctrl+click', () => {
  const result = (link?: string): EditResult =>
    ({
      info: { name: 'a', pageCount: 1, pages: [], wordCount: 0 },
      selection: { anchor: { p: 0, o: 0 }, focus: { p: 0, o: 0 } },
      caret: null,
      rects: [],
      dirty: [],
      format: (link ? { link } : {}) as EditResult['format'],
      modified: false,
    }) as EditResult;
  const setup = (link?: string) => {
    const sent: string[] = [];
    const engine = {
      command: async ({ cmd }: EditCommand & { now?: number }) => {
        sent.push(cmd);
        return result(cmd === 'click' ? link : undefined);
      },
    };
    const clipboard = {
      readText: async () => '',
      writeText: async () => {},
      writeHtml: async () => {},
    };
    const c = new DocController(
      engine as never,
      { refresh: () => {}, setSelection: () => {} },
      clipboard,
      () => {},
      () => {},
    );
    const followed: string[] = [];
    c.onFollowLink = (t) => followed.push(t);
    return { c, sent, followed };
  };
  const down = { type: 'down' as const, page: 0, x: 10, y: 10, shift: false, count: 1, ctrl: true };

  it('selects the sentence on text, in one step of the queue', async () => {
    const { c, sent, followed } = setup();
    c.pointer(down);
    await c.copy();
    expect(sent).toEqual(['click', 'selectSentence', 'copy']);
    expect(followed).toEqual([]);
  });

  it('follows a hyperlink', async () => {
    const { c, sent, followed } = setup('https://bluestar.vn');
    c.pointer(down);
    await c.idle();
    expect(sent).toEqual(['click']);
    expect(followed).toEqual(['https://bluestar.vn']);
  });
});

describe('DocController', () => {
  it('serializes commands, coalesces drags and prefers the internal clipboard format', async () => {
    const sent: EditCommand[] = [];
    const written: string[] = [];
    const res = (clip?: EditResult['clip']): EditResult =>
      ({
        info: { name: 'a', pageCount: 1, pages: [], wordCount: 0 },
        selection: { anchor: { p: 0, o: 0 }, focus: { p: 0, o: 0 } },
        caret: null,
        rects: [],
        dirty: [],
        format: {} as EditResult['format'],
        modified: false,
        clip,
      }) as EditResult;
    const engine = {
      command: async ({ now: _now, ...c }: EditCommand & { now?: number }) => {
        sent.push(c as EditCommand);
        await new Promise((r) => setTimeout(r, 1));
        return res(c.cmd === 'copy' ? { text: 'x', html: '<b>x</b>', internal: '[1]' } : undefined);
      },
    };
    const view = { refresh: () => {}, setSelection: () => {} };
    const clipboard = {
      readText: async () => 'x',
      writeText: async (t: string) => void written.push(t),
      writeHtml: async (h: string) => void written.push(h),
    };
    const c = new DocController(
      engine as never,
      view,
      clipboard,
      () => {},
      () => {},
    );
    c.input({ type: 'insert', text: 'a' });
    c.input({ type: 'key', key: 'Enter', ctrl: false, shift: false, alt: false });
    for (let i = 0; i < 5; i++)
      c.pointer({ type: 'drag', page: 0, x: i, y: 0, shift: true, count: 1 });
    await c.idle();
    await new Promise((r) => setTimeout(r, 20));
    expect(sent[0]).toEqual({ cmd: 'insertText', text: 'a' });
    expect(sent[1]).toEqual({ cmd: 'splitParagraph' });
    expect(sent.filter((s) => s.cmd === 'dragTo').length).toBeLessThan(5);
    expect(sent.at(-1)).toEqual({ cmd: 'dragTo', page: 0, x: 4, y: 0 });
    await c.copy();
    expect(written).toEqual(['<b>x</b>']);
    await c.pasteFromClipboard();
    expect(sent.at(-1)).toEqual({
      cmd: 'paste',
      internal: '[1]',
      html: '',
      text: 'x',
      mode: 'keep',
    });
  });
});
