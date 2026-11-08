import { renderPage, type GlyphStore, type ImageStore } from '@blueoffice/canvas-host';
import { useTranslation } from '@blueoffice/i18n';
import type { EngineHost } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import styles from './SlideEditor.module.css';

/** Pixels per point offered for PNG export. */
export const PNG_SCALES = [1, 2, 3];

/** PNG bytes of one slide drawn at `scale` px per point (same renderer as the editor). */
export async function slidePng(
  engine: EngineHost,
  glyphs: GlyphStore,
  images: ImageStore,
  index: number,
  scale: number,
): Promise<Uint8Array> {
  const page = await engine.getPage(index);
  await Promise.all([glyphs.ensure(page), images.ensure(page)]);
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.round(page.width * scale));
  canvas.height = Math.max(1, Math.round(page.height * scale));
  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error('canvas');
  renderPage(ctx, page, glyphs, scale, images);
  const blob = await new Promise<Blob | null>((ok) => canvas.toBlob(ok, 'image/png'));
  if (!blob) throw new Error('png');
  return new Uint8Array(await blob.arrayBuffer());
}

export interface ExportPngOptions {
  scope: 'current' | 'all';
  scale: number;
}

export interface ExportDialogProps {
  open: boolean;
  slide: { width: number; height: number };
  count: number;
  busy: boolean;
  onClose: () => void;
  onExport: (o: ExportPngOptions) => void;
}

/** Export slides as PNG: the current slide or all of them (one `.zip`), at a scale. */
export function ExportDialog({ open, slide, count, busy, onClose, onExport }: ExportDialogProps) {
  const { t } = useTranslation();
  const [scope, setScope] = useState<'current' | 'all'>('current');
  const [scale, setScale] = useState(2);
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={t('slideEditor.exportPng')}
      testId="export-png-dialog"
      footer={
        <>
          <button type="button" onClick={onClose}>
            {t('slideEditor.cancel')}
          </button>
          <button
            type="button"
            data-testid="export-png-ok"
            disabled={busy}
            onClick={() => onExport({ scope, scale })}
          >
            {t('slideEditor.export')}
          </button>
        </>
      }
    >
      <fieldset className={styles.exportForm}>
        <legend>{t('slideEditor.exportSlides')}</legend>
        <label className={styles.check}>
          <input
            type="radio"
            name="png-scope"
            checked={scope === 'current'}
            onChange={() => setScope('current')}
          />
          {t('slideEditor.exportCurrent')}
        </label>
        <label className={styles.check}>
          <input
            type="radio"
            name="png-scope"
            data-testid="export-png-all"
            checked={scope === 'all'}
            onChange={() => setScope('all')}
          />
          {t('slideEditor.exportAll', { count })}
        </label>
      </fieldset>
      <label className={styles.exportForm}>
        {t('slideEditor.exportScale')}
        <select
          data-testid="export-png-scale"
          value={scale}
          onChange={(e) => setScale(Number(e.target.value))}
        >
          {PNG_SCALES.map((s) => (
            <option key={s} value={s}>
              {t('slideEditor.exportScaleOption', {
                percent: s * 100,
                w: Math.round(slide.width * s),
                h: Math.round(slide.height * s),
              })}
            </option>
          ))}
        </select>
      </label>
    </Dialog>
  );
}
