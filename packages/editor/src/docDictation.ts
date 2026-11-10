/**
 * BlueDoc dictation target (CORE-LANG-002): text goes in at the caret (over the selection)
 * through the document's command bridge — one `insertText` per final phrase (one undo step;
 * `\n` splits the paragraph, U+2028 is a line break as Shift+Enter); a deleted word first
 * extends the selection back over it. The paragraph text around the caret (spacing, capitals,
 * "xoá từ vừa rồi") comes from the engine's `speechText` index text. Interim text is drawn at
 * the caret by the view (`setDictationPreview`), never written.
 */
import {
  byteToUtf16,
  utf16ToByte,
  type EditCommand,
  type EditResult,
  type EngineHost,
  type Pos,
} from '@blueoffice/platform';
import type { DictationEdit, DictationTarget } from './dictation';

/** The view's dictation preview (`DocumentView.setDictationPreview`). */
export interface DictationPreviewer {
  setDictationPreview(text: string | null): void;
}

/** Line break inside a paragraph (as Shift+Enter: `keyToCommand`). */
export const DOC_LINE_BREAK = ' ';

const ordered = (a: Pos, b: Pos): [Pos, Pos] =>
  a.p < b.p || (a.p === b.p && a.o <= b.o) ? [a, b] : [b, a];

export function docDictationTarget(
  engine: EngineHost,
  /** Runs an edit the way the editor does (view refresh, errors). */
  run: (cmd: EditCommand) => Promise<EditResult | null>,
  view: () => DictationPreviewer | null,
): DictationTarget {
  const around = async () => {
    const st = await engine.command({ cmd: 'state' });
    const [from, to] = ordered(st.selection.anchor, st.selection.focus);
    const r = await engine.command({ cmd: 'speechText', from: from.p, to: from.p });
    const text = r.speech?.paras?.find((x) => x.p === from.p)?.text ?? '';
    return { from, to, text };
  };
  return {
    lineBreak: DOC_LINE_BREAK,
    paragraph: '\n',
    async context() {
      const { from, to, text } = await around();
      const before = text.slice(0, byteToUtf16(text, from.o));
      const after = to.p === from.p ? text.slice(byteToUtf16(text, to.o)) : '';
      return { before, after };
    },
    async apply(e: DictationEdit) {
      const { from, to, text } = await around();
      let start = from;
      if (e.remove > 0) {
        const u16 = Math.max(0, byteToUtf16(text, from.o) - e.remove);
        start = { p: from.p, o: utf16ToByte(text, u16) };
      }
      const selected = start.p !== to.p || start.o !== to.o;
      if (start !== from)
        await run({ cmd: 'setSelection', selection: { anchor: start, focus: to } });
      if (e.text) await run({ cmd: 'insertText', text: e.text });
      else if (selected) await run({ cmd: 'deleteBackward' });
    },
    preview(text) {
      view()?.setDictationPreview(text);
    },
  };
}
