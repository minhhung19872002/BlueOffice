/**
 * BlueSlide dictation target (CORE-LANG-002): text goes into the text box (or table cell) being
 * edited, at its caret / over its selection — one `setText` per final phrase, so one undo step.
 * Line breaks are `\v` (the editing format's `a:br`), paragraphs `\n`. Without a text being
 * edited there is nowhere to write (`context` is `null`); interim text shows in the bar only.
 */
import type { DictationEdit, DictationTarget } from '@blueoffice/editor';
import type { SlideCommand, SlideEditResult, SlideTextEdit } from '@blueoffice/platform';

export interface SlideDictationSource {
  run(cmd: SlideCommand): Promise<SlideEditResult | null>;
  /** The latest engine result (updated as results arrive). */
  latest(): SlideEditResult | null;
  /** Commands queued before (typing) are done. */
  idle(): Promise<unknown>;
}

const range = (te: SlideTextEdit): [number, number] => [
  Math.min(te.start, te.end),
  Math.max(te.start, te.end),
];

export function slideDictationTarget(src: SlideDictationSource): DictationTarget {
  const editing = async () => {
    await src.idle();
    return src.latest()?.textEdit ?? null;
  };
  return {
    lineBreak: '\v',
    paragraph: '\n',
    async context() {
      const te = await editing();
      if (!te) return null;
      const [lo, hi] = range(te);
      return { before: te.text.slice(0, lo), after: te.text.slice(hi) };
    },
    async apply(e: DictationEdit) {
      const te = await editing();
      if (!te) return;
      const [lo, hi] = range(te);
      const start = Math.max(0, lo - e.remove);
      const text = te.text.slice(0, start) + e.text + te.text.slice(hi);
      const caret = start + e.text.length;
      await src.run({ cmd: 'setText', id: te.id, text, sel: [caret, caret], cell: te.cell });
    },
    preview() {
      // The bar shows the interim text.
    },
  };
}
