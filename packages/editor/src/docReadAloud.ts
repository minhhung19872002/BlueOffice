/**
 * BlueDoc read aloud (CORE-LANG-003): the body paragraphs come from the engine (`speechText`,
 * index text + language + hidden/deleted ranges); reading starts at the caret, or covers only
 * the selection. The word being spoken (UTF-16 offsets of the speech engine) maps back to byte
 * offsets of `Pos`, whose rectangles (`speechRects`) the view highlights with the paragraph.
 */
import {
  byteToUtf16,
  speechLanguage,
  utf16ToByte,
  type DocSpeechPara,
  type EngineHost,
  type SelRect,
} from '@blueoffice/platform';
import type { ReadAloudRequest } from './ReadAloudBar';
import type { ReadUnit } from './readAloud';

/** The view's read-aloud highlight (`DocumentView.setSpeechHighlights`). */
export interface SpeechHighlighter {
  setSpeechHighlights(word: SelRect[], para: SelRect[], follow?: boolean): void;
}

/** Read units of the paragraphs (byte offsets → UTF-16), selection `[from, to]` when given. */
export function docUnits(
  paras: readonly DocSpeechPara[],
  fallbackLang: string,
  sel?: { from: { p: number; o: number }; to: { p: number; o: number } },
): { units: ReadUnit[]; paras: DocSpeechPara[] } {
  const chosen = sel ? paras.filter((x) => x.p >= sel.from.p && x.p <= sel.to.p) : [...paras];
  const units = chosen.map((x): ReadUnit => {
    const u: ReadUnit = {
      text: x.text,
      lang: speechLanguage(x.detected, x.lang, fallbackLang),
      skip: (x.skip ?? []).map(([a, b]) => ({
        start: byteToUtf16(x.text, a),
        end: byteToUtf16(x.text, b),
      })),
    };
    if (sel && (x.p === sel.from.p || x.p === sel.to.p)) {
      u.range = {
        start: x.p === sel.from.p ? byteToUtf16(x.text, sel.from.o) : 0,
        end: x.p === sel.to.p ? byteToUtf16(x.text, sel.to.o) : x.text.length,
      };
    }
    return u;
  });
  return { units, paras: chosen };
}

const before = (a: { p: number; o: number }, b: { p: number; o: number }) =>
  a.p < b.p || (a.p === b.p && a.o < b.o);

/** What to read in the open document, and how the view follows it. */
export async function docReadRequest(
  engine: EngineHost,
  view: () => SpeechHighlighter | null,
  fallbackLang: string,
): Promise<ReadAloudRequest> {
  const state = await engine.command({ cmd: 'state' });
  const { anchor, focus } = state.selection;
  const collapsed = anchor.p === focus.p && anchor.o === focus.o;
  const [from, to] = before(focus, anchor) ? [focus, anchor] : [anchor, focus];
  const r = await engine.command({ cmd: 'speechText' });
  const all = r.speech?.paras ?? [];
  const { units, paras } = docUnits(all, fallbackLang, collapsed ? undefined : { from, to });
  let unit = 0;
  let offset: number | undefined;
  if (collapsed) {
    unit = Math.max(
      0,
      paras.findIndex((x) => x.p === from.p),
    );
    const at = paras[unit];
    offset = at ? byteToUtf16(at.text, from.o) : 0;
  }

  // One rectangles query at a time; words arriving meanwhile replace the waiting one.
  let busy = false;
  let waiting: { unit: number; start: number; end: number } | null = null;
  let stopped = false;
  const show = async (at: { unit: number; start: number; end: number }) => {
    if (busy) {
      waiting = at;
      return;
    }
    busy = true;
    try {
      for (let next: typeof at | null = at; next && !stopped; next = waiting) {
        waiting = null;
        const x = paras[next.unit];
        const u = units[next.unit];
        if (!x || !u) continue;
        const ranges = [
          { p: x.p, start: utf16ToByte(x.text, next.start), end: utf16ToByte(x.text, next.end) },
          {
            p: x.p,
            start: utf16ToByte(x.text, u.range?.start ?? 0),
            end: utf16ToByte(x.text, u.range?.end ?? x.text.length),
          },
        ];
        const res = await engine.command({ cmd: 'speechRects', ranges });
        if (stopped) break;
        const [word = [], para = []] = res.speech?.rects ?? [];
        view()?.setSpeechHighlights(word, para, true);
      }
    } finally {
      busy = false;
    }
  };

  return {
    units,
    unit,
    offset,
    onWord: (at) => void show(at),
    onStop: () => {
      stopped = true;
      waiting = null;
      view()?.setSpeechHighlights([], [], false);
    },
  };
}
