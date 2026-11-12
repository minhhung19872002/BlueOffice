/**
 * BlueSlide outline view (SLIDE-VIEW-001): every slide title and body
 * paragraph as an editable line. Structure changes (Enter, Tab, Shift+Tab,
 * Ctrl+Enter, Backspace on an empty line) and text edits (when a line loses
 * focus) are sent as one `applyOutline` command each — one undo step in the
 * engine. Ctrl+Z / Ctrl+Y (Ctrl+Shift+Z) in a line are the engine's undo /
 * redo, not the text box's own: typing not sent yet becomes a step first.
 */
import { useTranslation } from '@blueoffice/i18n';
import type { SlideCommand, SlideEditResult, SlideOutlineEdit } from '@blueoffice/platform';
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from 'react';
import { fromEditorText, toEditorText } from './editing';
import {
  demote,
  editsOf,
  enter,
  linesOf,
  newSlide,
  outlineKey,
  promote,
  removeEmpty,
  renumber,
  slideOfLine,
  type OutlineChange,
  type OutlineLine,
} from './outlineModel';
import styles from './views.module.css';

export interface OutlineViewProps {
  /** Runs a command and resolves with its result (`null` on failure). */
  runResult: (cmd: SlideCommand) => Promise<SlideEditResult | null>;
  /** Changes whenever the deck changed (re-reads the outline when idle). */
  revision: number;
  /** Current slide. */
  current: number;
  /** A line got the focus: its slide becomes the current one. */
  onSelect: (slide: number) => void;
  /** Preview of the current slide beside the outline. */
  preview?: ReactNode;
}

export function OutlineView({ runResult, revision, current, onSelect, preview }: OutlineViewProps) {
  const { t } = useTranslation();
  const [lines, setLines] = useState<OutlineLine[]>([]);
  /** Lines as the engine last saw them (detects text edits to send). */
  const sent = useRef<string>('');
  /** Local text edits not sent yet / commands in flight: no re-read meanwhile. */
  const dirty = useRef(false);
  const inFlight = useRef(0);
  const focusAt = useRef<{ line: number; caret: number } | null>(null);
  const refs = useRef<(HTMLInputElement | null)[]>([]);
  /** The last `applyOutline` sent (undo / redo waits for it). */
  const pending = useRef<Promise<unknown>>(Promise.resolve());

  const load = useCallback(
    async (force = false) => {
      const r = await runResult({ cmd: 'outline' });
      if (!r?.outline) return;
      if (!force && (dirty.current || inFlight.current > 0)) return;
      const next = linesOf(r.outline);
      // The engine's answer to our own last edit: keep the lines as typed (focus, empty lines).
      if (
        sent.current &&
        outlineKey(editsOf(next)) === outlineKey(JSON.parse(sent.current) as SlideOutlineEdit[])
      )
        return;
      sent.current = JSON.stringify(editsOf(next));
      setLines(next);
    },
    [runResult],
  );

  /** Engine undo / redo from a line: the outline is read back afterwards. */
  const history = useCallback(
    async (cmd: 'undo' | 'redo', line: number) => {
      await pending.current;
      inFlight.current++;
      try {
        await runResult({ cmd });
      } finally {
        inFlight.current--;
      }
      dirty.current = false;
      sent.current = '';
      const el = refs.current[line];
      focusAt.current = { line, caret: el?.value.length ?? 0 };
      await load(true);
    },
    [runResult, load],
  );

  useEffect(() => {
    if (!dirty.current && inFlight.current === 0) void load();
  }, [revision, load]);

  const commit = useCallback(
    (next: OutlineLine[], focus: number) => {
      const slides = editsOf(next);
      const key = JSON.stringify(slides);
      dirty.current = false;
      if (key === sent.current) return next;
      inFlight.current++;
      const numbered = renumber(next);
      sent.current = JSON.stringify(editsOf(numbered));
      const done = runResult({ cmd: 'applyOutline', slides, current: slideOfLine(next, focus) });
      pending.current = done.catch(() => null);
      void done
        .then((r) => {
          inFlight.current--;
          if (!r || r.error) {
            dirty.current = false;
            void load();
          }
        })
        .catch(() => {
          inFlight.current--;
        });
      return numbered;
    },
    [runResult, load],
  );

  const change = (c: OutlineChange | null) => {
    if (!c) return;
    focusAt.current = { line: c.focus, caret: c.caret };
    setLines(commit(c.lines, c.focus));
  };

  useLayoutEffect(() => {
    const f = focusAt.current;
    if (!f) return;
    focusAt.current = null;
    const el = refs.current[f.line];
    if (el) {
      el.focus();
      el.setSelectionRange(f.caret, f.caret);
    }
  });

  const onKey = (e: KeyboardEvent<HTMLInputElement>, i: number) => {
    const caret = e.currentTarget.selectionStart ?? 0;
    const mod = e.ctrlKey || e.metaKey;
    const k = e.key.toLowerCase();
    if (mod && !e.altKey && (k === 'z' || k === 'y')) {
      // The engine's history, not the text box's (one step per outline change).
      e.preventDefault();
      e.stopPropagation();
      const redo = k === 'y' || e.shiftKey;
      if (dirty.current) setLines(commit(lines, i));
      void history(redo ? 'redo' : 'undo', i);
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      change(newSlide(lines, i));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      change(enter(lines, i, caret));
    } else if (e.key === 'Tab') {
      e.preventDefault();
      change(e.shiftKey ? promote(lines, i) : demote(lines, i));
    } else if (e.key === 'Backspace' && caret === 0 && e.currentTarget.selectionEnd === 0) {
      const c = removeEmpty(lines, i);
      if (c) {
        e.preventDefault();
        change(c);
      }
    } else if (e.key === 'ArrowUp' && i > 0) {
      e.preventDefault();
      refs.current[i - 1]?.focus();
    } else if (e.key === 'ArrowDown' && i + 1 < lines.length) {
      e.preventDefault();
      refs.current[i + 1]?.focus();
    }
  };

  let slideNo = 0;
  return (
    <div className={styles.outlineView} data-testid="outline-view">
      <div className={styles.outlinePane} role="list" aria-label={t('slideEditor.views.outline')}>
        {lines.map((l, i) => {
          if (l.kind === 'title') slideNo++;
          const n = slideNo;
          return (
            <div
              key={i}
              role="listitem"
              className={l.kind === 'title' ? styles.outlineTitle : styles.outlineBullet}
              style={{ paddingLeft: l.kind === 'title' ? 0 : 28 + l.level * 20 }}
              data-current={l.kind === 'title' && n - 1 === current ? 'true' : undefined}
            >
              {l.kind === 'title' && (
                <span className={styles.outlineNo} data-testid={`outline-slide-${n}`}>
                  {n}
                </span>
              )}
              <input
                ref={(el) => {
                  refs.current[i] = el;
                }}
                className={styles.outlineInput}
                data-testid={`outline-line-${i}`}
                data-kind={l.kind}
                data-level={l.level}
                aria-label={
                  l.kind === 'title'
                    ? t('slideEditor.views.outlineTitle', { n })
                    : t('slideEditor.views.outlineBullet', { level: l.level + 1 })
                }
                placeholder={l.kind === 'title' ? t('slideEditor.views.outlineTitleHint') : ''}
                value={toEditorText(l.text)}
                onChange={(e) => {
                  dirty.current = true;
                  const text = fromEditorText(e.target.value);
                  setLines((ls) => ls.map((x, k) => (k === i ? { ...x, text } : x)));
                }}
                onFocus={() => {
                  const s = slideOfLine(lines, i);
                  if (s !== current) onSelect(s);
                }}
                onBlur={() => {
                  if (dirty.current) setLines(commit(lines, i));
                }}
                onKeyDown={(e) => onKey(e, i)}
              />
            </div>
          );
        })}
        <p className={styles.outlineHelp}>{t('slideEditor.views.outlineHelp')}</p>
      </div>
      {preview && <div className={styles.outlinePreview}>{preview}</div>}
    </div>
  );
}
