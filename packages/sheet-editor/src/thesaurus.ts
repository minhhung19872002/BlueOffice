/**
 * BlueSheet thesaurus (CORE-THES-001): a cell holding one word (not a formula) is the word the
 * pane works on; a replacement writes the cell as text (one undoable engine edit).
 */
import { wordAtCaret } from '@blueoffice/platform';

/** The word of a cell that holds exactly one word (formulas and numbers are not words). */
export function cellWord(input: string, display: string): string | null {
  if (input.startsWith('=')) return null;
  const text = display.trim();
  if (!text) return null;
  const w = wordAtCaret(text, 0);
  return w && w.from === 0 && w.to === text.length ? w.word : null;
}
