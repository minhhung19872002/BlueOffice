/**
 * BlueSlide read aloud (CORE-LANG-003). While a text box is edited its paragraphs are read from
 * the caret (or only the selection) and the word being spoken is selected in the text
 * (`textSelect`, UTF-16 offsets like the speech events). Otherwise the text of the current
 * slide is read — the selected shapes, else every shape, top to bottom then left to right —
 * selecting each shape while it is read, then the speaker notes when asked for; the selection is
 * restored when reading stops.
 */
import type { ReadAloudRequest, ReadUnit } from '@blueoffice/editor';
import {
  detectLanguage,
  speechLanguage,
  type SlideCommand,
  type SlideEditResult,
  type SlideShapeInfo,
} from '@blueoffice/platform';

export interface SlideReadSource {
  run(cmd: SlideCommand): Promise<SlideEditResult | null>;
  /** The latest engine result. */
  edit: SlideEditResult | null;
  /** Also read the speaker notes (plain playback). */
  withNotes: boolean;
  fallbackLang: string;
}

const VT = /\v/g;

/** A read unit of slide text (line breaks spoken as pauses; same length). */
function unit(text: string, fallback: string): ReadUnit {
  const t = text.replace(VT, '\n');
  return { text: t, lang: speechLanguage(detectLanguage(t), null, fallback) };
}

/** Paragraphs of an edited text with their start offsets. */
export function paragraphsOf(text: string): { text: string; base: number }[] {
  const out: { text: string; base: number }[] = [];
  let base = 0;
  for (const p of text.split('\n')) {
    out.push({ text: p, base });
    base += p.length + 1;
  }
  return out;
}

const top = (s: SlideShapeInfo) => s.rect.cy - s.rect.h / 2;
const left = (s: SlideShapeInfo) => s.rect.cx - s.rect.w / 2;

/** Shapes in reading order: top to bottom, then left to right. */
export function readingOrder(shapes: readonly SlideShapeInfo[]): SlideShapeInfo[] {
  return shapes
    .filter((s) => s.text && s.text.trim())
    .sort((a, b) => top(a) - top(b) || left(a) - left(b));
}

export async function slideReadRequest(src: SlideReadSource): Promise<ReadAloudRequest> {
  const te = src.edit?.textEdit;
  if (te) {
    const paras = paragraphsOf(te.text);
    const lo = Math.min(te.start, te.end);
    const hi = Math.max(te.start, te.end);
    const collapsed = lo === hi;
    const units: ReadUnit[] = [];
    const bases: number[] = [];
    let first = 0;
    paras.forEach((p) => {
      const end = p.base + p.text.length;
      if (!collapsed && (end < lo || p.base > hi)) return;
      if (collapsed && p.base <= lo && lo <= end) first = units.length;
      const u = unit(p.text, src.fallbackLang);
      if (!collapsed && (p.base < lo || end > hi)) {
        u.range = { start: Math.max(0, lo - p.base), end: Math.min(p.text.length, hi - p.base) };
      }
      units.push(u);
      bases.push(p.base);
    });
    const select = (start: number, end: number) =>
      src.run({ cmd: 'textSelect', id: te.id, start, end, cell: te.cell });
    return {
      units,
      unit: collapsed ? first : 0,
      offset: collapsed ? lo - (bases[first] ?? 0) : undefined,
      onWord: (at) => {
        const b = bases[at.unit] ?? 0;
        void select(b + at.start, b + at.end);
      },
      onStop: () => void select(te.start, te.end),
    };
  }
  const before = src.edit?.selection ?? [];
  let shapes = readingOrder(before);
  if (!shapes.length) {
    const all = await src.run({ cmd: 'selectAll' });
    shapes = readingOrder(all?.selection ?? []);
  }
  const units = shapes.map((s) => unit(s.text ?? '', src.fallbackLang));
  const notes = src.edit?.notes ?? '';
  if (src.withNotes && notes.trim()) units.push(unit(notes, src.fallbackLang));
  const ids = before.map((s) => s.id);
  // Show which shape is read; nothing selected while the notes are read.
  if (!before.length && shapes.length) void src.run({ cmd: 'select', ids: [] });
  return {
    units,
    onUnit: (i) => {
      const s = shapes[i];
      void src.run({ cmd: 'select', ids: s ? [s.id] : [] });
    },
    onStop: () => void src.run({ cmd: 'select', ids }),
  };
}
