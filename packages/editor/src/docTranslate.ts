/**
 * BlueDoc translation (CORE-LANG-001).
 *
 * - Selection: its text comes from the engine (`state` + `speechText`, byte offsets of `Pos`);
 *   Replace types the translation over the selection (`insertText`, formatting of the replaced
 *   text's start), Insert types it after the selection.
 * - Whole document: the engine lists the text segments of every body paragraph
 *   (`translationSegments`; a segment stops at tabs, breaks, pictures, fields), they are
 *   translated, then written into a **copy** of the document (`translationApply`), which shares
 *   each segment's translation among its runs in proportion to their length — paragraph styles
 *   and run formatting are kept, paragraph count unchanged (`bo_edit::translate`). Headers,
 *   footers, notes, comments and text boxes are not translated.
 */
import {
  byteToUtf16,
  translateTexts,
  type DocTextSegment,
  type EditCommand,
  type EditResult,
  type TranslateTextsOptions,
  type TranslationProvider,
} from '@blueoffice/platform';

type Run = (cmd: EditCommand) => Promise<EditResult | null | undefined>;

const OBJECT = /￼/g;

const before = (a: { p: number; o: number }, b: { p: number; o: number }) =>
  a.p < b.p || (a.p === b.p && a.o < b.o);

/** Text of the selection (paragraphs joined by `\n`; objects left out). */
export async function docSelectionText(run: Run): Promise<string> {
  const state = await run({ cmd: 'state' });
  if (!state) return '';
  const { anchor, focus } = state.selection;
  if (anchor.p === focus.p && anchor.o === focus.o) return '';
  const [from, to] = before(focus, anchor) ? [focus, anchor] : [anchor, focus];
  const r = await run({ cmd: 'speechText', from: from.p, to: to.p });
  const paras = r?.speech?.paras ?? [];
  return paras
    .map((x) => {
      const start = x.p === from.p ? byteToUtf16(x.text, from.o) : 0;
      const end = x.p === to.p ? byteToUtf16(x.text, to.o) : x.text.length;
      return x.text.slice(start, end).replace(OBJECT, '');
    })
    .join('\n');
}

/** Types `text` after the selection. */
export async function docInsertAfter(run: Run, text: string): Promise<void> {
  const state = await run({ cmd: 'state' });
  if (!state) return;
  const { anchor, focus } = state.selection;
  const end = before(anchor, focus) ? focus : anchor;
  await run({ cmd: 'setSelection', selection: { anchor: end, focus: end } });
  await run({ cmd: 'insertText', text: /^\s/.test(text) ? text : ` ${text}` });
}

/** Translated segments of the whole document (only those that change). */
export async function translateDocumentSegments(
  run: Run,
  provider: TranslationProvider,
  from: string,
  to: string,
  options: TranslateTextsOptions = {},
): Promise<DocTextSegment[]> {
  const r = await run({ cmd: 'translationSegments' });
  const segs = r?.translation ?? [];
  const out = await translateTexts(
    provider,
    segs.map((s) => s.text),
    from,
    to,
    options,
  );
  return segs.flatMap((s, i) => (out[i] !== s.text ? [{ ...s, text: out[i]! }] : []));
}

/** File name of the translated copy: `Báo cáo.docx` → `Báo cáo (EN).docx`. */
export function translatedName(name: string, to: string, ext = 'docx'): string {
  const stem = name.replace(/\.[^.]+$/, '') || name;
  return `${stem} (${to.toUpperCase()}).${ext}`;
}
