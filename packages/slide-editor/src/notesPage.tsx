/**
 * BlueSlide notes page view (SLIDE-VIEW-001): one page per slide at the notes
 * page size (ECMA-376 Part 1, §19.2.1.23 notesSz) with the slide picture in the
 * slide image box and the speaker notes, editable, in the notes box — both
 * boxes from the notes slide or the notes master (§19.3.1.26 notes,
 * §19.3.1.27 notesMaster). A notes edit is the undoable `setNotes` command.
 * The header, date, footer and page number come from the notes master's
 * placeholders shown by its `p:hf` flags (§19.3.1.25).
 */
import { useTranslation } from '@blueoffice/i18n';
import type { SlideCommand, SlideEditResult, SlideNotesPage } from '@blueoffice/platform';
import { useEffect, useState, type ReactNode } from 'react';
import { fromEditorText, toEditorText } from './editing';
import styles from './views.module.css';

export interface NotesPageViewProps {
  runResult: (cmd: SlideCommand) => Promise<SlideEditResult | null>;
  run: (cmd: SlideCommand) => void;
  /** Changes whenever the deck changed. */
  revision: number;
  current: number;
  onSelect: (slide: number) => void;
  /** Slide picture of `index` at `width` CSS pixels. */
  canvas: (index: number, width: number) => ReactNode;
  /** Width of a page on screen (CSS pixels). */
  pageWidth?: number;
}

/** Page box → CSS (pixels) at `k` pixels per point. */
export function boxStyle(b: { x: number; y: number; w: number; h: number }, k: number) {
  return { left: b.x * k, top: b.y * k, width: b.w * k, height: b.h * k };
}

export function NotesPageView({
  runResult,
  run,
  revision,
  current,
  onSelect,
  canvas,
  pageWidth = 560,
}: NotesPageViewProps) {
  const { t } = useTranslation();
  const [pages, setPages] = useState<SlideNotesPage[]>([]);
  const [drafts, setDrafts] = useState<Record<number, string>>({});

  useEffect(() => {
    let alive = true;
    void runResult({ cmd: 'notesPages' }).then((r) => {
      if (alive && r?.notesPages) setPages(r.notesPages);
    });
    return () => {
      alive = false;
    };
  }, [revision, runResult]);

  const commit = (i: number) => {
    const d = drafts[i];
    if (d === undefined) return;
    setDrafts((all) => {
      const rest = { ...all };
      delete rest[i];
      return rest;
    });
    const text = fromEditorText(d);
    if (text !== (pages[i]?.text ?? '')) run({ cmd: 'setNotes', slide: i, text });
  };

  return (
    <div className={styles.notesPages} data-testid="notes-page-view">
      {pages.map((p, i) => {
        const k = pageWidth / (p.width || 1);
        return (
          <section
            key={i}
            className={styles.notesPage}
            data-testid={`notes-page-${i + 1}`}
            data-current={i === current ? 'true' : undefined}
            aria-label={t('slideEditor.views.notesPageOf', { n: i + 1, total: pages.length })}
            style={{ width: pageWidth, height: p.height * k }}
          >
            <button
              type="button"
              className={styles.notesPageImage}
              style={boxStyle(p.image, k)}
              aria-label={t('slideEditor.slideOf', { n: i + 1, total: pages.length })}
              onClick={() => onSelect(i)}
            >
              {canvas(i, p.image.w * k)}
            </button>
            {(p.hf ?? []).map((h) => (
              <div
                key={h.kind}
                className={styles.notesPageHf}
                data-testid={`notes-page-hf-${h.kind}-${i + 1}`}
                data-bottom={h.bottom ? 'true' : undefined}
                style={{ ...boxStyle(h.rect, k), fontSize: h.size * k, textAlign: h.align }}
              >
                {h.text}
              </div>
            ))}
            <textarea
              className={styles.notesPageText}
              data-testid={`notes-page-text-${i + 1}`}
              style={{ ...boxStyle(p.body, k), fontSize: p.size * k }}
              aria-label={t('slideEditor.notes')}
              placeholder={t('slideEditor.notesPlaceholder')}
              value={drafts[i] ?? toEditorText(p.text)}
              onFocus={() => i !== current && onSelect(i)}
              onChange={(e) => {
                const v = e.target.value;
                setDrafts((d) => ({ ...d, [i]: v }));
              }}
              onBlur={() => commit(i)}
            />
          </section>
        );
      })}
    </div>
  );
}
