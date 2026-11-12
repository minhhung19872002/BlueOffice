import { useTranslation } from '@blueoffice/i18n';
import type { SlideExportOptions, SlidePaper } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import styles from './SlideEditor.module.css';

/** Slides per handout page the engine lays out. */
export const HANDOUT_SIZES = [1, 2, 3, 4, 6, 9] as const;
export type HandoutSize = (typeof HANDOUT_SIZES)[number];

export type ExportKind = 'handout' | 'notes' | 'image' | 'package';
export type PictureFormat = 'png' | 'jpg' | 'svg';

/** Everything the dialog asks. */
export interface ExportDocsState {
  kind: ExportKind;
  perPage: HandoutSize;
  order: 'horizontal' | 'vertical';
  frame: boolean;
  paper: Exclude<SlidePaper, 'notes'>;
  /** Handout pages in landscape orientation. */
  landscape: boolean;
  /** Header / footer from the handout / notes master. */
  fromMaster: boolean;
  header: string;
  footer: string;
  showDate: boolean;
  date: string;
  pageNumbers: boolean;
  format: PictureFormat;
  scope: 'current' | 'all';
  width: number;
  quality: number;
}

export function defaultExportState(date: string): ExportDocsState {
  return {
    kind: 'handout',
    perPage: 6,
    order: 'horizontal',
    frame: true,
    paper: 'a4',
    landscape: false,
    fromMaster: true,
    header: '',
    footer: '',
    showDate: true,
    date,
    pageNumbers: true,
    format: 'png',
    scope: 'current',
    width: 1280,
    quality: 90,
  };
}

/** A file the export produces. */
export interface ExportPlan {
  options: SlideExportOptions;
  name: string;
  mime: string;
}

/** Engine options and file name of the export described by `s`. */
export function exportPlan(
  s: ExportDocsState,
  base: string,
  current: number,
  count: number,
): ExportPlan {
  const hf = s.fromMaster
    ? {}
    : {
        header: s.header,
        footer: s.footer,
        date: s.showDate ? s.date : '',
        pageNumbers: s.pageNumbers,
      };
  switch (s.kind) {
    case 'handout':
      return {
        options: {
          kind: 'handout',
          perPage: s.perPage,
          order: s.order,
          frame: s.frame,
          paper: s.paper,
          ...(s.landscape ? { landscape: true } : {}),
          ...hf,
        },
        name: `${base}-handout-${s.perPage}.pdf`,
        mime: 'application/pdf',
      };
    case 'notes':
      return {
        options: { kind: 'notes', paper: s.paper, ...hf },
        name: `${base}-notes.pdf`,
        mime: 'application/pdf',
      };
    case 'image': {
      const width = Math.round(Math.min(8192, Math.max(16, s.width || 0)));
      const quality = Math.round(Math.min(100, Math.max(1, s.quality)));
      const one = s.scope === 'current' || count === 1;
      const slide = Math.min(Math.max(0, current), Math.max(0, count - 1));
      const mime =
        s.format === 'png' ? 'image/png' : s.format === 'jpg' ? 'image/jpeg' : 'image/svg+xml';
      return {
        options: {
          kind: 'image',
          format: s.format,
          ...(one ? { slides: [slide] } : {}),
          width,
          ...(s.format === 'jpg' ? { quality } : {}),
        },
        name: one ? `${base}-${slide + 1}.${s.format}` : `${base}-${s.format}.zip`,
        mime: one ? mime : 'application/zip',
      };
    }
    case 'package':
      return {
        options: { kind: 'package' },
        name: `${base}-package.zip`,
        mime: 'application/zip',
      };
  }
}

export interface ExportDocsDialogProps {
  open: boolean;
  /** Number of slides. */
  count: number;
  /** Slide size in points (for the picture height). */
  slide: { width: number; height: number };
  /** Today's date as the host formats it (default date text). */
  today: string;
  busy: boolean;
  onClose: () => void;
  onExport: (s: ExportDocsState) => void;
}

/**
 * Export the presentation as handouts, notes pages, slide pictures or a
 * package for another computer.
 */
export function ExportDocsDialog({
  open,
  count,
  slide,
  today,
  busy,
  onClose,
  onExport,
}: ExportDocsDialogProps) {
  const { t } = useTranslation();
  const [s, setS] = useState<ExportDocsState>(() => defaultExportState(today));
  const set = <K extends keyof ExportDocsState>(k: K, v: ExportDocsState[K]) =>
    setS((o) => ({ ...o, [k]: v }));
  const pages = s.kind === 'handout' || s.kind === 'notes';
  const height = slide.width > 0 ? Math.round((s.width * slide.height) / slide.width) : 0;
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={t('slideExport.title')}
      testId="export-docs-dialog"
      width={460}
      footer={
        <>
          <button type="button" onClick={onClose}>
            {t('slideEditor.cancel')}
          </button>
          <button
            type="button"
            data-testid="export-docs-ok"
            disabled={busy || count === 0}
            onClick={() => onExport(s)}
          >
            {t('slideEditor.export')}
          </button>
        </>
      }
    >
      <fieldset className={styles.exportForm}>
        <legend>{t('slideExport.what')}</legend>
        {(['handout', 'notes', 'image', 'package'] as const).map((k) => (
          <label key={k} className={styles.check}>
            <input
              type="radio"
              name="export-kind"
              data-testid={`export-kind-${k}`}
              checked={s.kind === k}
              onChange={() => set('kind', k)}
            />
            {t(`slideExport.kind.${k}`)}
          </label>
        ))}
      </fieldset>

      {s.kind === 'handout' && (
        <fieldset className={styles.exportForm}>
          <legend>{t('slideExport.handout')}</legend>
          <label>
            {t('slideExport.perPage')}{' '}
            <select
              data-testid="export-per-page"
              value={s.perPage}
              onChange={(e) => set('perPage', Number(e.target.value) as HandoutSize)}
            >
              {HANDOUT_SIZES.map((n) => (
                <option key={n} value={n}>
                  {n === 3 ? t('slideExport.perPageLines', { n }) : n}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t('slideExport.order')}{' '}
            <select
              data-testid="export-order"
              value={s.order}
              disabled={s.perPage < 4}
              onChange={(e) => set('order', e.target.value as ExportDocsState['order'])}
            >
              <option value="horizontal">{t('slideExport.orderHorizontal')}</option>
              <option value="vertical">{t('slideExport.orderVertical')}</option>
            </select>
          </label>
          <label className={styles.check}>
            <input
              type="checkbox"
              data-testid="export-frame"
              checked={s.frame}
              onChange={(e) => set('frame', e.target.checked)}
            />
            {t('slideExport.frame')}
          </label>
          <label>
            {t('slideExport.orientation')}{' '}
            <select
              data-testid="export-orientation"
              value={s.landscape ? 'landscape' : 'portrait'}
              onChange={(e) => set('landscape', e.target.value === 'landscape')}
            >
              <option value="portrait">{t('slideExport.portrait')}</option>
              <option value="landscape">{t('slideExport.landscape')}</option>
            </select>
          </label>
        </fieldset>
      )}

      {pages && (
        <>
          <label className={styles.exportForm}>
            {t('slideExport.paper')}
            <select
              data-testid="export-paper"
              value={s.paper}
              onChange={(e) => set('paper', e.target.value as ExportDocsState['paper'])}
            >
              <option value="a4">{t('slideExport.paperA4')}</option>
              <option value="letter">{t('slideExport.paperLetter')}</option>
            </select>
          </label>
          <fieldset className={styles.exportForm}>
            <legend>{t('slideExport.headerFooter')}</legend>
            <label className={styles.check}>
              <input
                type="checkbox"
                data-testid="export-from-master"
                checked={s.fromMaster}
                onChange={(e) => set('fromMaster', e.target.checked)}
              />
              {t(
                s.kind === 'handout'
                  ? 'slideExport.fromHandoutMaster'
                  : 'slideExport.fromNotesMaster',
              )}
            </label>
            {!s.fromMaster && (
              <>
                <label>
                  {t('slideExport.header')}{' '}
                  <input
                    type="text"
                    data-testid="export-header"
                    value={s.header}
                    onChange={(e) => set('header', e.target.value)}
                  />
                </label>
                <label>
                  {t('slideExport.footer')}{' '}
                  <input
                    type="text"
                    data-testid="export-footer"
                    value={s.footer}
                    onChange={(e) => set('footer', e.target.value)}
                  />
                </label>
                <label className={styles.check}>
                  <input
                    type="checkbox"
                    data-testid="export-show-date"
                    checked={s.showDate}
                    onChange={(e) => set('showDate', e.target.checked)}
                  />
                  {t('slideExport.date')}
                </label>
                {s.showDate && (
                  <input
                    type="text"
                    aria-label={t('slideExport.date')}
                    data-testid="export-date"
                    value={s.date}
                    onChange={(e) => set('date', e.target.value)}
                  />
                )}
                <label className={styles.check}>
                  <input
                    type="checkbox"
                    data-testid="export-page-numbers"
                    checked={s.pageNumbers}
                    onChange={(e) => set('pageNumbers', e.target.checked)}
                  />
                  {t('slideExport.pageNumbers')}
                </label>
              </>
            )}
          </fieldset>
        </>
      )}

      {s.kind === 'image' && (
        <fieldset className={styles.exportForm}>
          <legend>{t('slideExport.pictures')}</legend>
          <label>
            {t('slideExport.format')}{' '}
            <select
              data-testid="export-format"
              value={s.format}
              onChange={(e) => set('format', e.target.value as PictureFormat)}
            >
              <option value="png">{t('slideExport.formatPng')}</option>
              <option value="jpg">{t('slideExport.formatJpg')}</option>
              <option value="svg">{t('slideExport.formatSvg')}</option>
            </select>
          </label>
          <label className={styles.check}>
            <input
              type="radio"
              name="export-scope"
              checked={s.scope === 'current'}
              onChange={() => set('scope', 'current')}
            />
            {t('slideEditor.exportCurrent')}
          </label>
          <label className={styles.check}>
            <input
              type="radio"
              name="export-scope"
              data-testid="export-scope-all"
              checked={s.scope === 'all'}
              onChange={() => set('scope', 'all')}
            />
            {t('slideEditor.exportAll', { count })}
          </label>
          <label>
            {t('slideExport.width')}{' '}
            <input
              type="number"
              data-testid="export-width"
              min={16}
              max={8192}
              value={s.width}
              onChange={(e) => set('width', Number(e.target.value))}
            />{' '}
            <span data-testid="export-size">
              {t('slideExport.pixels', { w: s.width, h: height })}
            </span>
          </label>
          {s.format === 'jpg' && (
            <label>
              {t('slideExport.quality')}{' '}
              <input
                type="range"
                data-testid="export-quality"
                min={10}
                max={100}
                value={s.quality}
                onChange={(e) => set('quality', Number(e.target.value))}
              />{' '}
              {s.quality}
            </label>
          )}
        </fieldset>
      )}

      {s.kind === 'package' && (
        <p className={styles.exportForm} data-testid="export-package-note">
          {t('slideExport.packageNote')}
        </p>
      )}
    </Dialog>
  );
}
