/**
 * `addin.selection` per application (CORE-ADDIN-001): the current selection
 * and its content, read through the BlueScript host (ADR-0012) like any other
 * call. Needs only the `selection` permission — the rest of the document stays
 * out of reach without `read-document`.
 */
import type { AddinSelection } from '@blueoffice/addin-sdk';
import { rangeAddress, type ScriptHost } from '@blueoffice/script';

/** Cells `addin.selection` returns at most (larger selections return the address only). */
export const MAX_SELECTION_CELLS = 10_000;

interface DocPos {
  paragraph: number;
  offset: number;
}

const before = (a: DocPos, b: DocPos) =>
  a.paragraph < b.paragraph || (a.paragraph === b.paragraph && a.offset <= b.offset);

/** BlueDoc: anchor, focus and the selected text (paragraphs joined by `\n`). */
export async function docSelection(host: ScriptHost): Promise<AddinSelection> {
  const sel = (await host.call('doc.selection', [])) as { anchor: DocPos; focus: DocPos };
  const [from, to] = before(sel.anchor, sel.focus)
    ? [sel.anchor, sel.focus]
    : [sel.focus, sel.anchor];
  let text = '';
  if (from.paragraph !== to.paragraph || from.offset !== to.offset) {
    const r = (await host.call('doc.command', [
      { cmd: 'speechText', from: from.paragraph, to: to.paragraph },
    ])) as { speech?: { paras?: { p: number; text: string }[] } };
    const parts: string[] = [];
    for (const p of r.speech?.paras ?? []) {
      if (p.p < from.paragraph || p.p > to.paragraph) continue;
      const start = p.p === from.paragraph ? from.offset : 0;
      const end = p.p === to.paragraph ? to.offset : p.text.length;
      parts.push(p.text.slice(start, end));
    }
    text = parts.join('\n');
  }
  return { app: 'doc', anchor: sel.anchor, focus: sel.focus, text };
}

export interface SheetSelectionState {
  sheet: number;
  sheetName: string;
  range: { r0: number; c0: number; r1: number; c1: number };
  active: { row: number; col: number };
}

/** BlueSheet: the selected range, its active cell and values. */
export async function sheetSelection(
  host: ScriptHost,
  s: SheetSelectionState,
): Promise<AddinSelection> {
  const address = rangeAddress(s.range);
  const cells = (s.range.r1 - s.range.r0 + 1) * (s.range.c1 - s.range.c0 + 1);
  const values =
    cells <= MAX_SELECTION_CELLS
      ? ((await host.call('sheet.getValues', [s.sheet, address])) as (
          string | number | boolean | null
        )[][])
      : [];
  return {
    app: 'sheet',
    sheet: s.sheet,
    sheetName: s.sheetName,
    address,
    active: rangeAddress({
      r0: s.active.row,
      c0: s.active.col,
      r1: s.active.row,
      c1: s.active.col,
    }),
    values,
  };
}

/** BlueSlide: the slide shown and the ids of the selected shapes. */
export function slideSelection(slide: number, shapes: readonly number[]): AddinSelection {
  return { app: 'slide', slide, shapes: [...shapes] };
}
