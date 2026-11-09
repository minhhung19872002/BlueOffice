/**
 * Editable outline of a deck (SLIDE-VIEW-001): one line per slide title and
 * per body paragraph. The editing operations are pure functions; the outline
 * view sends the result to the engine as one `applyOutline` command.
 *
 * Keys (our choice, see the outline view): Enter → new bullet (a title gets a
 * first bullet), Tab / Shift+Tab → demote / promote (a level-0 bullet promoted
 * becomes the title of a new slide, a title demoted becomes a bullet of the
 * previous slide), Ctrl+Enter → new slide after the current one.
 */
import type { SlideOutlineEdit, SlideOutlineSlide } from '@blueoffice/platform';

/** Deepest bullet level (ECMA-376 Part 1, §21.1.2.2.7 `pPr@lvl` 0–8). */
export const MAX_OUTLINE_LEVEL = 8;

export interface OutlineLine {
  kind: 'title' | 'bullet';
  /** Bullet level 0–8 (0 for titles). */
  level: number;
  text: string;
  /** Titles: the slide this line stands for (`null`: a new slide). */
  slide: number | null;
}

/** Result of an editing operation: the new lines and the line to focus. */
export interface OutlineChange {
  lines: OutlineLine[];
  focus: number;
  /** Caret position in the focused line. */
  caret: number;
}

export function linesOf(outline: SlideOutlineSlide[]): OutlineLine[] {
  const out: OutlineLine[] = [];
  outline.forEach((s, i) => {
    out.push({ kind: 'title', level: 0, text: s.title, slide: i });
    for (const b of s.bullets)
      out.push({ kind: 'bullet', level: b.level, text: b.text, slide: null });
  });
  return out;
}

/** The `applyOutline` slides of the lines. */
export function editsOf(lines: OutlineLine[]): SlideOutlineEdit[] {
  const out: SlideOutlineEdit[] = [];
  for (const l of lines) {
    if (l.kind === 'title' || out.length === 0) {
      const e: SlideOutlineEdit = { title: l.kind === 'title' ? l.text : '', bullets: [] };
      if (l.kind === 'title' && l.slide !== null) e.slide = l.slide;
      out.push(e);
      if (l.kind === 'title') continue;
    }
    out[out.length - 1]!.bullets.push({ text: l.text, level: l.level });
  }
  return out;
}

/** Index (in the edited deck) of the slide holding line `i`. */
export function slideOfLine(lines: OutlineLine[], i: number): number {
  let n = -1;
  for (let k = 0; k <= i && k < lines.length; k++) if (lines[k]!.kind === 'title') n++;
  return Math.max(0, n);
}

/** After the outline was applied, slide `k` of the deck is the `k`-th title. */
export function renumber(lines: OutlineLine[]): OutlineLine[] {
  let n = 0;
  return lines.map((l) => (l.kind === 'title' ? { ...l, slide: n++ } : l));
}

const replace = (lines: OutlineLine[], i: number, ...items: OutlineLine[]) => [
  ...lines.slice(0, i),
  ...items,
  ...lines.slice(i + 1),
];

/** Enter at `caret` in line `i`: the text after the caret goes to a new bullet
 *  below (same level; level 0 under a title). */
export function enter(lines: OutlineLine[], i: number, caret: number): OutlineChange {
  const l = lines[i]!;
  const at = Math.max(0, Math.min(caret, l.text.length));
  const head = { ...l, text: l.text.slice(0, at) };
  const tail: OutlineLine = {
    kind: 'bullet',
    level: l.kind === 'title' ? 0 : l.level,
    text: l.text.slice(at),
    slide: null,
  };
  return { lines: replace(lines, i, head, tail), focus: i + 1, caret: 0 };
}

/** Tab: a bullet goes one level deeper; a title (not the first) becomes a
 *  level-0 bullet of the previous slide. `null`: nothing to do. */
export function demote(lines: OutlineLine[], i: number): OutlineChange | null {
  const l = lines[i]!;
  if (l.kind === 'title') {
    if (!lines.slice(0, i).some((x) => x.kind === 'title')) return null;
    return {
      lines: replace(lines, i, { kind: 'bullet', level: 0, text: l.text, slide: null }),
      focus: i,
      caret: l.text.length,
    };
  }
  if (l.level >= MAX_OUTLINE_LEVEL) return null;
  return { lines: replace(lines, i, { ...l, level: l.level + 1 }), focus: i, caret: l.text.length };
}

/** Shift+Tab: a bullet goes one level up; a level-0 bullet becomes the title
 *  of a new slide (the bullets after it follow it). */
export function promote(lines: OutlineLine[], i: number): OutlineChange | null {
  const l = lines[i]!;
  if (l.kind === 'title') return null;
  const next: OutlineLine =
    l.level > 0
      ? { ...l, level: l.level - 1 }
      : { kind: 'title', level: 0, text: l.text, slide: null };
  return { lines: replace(lines, i, next), focus: i, caret: l.text.length };
}

/** Ctrl+Enter: an empty new slide after the slide holding line `i`. */
export function newSlide(lines: OutlineLine[], i: number): OutlineChange {
  let at = i + 1;
  while (at < lines.length && lines[at]!.kind !== 'title') at++;
  const title: OutlineLine = { kind: 'title', level: 0, text: '', slide: null };
  return { lines: [...lines.slice(0, at), title, ...lines.slice(at)], focus: at, caret: 0 };
}

/** Backspace on an empty line (not the first): remove it. */
export function removeEmpty(lines: OutlineLine[], i: number): OutlineChange | null {
  const l = lines[i];
  if (!l || i === 0 || l.text !== '') return null;
  const prev = lines[i - 1]!;
  return { lines: lines.filter((_, k) => k !== i), focus: i - 1, caret: prev.text.length };
}
