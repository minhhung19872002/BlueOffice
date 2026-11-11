/**
 * Find and Replace of BlueSlide (Ctrl+F, Ctrl+H).
 *
 * Find next walks the text shapes from the last match on, slide after slide, wraps at
 * the end of the deck and selects the shape that holds the match; Replace changes that
 * match and looks for the next one; Replace all changes every match of the deck as one
 * undo step and tells how many were changed.
 */
import { useTranslation } from '@blueoffice/i18n';
import type { SlideCommand, SlideEditResult, SlideShapeInfo } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useRef, useState } from 'react';
import styles from './views.module.css';

export interface FindOptions {
  matchCase: boolean;
  wholeWord: boolean;
}

/** A match: slide (0-based), shape, UTF-16 range in the shape's text. */
export interface FindHit {
  slide: number;
  id: number;
  start: number;
  end: number;
}

const isWordChar = (ch: string | undefined) => !!ch && /[\p{L}\p{N}_]/u.test(ch);

/** Ranges of `query` in `text`, left to right, not overlapping. */
export function findInText(text: string, query: string, opts: FindOptions): [number, number][] {
  const out: [number, number][] = [];
  if (!query) return out;
  // Lower-casing keeps the length for the scripts used here; when it does not, compare as is.
  const fold = (s: string) => {
    const l = s.toLocaleLowerCase();
    return l.length === s.length ? l : s;
  };
  const hay = opts.matchCase ? text : fold(text);
  const needle = opts.matchCase ? query : fold(query);
  let from = 0;
  for (;;) {
    const at = hay.indexOf(needle, from);
    if (at < 0) break;
    const end = at + needle.length;
    if (!opts.wholeWord || (!isWordChar(text[at - 1]) && !isWordChar(text[end]))) {
      out.push([at, end]);
      from = end;
    } else from = at + 1;
  }
  return out;
}

/** `text` with every match replaced, and how many there were. */
export function replaceInText(
  text: string,
  query: string,
  replacement: string,
  opts: FindOptions,
): { text: string; count: number } {
  const hits = findInText(text, query, opts);
  let out = '';
  let from = 0;
  for (const [a, z] of hits) {
    out += text.slice(from, a) + replacement;
    from = z;
  }
  return { text: out + text.slice(from), count: hits.length };
}

type Run = (cmd: SlideCommand) => Promise<SlideEditResult | null>;

const searchable = (s: SlideShapeInfo) => s.editableText && s.kind !== 'table' && !!s.text;

/** The text shapes of slide `i` (the slide becomes the current one). */
async function shapesOf(run: Run, i: number): Promise<SlideShapeInfo[]> {
  await run({ cmd: 'goTo', slide: i });
  const all = await run({ cmd: 'selectAll' });
  await run({ cmd: 'select', ids: [] });
  return (all?.selection ?? []).filter(searchable);
}

/**
 * The first match after `after` (a previous hit; without one: from the start of slide
 * `from`), going on through the deck and wrapping at its end; the shape holding the
 * match ends up selected on its slide.
 */
export async function findNext(
  run: Run,
  count: number,
  from: number,
  after: FindHit | null,
  query: string,
  opts: FindOptions,
): Promise<FindHit | null> {
  if (!query || count <= 0) return null;
  const start = Math.max(0, Math.min(after ? after.slide : from, count - 1));
  const hitsOf = async (slide: number) => {
    const shapes = await shapesOf(run, slide);
    const order = new Map(shapes.map((s, i) => [s.id, i]));
    const hits = shapes.flatMap((s) =>
      findInText(s.text ?? '', query, opts).map(([a, z]): FindHit => ({
        slide,
        id: s.id,
        start: a,
        end: z,
      })),
    );
    return { hits, order };
  };
  const first = await hitsOf(start);
  const at = after ? first.order.get(after.id) : undefined;
  const isAfter = (h: FindHit) =>
    !after ||
    at === undefined ||
    (first.order.get(h.id) ?? 0) > at ||
    (h.id === after.id && h.start >= after.end);
  let found = first.hits.find(isAfter);
  for (let k = 1; !found && k < count; k++) {
    found = (await hitsOf((start + k) % count)).hits[0];
  }
  // Wrapped: what is before the last hit on its slide.
  found ??= first.hits.find((h) => !isAfter(h));
  await run({ cmd: 'goTo', slide: found ? found.slide : start });
  if (found) await run({ cmd: 'select', ids: [found.id] });
  return found ?? null;
}

/** Replaces the match `hit` when the text still holds it; true when it did. */
export async function replaceHit(
  run: Run,
  hit: FindHit,
  query: string,
  replacement: string,
  opts: FindOptions,
): Promise<boolean> {
  await run({ cmd: 'goTo', slide: hit.slide });
  const r = await run({ cmd: 'select', ids: [hit.id] });
  const text = r?.selection.find((s) => s.id === hit.id)?.text ?? '';
  const still = findInText(text, query, opts).some(([a, z]) => a === hit.start && z === hit.end);
  if (!still) return false;
  await run({
    cmd: 'setText',
    id: hit.id,
    text: text.slice(0, hit.start) + replacement + text.slice(hit.end),
  });
  return true;
}

/** Replaces every match of the deck (one undo step); the number of replacements. */
export async function replaceAll(
  run: Run,
  count: number,
  current: number,
  query: string,
  replacement: string,
  opts: FindOptions,
): Promise<number> {
  if (!query) return 0;
  const since = (await run({ cmd: 'historyLen' }))?.history;
  let n = 0;
  try {
    for (let i = 0; i < count; i++) {
      for (const s of await shapesOf(run, i)) {
        const r = replaceInText(s.text ?? '', query, replacement, opts);
        if (!r.count) continue;
        await run({ cmd: 'setText', id: s.id, text: r.text });
        n += r.count;
      }
    }
  } finally {
    await run({ cmd: 'goTo', slide: current });
    if (n > 1 && since !== undefined) await run({ cmd: 'mergeHistory', since });
  }
  return n;
}

export interface FindReplaceProps {
  /** `find`: only the search; `replace`: with the replacement row. */
  mode: 'find' | 'replace' | null;
  onClose: () => void;
  run: Run;
  count: number;
  current: number;
  onError: (e: unknown) => void;
}

export function FindReplaceDialog(props: FindReplaceProps) {
  return props.mode ? <FindForm {...props} mode={props.mode} /> : null;
}

function FindForm({
  mode,
  onClose,
  run,
  count,
  current,
  onError,
}: FindReplaceProps & { mode: 'find' | 'replace' }) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [replacement, setReplacement] = useState('');
  const [matchCase, setMatchCase] = useState(false);
  const [wholeWord, setWholeWord] = useState(false);
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const hit = useRef<FindHit | null>(null);
  const opts = { matchCase, wholeWord };
  // Another search starts over from the current slide.
  const fresh = () => {
    hit.current = null;
    setMessage('');
  };

  const guard = (f: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    void f()
      .catch(onError)
      .finally(() => setBusy(false));
  };
  const next = async () => {
    const h = await findNext(run, count, current, hit.current, query, opts);
    hit.current = h;
    setMessage(
      h ? t('slideEditor.find.found', { slide: h.slide + 1 }) : t('slideEditor.find.notFound'),
    );
  };
  const replaceOne = async () => {
    const h = hit.current;
    if (h && (await replaceHit(run, h, query, replacement, opts))) {
      // The text after the replacement starts where the replacement ends.
      hit.current = { ...h, end: h.start + replacement.length };
    }
    await next();
  };
  const all = async () => {
    const n = await replaceAll(run, count, current, query, replacement, opts);
    hit.current = null;
    setMessage(n ? t('slideEditor.find.replaced', { count: n }) : t('slideEditor.find.notFound'));
  };
  const ok = query.length > 0 && !busy;

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t(mode === 'find' ? 'slideEditor.find.title' : 'slideEditor.find.replaceTitle')}
      testId="slide-find-dialog"
      footer={
        <>
          <button
            type="button"
            data-testid="slide-find-next"
            disabled={!ok}
            onClick={() => guard(next)}
          >
            {t('slideEditor.find.next')}
          </button>
          {mode === 'replace' && (
            <>
              <button
                type="button"
                data-testid="slide-find-replace"
                disabled={!ok}
                onClick={() => guard(replaceOne)}
              >
                {t('slideEditor.find.replace')}
              </button>
              <button
                type="button"
                data-testid="slide-find-replace-all"
                disabled={!ok}
                onClick={() => guard(all)}
              >
                {t('slideEditor.find.replaceAll')}
              </button>
            </>
          )}
          <button type="button" onClick={onClose}>
            {t('slideEditor.find.close')}
          </button>
        </>
      }
    >
      <label className={styles.form}>
        {t('slideEditor.find.what')}
        <input
          data-testid="slide-find-what"
          value={query}
          onChange={(e) => {
            fresh();
            setQuery(e.target.value);
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && ok) guard(next);
          }}
          autoFocus
        />
      </label>
      {mode === 'replace' && (
        <label className={styles.form}>
          {t('slideEditor.find.with')}
          <input
            data-testid="slide-find-with"
            value={replacement}
            onChange={(e) => setReplacement(e.target.value)}
          />
        </label>
      )}
      <label>
        <input
          type="checkbox"
          data-testid="slide-find-case"
          checked={matchCase}
          onChange={(e) => {
            fresh();
            setMatchCase(e.target.checked);
          }}
        />{' '}
        {t('slideEditor.find.matchCase')}
      </label>{' '}
      <label>
        <input
          type="checkbox"
          data-testid="slide-find-word"
          checked={wholeWord}
          onChange={(e) => {
            fresh();
            setWholeWord(e.target.checked);
          }}
        />{' '}
        {t('slideEditor.find.wholeWord')}
      </label>
      <p role="status" data-testid="slide-find-status">
        {message}
      </p>
    </Dialog>
  );
}
