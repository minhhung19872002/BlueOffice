/**
 * BlueSlide thesaurus (CORE-THES-001): the word at the caret (or the one-word selection) of the
 * text being edited; replacing splices the synonym into the shape's text with one `setText`
 * (one undo step; the engine keeps the formatting of the unchanged text and gives the new word
 * the formatting of the replaced one).
 */
import { wordAtCaret, type SlideCommand, type SlideEditResult } from '@blueoffice/platform';

type SlideRun = (cmd: SlideCommand) => Promise<SlideEditResult | null | undefined>;

/** The word at the caret of `text` (UTF-16 selection `start..end`); a selection must be one word. */
export function wordInText(
  text: string,
  start: number,
  end: number,
): { from: number; to: number; word: string } | null {
  const lo = Math.min(start, end);
  const hi = Math.max(start, end);
  const w = wordAtCaret(text, lo);
  if (!w) return null;
  if (lo !== hi && text.slice(lo, hi).trim() !== w.word) return null;
  return w;
}

/** The word at the caret of the edited text. */
export function slideWordAtCaret(
  edit: SlideEditResult | null | undefined,
): { from: number; to: number; word: string } | null {
  const te = edit?.textEdit;
  return te ? wordInText(te.text, te.start, te.end) : null;
}

/**
 * Replaces the word at the caret of the edited text (read from the engine) by `text` when it is
 * still `expected`; `false` otherwise.
 */
export async function slideReplaceWord(
  run: SlideRun,
  expected: string,
  text: string,
): Promise<boolean> {
  const edit = await run({ cmd: 'state' });
  const te = edit?.textEdit;
  const w = slideWordAtCaret(edit);
  if (!te || !w || w.word !== expected) return false;
  const next = te.text.slice(0, w.from) + text + te.text.slice(w.to);
  const caret = w.from + text.length;
  await run({ cmd: 'setText', id: te.id, text: next, sel: [caret, caret], cell: te.cell });
  return true;
}
