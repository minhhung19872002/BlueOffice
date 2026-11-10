import { describe, expect, it } from 'vitest';
import type { Platform, SlideCommand, SlideEditResult } from '@blueoffice/platform';
import { SlideController } from '../src/controller';

const tick = () => new Promise((r) => setTimeout(r, 0));

function controller() {
  const seen: SlideCommand[] = [];
  const engine = {
    slideCommand: async (c: SlideCommand) => {
      seen.push(c);
      return (
        c.cmd === 'copy'
          ? { slide: 0, clip: 'shapes', clipboard: { text: 'Hộp mới' } }
          : { slide: 0 }
      ) as SlideEditResult;
    },
  };
  const ctrl = new SlideController(
    engine as unknown as Platform['engine'],
    () => {},
    (e) => {
      throw e;
    },
  );
  return { ctrl, seen };
}

describe('SlideController copy in flight (CORE-MISC-003)', () => {
  it('a paste right after a copy pastes the copy, not the older clipboard content', async () => {
    const { ctrl } = controller();
    let system = { text: 'chữ cũ của chương trình khác' };
    let finishWrite: () => void = () => {};
    const copying = ctrl.copy(
      { cmd: 'copy' },
      (r) =>
        new Promise<void>((resolve) => {
          finishWrite = () => {
            system = { text: r.clipboard?.text ?? '' };
            resolve();
          };
        }),
    );
    // The paste event's data was read before the copy reached the clipboard.
    const stale = { ...system };
    let read = 0;
    const pasting = ctrl.clipboardForPaste(async () => {
      read++;
      return stale;
    });
    await tick();
    expect(ctrl.copies.busy).toBe(true);
    finishWrite();
    expect((await copying)?.clipboard?.text).toBe('Hộp mới');
    // null: paste the engine's own copy; the stale content is never used.
    expect(await pasting).toBeNull();
    expect(read).toBe(0);
    expect(system.text).toBe('Hộp mới');
  });

  it('without a copy in flight the clipboard is read', async () => {
    const { ctrl } = controller();
    expect(await ctrl.clipboardForPaste(async () => ({ text: 'ngoài' }))).toEqual({
      text: 'ngoài',
    });
    // A copy that copied nothing does not hide the clipboard either.
    await ctrl.copy({ cmd: 'cut' }, async () => {});
    expect(await ctrl.clipboardForPaste(async () => ({ text: 'ngoài' }))).toEqual({
      text: 'ngoài',
    });
  });
});
