/**
 * BlueDoc thesaurus (CORE-THES-001): the word under the caret (or the one-word selection) comes
 * from the engine (`state` + `speechText`, byte offsets of `Pos`); replacing selects it and types
 * the synonym over it (`insertText`: one undo step, formatting of the replaced word's start).
 */
import {
  byteToUtf16,
  utf16ToByte,
  wordAtCaret,
  type EditCommand,
  type EditResult,
} from '@blueoffice/platform';

type Run = (cmd: EditCommand) => Promise<EditResult | null | undefined>;

/** A word of a body paragraph: paragraph id and byte offsets. */
export interface DocWord {
  p: number;
  start: number;
  end: number;
  word: string;
}

/** The selected word, or the word at the caret; `null` when there is none. */
export async function docWordAtCaret(run: Run): Promise<DocWord | null> {
  const state = await run({ cmd: 'state' });
  if (!state) return null;
  const { anchor, focus } = state.selection;
  if (anchor.p !== focus.p) return null;
  const r = await run({ cmd: 'speechText', from: anchor.p, to: anchor.p });
  const text = r?.speech?.paras?.find((x) => x.p === anchor.p)?.text;
  if (text === undefined) return null;
  const lo = Math.min(anchor.o, focus.o);
  const hi = Math.max(anchor.o, focus.o);
  if (lo !== hi) {
    // A selection counts when it is one word (white space around it is left out).
    const a = byteToUtf16(text, lo);
    const b = byteToUtf16(text, hi);
    const w = wordAtCaret(text, a);
    if (!w) return null;
    const sel = text.slice(a, b).trim();
    if (sel !== w.word) return null;
    return {
      p: anchor.p,
      start: utf16ToByte(text, w.from),
      end: utf16ToByte(text, w.to),
      word: w.word,
    };
  }
  const w = wordAtCaret(text, byteToUtf16(text, lo));
  if (!w) return null;
  return {
    p: anchor.p,
    start: utf16ToByte(text, w.from),
    end: utf16ToByte(text, w.to),
    word: w.word,
  };
}

/**
 * Replaces the word at the caret by `text` when it is still `expected`; `false` otherwise.
 * The caret ends after the new word (so it is the word at the caret for a next replacement).
 */
export async function docReplaceWord(run: Run, expected: string, text: string): Promise<boolean> {
  const w = await docWordAtCaret(run);
  if (!w || w.word !== expected) return false;
  await run({
    cmd: 'setSelection',
    selection: { anchor: { p: w.p, o: w.start }, focus: { p: w.p, o: w.end } },
  });
  await run({ cmd: 'insertText', text });
  return true;
}
