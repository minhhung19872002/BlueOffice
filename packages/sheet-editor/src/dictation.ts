/**
 * BlueSheet dictation target (CORE-LANG-002): text goes into the active cell editor at its caret
 * (the cell's or the formula bar's input); when no edit is open, an in-place edit of the active
 * cell opens with its content and the caret at the end (Enter commits it, as typing would).
 * Line breaks and paragraphs are both `\n` (a line break in the cell, as Alt+Enter). Interim
 * text shows in the bar only.
 */
import type { DictationEdit, DictationTarget } from '@blueoffice/editor';

/** The open cell edit: its text and selection (UTF-16). */
export interface CellEditText {
  text: string;
  start: number;
  end: number;
}

export interface SheetDictationHost {
  /** The edit in progress, `null` when none. */
  editing(): CellEditText | null;
  /** Opens an in-place edit of the active cell (`null`: cannot, e.g. no workbook). */
  begin(): Promise<CellEditText | null>;
  /** Sets the edit's text and caret. */
  set(text: string, caret: number): void;
}

export function sheetDictationTarget(h: SheetDictationHost): DictationTarget {
  const current = async () => h.editing() ?? (await h.begin());
  return {
    lineBreak: '\n',
    paragraph: '\n',
    async context() {
      const c = await current();
      if (!c) return null;
      const lo = Math.min(c.start, c.end);
      const hi = Math.max(c.start, c.end);
      return { before: c.text.slice(0, lo), after: c.text.slice(hi) };
    },
    async apply(e: DictationEdit) {
      const c = await current();
      if (!c) return;
      const lo = Math.min(c.start, c.end);
      const hi = Math.max(c.start, c.end);
      const start = Math.max(0, lo - e.remove);
      h.set(c.text.slice(0, start) + e.text + c.text.slice(hi), start + e.text.length);
    },
    preview() {
      // The bar shows the interim text.
    },
  };
}
