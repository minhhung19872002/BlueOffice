/**
 * BlueSlide translation (CORE-LANG-001).
 *
 * - While a text is edited, the selected text goes to the pane; Replace / Insert splice the
 *   translation into the shape's text (`setText`).
 * - Selected shapes (not editing): each shape's text is translated in place.
 * - Whole deck: every slide's shape texts and speaker notes are read and translated first, then
 *   written into a **copy** of the deck (the current state saved and opened as a new file).
 *
 * Shape text is translated paragraph by paragraph (`\n`) and line by line (`\v` line breaks
 * stay where they are); `setText` keeps the paragraph-level formatting of each paragraph.
 */
import {
  translateTexts,
  type SlideCommand,
  type SlideEditResult,
  type SlideShapeInfo,
  type TranslateTextsOptions,
  type TranslationProvider,
} from '@blueoffice/platform';

export type SlideRun = (cmd: SlideCommand) => Promise<SlideEditResult | null>;

/** Lines of a shape text with their separators (`\n` paragraph, `\v` line break). */
export function textLines(text: string): { lines: string[]; seps: string[] } {
  const lines: string[] = [];
  const seps: string[] = [];
  let cur = '';
  for (const ch of text) {
    if (ch === '\n' || ch === '\v') {
      lines.push(cur);
      seps.push(ch);
      cur = '';
    } else cur += ch;
  }
  lines.push(cur);
  return { lines, seps };
}

export function joinLines(lines: readonly string[], seps: readonly string[]): string {
  return lines.map((l, i) => l + (seps[i] ?? '')).join('');
}

/** Translates several shape texts (each kept line by line). */
export async function translateShapeTexts(
  provider: TranslationProvider,
  texts: readonly string[],
  from: string,
  to: string,
  options: TranslateTextsOptions = {},
): Promise<string[]> {
  const split = texts.map(textLines);
  const flat = split.flatMap((s) => s.lines);
  const out = await translateTexts(provider, flat, from, to, options);
  let k = 0;
  return split.map((s) => {
    const lines = s.lines.map(() => out[k++]!);
    return joinLines(lines, s.seps);
  });
}

const withText = (s: SlideShapeInfo) => s.editableText && s.kind !== 'table' && !!s.text?.trim();

/** The selected text while editing (UTF-16 range of the edited text). */
export function editedSelection(edit: SlideEditResult | null): string {
  const te = edit?.textEdit;
  if (!te) return '';
  const lo = Math.min(te.start, te.end);
  const hi = Math.max(te.start, te.end);
  return te.text.slice(lo, hi).replace(/\v/g, '\n');
}

/** Splices `text` into the edited text over the selection (`replace`) or after it. */
export async function spliceEdited(
  run: SlideRun,
  edit: SlideEditResult | null,
  text: string,
  replace: boolean,
): Promise<void> {
  const te = edit?.textEdit;
  if (!te) return;
  const lo = Math.min(te.start, te.end);
  const hi = Math.max(te.start, te.end);
  const piece = replace || /^\s/.test(text) ? text : ` ${text}`;
  const next = replace
    ? te.text.slice(0, lo) + piece + te.text.slice(hi)
    : te.text.slice(0, hi) + piece + te.text.slice(hi);
  const caret = (replace ? lo : hi) + piece.length;
  await run({ cmd: 'setText', id: te.id, text: next, sel: [caret, caret], cell: te.cell });
}

/** Translates the selected shapes in place; returns how many changed. */
export async function translateSelectedShapes(
  run: SlideRun,
  shapes: readonly SlideShapeInfo[],
  provider: TranslationProvider,
  from: string,
  to: string,
  options: TranslateTextsOptions = {},
): Promise<number> {
  const chosen = shapes.filter(withText);
  if (!chosen.length) return 0;
  const out = await translateShapeTexts(
    provider,
    chosen.map((s) => s.text ?? ''),
    from,
    to,
    options,
  );
  if (options.signal?.aborted) return 0;
  let n = 0;
  for (const [i, s] of chosen.entries()) {
    if (out[i] === s.text) continue;
    await run({ cmd: 'setText', id: s.id, text: out[i]! });
    n++;
  }
  return n;
}

/** Texts of a deck: per slide, shapes with text and the notes. */
export interface DeckTexts {
  slides: { shapes: { id: number; text: string }[]; notes: string }[];
}

/** Reads every slide's texts, then goes back to slide `current` with `selected` shapes. */
export async function readDeckTexts(
  run: SlideRun,
  count: number,
  current: number,
  selected: number[],
): Promise<DeckTexts> {
  const slides: DeckTexts['slides'] = [];
  try {
    for (let i = 0; i < count; i++) {
      await run({ cmd: 'goTo', slide: i });
      const all = await run({ cmd: 'selectAll' });
      slides.push({
        shapes: (all?.selection ?? [])
          .filter(withText)
          .map((s) => ({ id: s.id, text: s.text ?? '' })),
        notes: all?.notes ?? '',
      });
    }
  } finally {
    await run({ cmd: 'goTo', slide: current });
    await run({ cmd: 'select', ids: selected });
  }
  return { slides };
}

/** Translates the deck's texts (shapes and notes); only changed texts stay in the result. */
export async function translateDeckTexts(
  deck: DeckTexts,
  provider: TranslationProvider,
  from: string,
  to: string,
  options: TranslateTextsOptions = {},
): Promise<{ deck: DeckTexts; count: number }> {
  const texts = deck.slides.flatMap((s) => [...s.shapes.map((x) => x.text), s.notes]);
  const out = await translateShapeTexts(provider, texts, from, to, options);
  let k = 0;
  let count = 0;
  const slides = deck.slides.map((s) => {
    const shapes = s.shapes.flatMap((x) => {
      const t = out[k++]!;
      if (t === x.text) return [];
      count++;
      return [{ id: x.id, text: t }];
    });
    const notesOut = out[k++]!;
    const notes = notesOut !== s.notes ? notesOut : '';
    if (notes) count++;
    return { shapes, notes };
  });
  return { deck: { slides }, count };
}

/** Writes translated texts into the open deck (the copy). */
export async function applyDeckTexts(run: SlideRun, deck: DeckTexts): Promise<void> {
  for (const [i, s] of deck.slides.entries()) {
    if (!s.shapes.length && !s.notes) continue;
    await run({ cmd: 'goTo', slide: i });
    for (const x of s.shapes) await run({ cmd: 'setText', id: x.id, text: x.text });
    if (s.notes) await run({ cmd: 'setNotes', slide: i, text: s.notes });
  }
  await run({ cmd: 'goTo', slide: 0 });
  await run({ cmd: 'select', ids: [] });
}
