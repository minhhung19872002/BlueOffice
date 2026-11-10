/**
 * Overtype mode (Insert key; status bar item, CORE-UI-007): typed text replaces the
 * characters after the caret instead of pushing them. The paragraph mark is never typed
 * over — at the end of a paragraph the text is inserted.
 */
import type { EditCommand, EditResult } from '@blueoffice/platform';

type Run = (c: EditCommand) => Promise<EditResult | null>;
type Sel = EditResult['selection'];

const collapsed = (s: Sel) => s.anchor.p === s.focus.p && s.anchor.o === s.focus.o;

/** Types `text` over what follows the caret (a range selection is replaced as usual). */
export async function overtypeInsert(
  run: Run,
  text: string,
  selection: Sel | undefined,
): Promise<EditResult | null> {
  if (selection && collapsed(selection)) {
    const anchor = selection.anchor;
    let focus = anchor;
    for (let i = [...text].length; i > 0; i--) {
      const r = await run({ cmd: 'move', motion: 'right', extend: true });
      const f = r?.selection.focus;
      // Stop at the end of the paragraph (the selection would reach the next one).
      if (!f || f.p !== anchor.p || f.o <= focus.o) break;
      focus = f;
    }
    await run({ cmd: 'setSelection', selection: { anchor, focus } });
  }
  return run({ cmd: 'insertText', text });
}
