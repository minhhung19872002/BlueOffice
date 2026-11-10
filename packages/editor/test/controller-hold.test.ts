import { describe, expect, it } from 'vitest';
import type { EditCommand, EditResult } from '@blueoffice/platform';
import { DocController } from '../src/controller';

/**
 * A save waits for `idle()`. An operation of several queued commands (insert a picture,
 * swap its data, merge the history) leaves the queue empty between its steps, so without
 * `hold` a save could slip in between them (the icon insertion saved a placeholder and the
 * document was modified again right after the save).
 */
describe('DocController.hold', () => {
  function controller() {
    const sent: string[] = [];
    const engine = {
      command: async (c: EditCommand) => {
        sent.push(c.cmd);
        await new Promise((r) => setTimeout(r, 2));
        return { cmd: c.cmd } as unknown as EditResult;
      },
    };
    const view = { refresh: () => {}, setSelection: () => {} };
    const clipboard = {
      readText: async () => '',
      writeText: async () => {},
      writeHtml: async () => {},
    };
    const c = new DocController(
      engine as never,
      view,
      clipboard,
      () => {},
      () => {},
    );
    return { c, sent };
  }

  async function steps(c: DocController) {
    await c.run({ cmd: 'insertText', text: 'a' } as EditCommand);
    // A pause between steps, like rendering a PNG between two engine calls.
    await new Promise((r) => setTimeout(r, 15));
    await c.run({ cmd: 'insertText', text: 'b' } as EditCommand);
  }

  it('idle waits for a held operation as a whole', async () => {
    const { c, sent } = controller();
    void c.hold(steps(c));
    await new Promise((r) => setTimeout(r, 5));
    await c.idle();
    sent.push('save');
    expect(sent).toEqual(['insertText', 'insertText', 'save']);
  });

  it('without hold a save can come between the steps (the race this prevents)', async () => {
    const { c, sent } = controller();
    void steps(c);
    await new Promise((r) => setTimeout(r, 5));
    await c.idle();
    sent.push('save');
    expect(sent).toEqual(['insertText', 'save']);
  });

  it('a failed held operation does not block idle', async () => {
    const { c } = controller();
    const op = c.hold(Promise.reject(new Error('boom')));
    await expect(op).rejects.toThrow('boom');
    let done = false;
    await c.idle().then(() => (done = true));
    expect(done).toBe(true);
  });
});
